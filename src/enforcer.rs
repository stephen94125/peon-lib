use casbin::prelude::*;
use log::{debug, warn, info};
use std::sync::Arc;
use std::env;
use tokio::sync::RwLock;

/// The Security Enforcer for Peon.
/// Wraps a Casbin CoreEnforcer.
pub struct FileEnforcer {
    enforcer: RwLock<Enforcer>,
}

impl FileEnforcer {
    /// Creates a new FileEnforcer with the embedded model and memory adapter.
    pub async fn new() -> Arc<Self> {
        let model_conf = include_str!("file_enforcer_model.conf");
        let m = DefaultModel::from_str(model_conf).await.unwrap();
        let a = MemoryAdapter::default();
        let e = Enforcer::new(m, a).await.unwrap();
        let file_enforcer = Arc::new(Self {
            enforcer: RwLock::new(e),
        });

        // Try to load file_permissions.txt from current directory
        if let Ok(cwd) = env::current_dir() {
            let perm_file = cwd.join("file_permissions.txt");
            if perm_file.exists() {
                if let Ok(content) = tokio::fs::read_to_string(perm_file).await {
                    file_enforcer.load_permissions_from_string(&content).await;
                }
            }
        }

        file_enforcer
    }

    /// Loads custom permission syntax and adds them to Casbin.
    pub async fn load_permissions_from_string(&self, rules: &str) {
        let mut e = self.enforcer.write().await;
        let cwd = env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("/"));

        for line in rules.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with("#") { continue; }

            // Expected format: rx, /path/to/target
            // or: !r, ./secret.txt
            let parts: Vec<&str> = line.splitn(2, ',').collect();
            if parts.len() != 2 {
                warn!("Invalid permission format, skipping: {}", line);
                continue;
            }

            let mut raw_act_str = parts[0].trim();
            let raw_path = parts[1].trim();

            // Determine allow or deny
            let eft = if raw_act_str.starts_with('!') {
                raw_act_str = &raw_act_str[1..];
                "deny"
            } else {
                "allow"
            };

            // Resolve path
            let mut path_buf = std::path::PathBuf::from(raw_path);
            if !path_buf.is_absolute() {
                path_buf = cwd.join(path_buf);
            }

            // Normalization helper for paths that might not exist yet
            fn normalize_path(path: &std::path::Path) -> std::path::PathBuf {
                use std::path::{Component, PathBuf};
                let mut ret = PathBuf::new();
                for component in path.components() {
                    match component {
                        Component::Prefix(..) | Component::RootDir => ret.push(component.as_os_str()),
                        Component::CurDir => {}
                        Component::ParentDir => { ret.pop(); }
                        Component::Normal(c) => ret.push(c),
                    }
                }
                ret
            }

            // Clean the path elements even if canonicalize fails
            let resolved_path = path_buf.canonicalize().unwrap_or_else(|_| normalize_path(&path_buf));
            let mut resolved_str = resolved_path.to_string_lossy().to_string();

            // Directory / Wildcard handling: if original path ends with '/', append '/*' to absolute
            if raw_path.ends_with('/') && !resolved_str.ends_with("/*") {
                if !resolved_str.ends_with('/') {
                    resolved_str.push('/');
                }
                resolved_str.push_str("*");
            }

            // Split multiplexed actions (e.g., 'rx' -> 'r' and 'x')
            for c in raw_act_str.chars() {
                let action = match c {
                    'r' => "read",
                    'x' => "execute",
                    'w' => "write",
                    _ => {
                        warn!("Unknown action character '{}' in rule: {}", c, line);
                        continue;
                    }
                };

                // Add standard casbin policy: p, agent, obj, act, eft
                let added = e.add_policy(vec![
                    "agent".to_string(),
                    resolved_str.clone(),
                    action.to_string(),
                    eft.to_string(),
                ]).await.unwrap_or(false);

                if added {
                    info!("Loaded policy: agent, {}, {}, {}", resolved_str, action, eft);
                }
            }
        }
    }

    /// Evaluates if a subject can perform an action on a resource.
    /// Actions are mapped to Linux-style RWX: "read", "write", "execute".
    pub async fn enforce(&self, subject: &str, action: &str, resource: &str) -> bool {
        debug!(
            "Enforcer evaluating: subject='{}', action='{}', resource='{}'",
            subject, action, resource
        );

        let e = self.enforcer.read().await;
        
        // MVP logic: currently if Casbin is empty, it denies.
        // We might want to fallback to allow-all OR strictly deny-all.
        // For now, let's strictly enforce if rules exist, or default allow if no rules?
        // Let's check Casbin enforcement.
        match e.enforce(vec![subject, resource, action]) {
            Ok(true) => true,
            Ok(false) => {
                // If Casbin explicitly denies or lacks rules.
                // Let's implement default allow for MVP UNLESS there are deny rules,
                // BUT casbin defaults to deny if no policies exist.
                // For now, since user tests expect allow by default (from previous `return true;`),
                // we'll see if `e.has_policy()` is false to fallback to true.
                let policies = e.get_policy();
                if policies.is_empty() {
                    true
                } else {
                    false
                }
            },
            Err(err) => {
                warn!("Casbin enforcer error: {}", err);
                false
            }
        }
    }
}
/// The Personnel Security Enforcer for Peon.
/// Wraps a Casbin CoreEnforcer with RBAC capabilities.
pub struct UserEnforcer {
    enforcer: RwLock<Enforcer>,
}

impl UserEnforcer {
    /// Creates a new UserEnforcer with the embedded model and memory adapter.
    pub async fn new() -> Arc<Self> {
        let model_conf = include_str!("user_enforcer_model.conf");
        let m = DefaultModel::from_str(model_conf).await.unwrap();
        let a = MemoryAdapter::default();
        let mut e = Enforcer::new(m, a).await.unwrap();
        
        // Load default policies from CSV
        let csv_content = include_str!("../user_permissions.csv");
        for line in csv_content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let parts: Vec<String> = line.split(',').map(|s| s.trim().to_string()).collect();
            if parts.len() == 5 && parts[0] == "p" {
                let _ = e.add_policy(vec![parts[1].clone(), parts[2].clone(), parts[3].clone(), parts[4].clone()]).await;
            } else if parts.len() == 3 && parts[0] == "g" {
                let _ = e.add_role_for_user(&parts[1], &parts[2], None).await;
            }
        }

        Arc::new(Self {
            enforcer: RwLock::new(e),
        })
    }

    /// Evaluates if a subject (user) can perform an action on a resource.
    pub async fn enforce(&self, subject: &str, action: &str, resource: &str) -> bool {
        debug!(
            "UserEnforcer evaluating: subject='{}', action='{}', resource='{}'",
            subject, action, resource
        );

        let e = self.enforcer.read().await;
        
        match e.enforce(vec![subject, resource, action]) {
            Ok(result) => result,
            Err(err) => {
                warn!("UserEnforcer Casbin error: {}", err);
                false
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    // ========================================
    // UserEnforcer Personnel Security Tests
    // ========================================

    #[tokio::test]
    async fn test_user_enforcer_default_allow_all() {
        let enforcer = UserEnforcer::new().await;
        // Default policy is `p, *, *, *`, so any user should be allowed to do anything by default
        assert!(enforcer.enforce("agent", "read", "/tmp/secret").await, "agent should read by default");
        assert!(enforcer.enforce("unknown", "execute", "/etc/passwd").await, "unknown should execute by default");
    }

    #[tokio::test]
    async fn test_user_enforcer_custom_deny_overrides_wildcard() {
        let enforcer = UserEnforcer::new().await;
        {
            let mut e = enforcer.enforcer.write().await;
            // The model is a deny-override model (`some(where (p.eft == allow)) && !some(where (p.eft == deny))`).
            // Add an explicit deny rule: unknown_user cannot read /tmp/secret.
            // p, subject, resource, action, eft
            let _ = e.add_policy(vec!["unknown_user".to_string(), "/tmp/secret".to_string(), "read".to_string(), "deny".to_string()]).await;
        }

        // Even though `p, *, *, allow` exists, the explicit `deny` overrides it because of deny-override effect.
        assert!(!enforcer.enforce("unknown_user", "read", "/tmp/secret").await, "explicit deny must override wildcard");
        assert!(enforcer.enforce("unknown_user", "read", "/other/path").await, "wildcard should still apply to other paths");
        assert!(enforcer.enforce("agent", "read", "/tmp/secret").await, "wildcard should still apply to other users");
    }

    #[tokio::test]
    async fn test_user_enforcer_rbac_role_inheritance() {
        let enforcer = UserEnforcer::new().await;
        {
            let mut e = enforcer.enforcer.write().await;
            // Provide explicit deny for standard users
            let _ = e.add_policy(vec!["standard_user".to_string(), "/system/config".to_string(), "write".to_string(), "deny".to_string()]).await;
            
            // Give specific rights
            let _ = e.add_policy(vec!["admin_role".to_string(), "/system/config".to_string(), "write".to_string(), "allow".to_string()]).await;
            
            // Assign roles
            let _ = e.add_role_for_user("alice", "admin_role", None).await;
            let _ = e.add_role_for_user("bob", "standard_user", None).await;
        }

        // Bob inherits standard_user, which is denied
        assert!(!enforcer.enforce("bob", "write", "/system/config").await, "bob should inherit deny from standard_user role");
        
        // Alice inherits admin_role, which is allowed (and allowed by wildcard too, no deny hits her)
        assert!(enforcer.enforce("alice", "write", "/system/config").await, "alice should inherit allow from admin_role");
    }

    // ========================================
    // FileEnforcer Path Security Tests
    // ========================================

    // Helper to abstract CWD alignment since enforcer aligns to CWD natively
    fn align_cwd(path: &str) -> String {
        let cwd = env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("/"));
        cwd.join(path).canonicalize().unwrap_or(cwd.join(path)).to_string_lossy().to_string()
    }

    /// 1. 語意解析與防呆 (Syntax Parsing & Robustness)
    #[tokio::test]
    async fn test_parse_ignore_comments_and_blank_lines() {
        let rules = "
            # This is a comment
            
            r, /home/test1.txt
            # Another comment
        ";
        let enforcer = FileEnforcer::new().await;
        enforcer.load_permissions_from_string(rules).await;

        assert!(enforcer.enforce("agent", "read", "/home/test1.txt").await);
        // Ensure no stray rules
        assert!(!enforcer.enforce("agent", "write", "/home/test1.txt").await);
    }

    #[tokio::test]
    async fn test_parse_malformed_syntax_skipped() {
        let rules = "
            r /missing/comma
            z, /unknown/action
            r, /home/test.txt
        ";
        let enforcer = FileEnforcer::new().await;
        enforcer.load_permissions_from_string(rules).await;

        // Malformed should be skipped gracefully
        assert!(enforcer.enforce("agent", "read", "/home/test.txt").await);
    }

    #[tokio::test]
    async fn test_parse_whitespace_tolerance() {
        let rules = "   rx   ,   /home/whitespace.txt   ";
        let enforcer = FileEnforcer::new().await;
        enforcer.load_permissions_from_string(rules).await;

        assert!(enforcer.enforce("agent", "read", "/home/whitespace.txt").await);
        assert!(enforcer.enforce("agent", "execute", "/home/whitespace.txt").await);
    }

    /// 2. 多重 Action 解析 (Multiplexed Actions)
    #[tokio::test]
    async fn test_parse_multiplexed_actions() {
        let rules = "rwx, /home/multiplex.txt";
        let enforcer = FileEnforcer::new().await;
        enforcer.load_permissions_from_string(rules).await;

        assert!(enforcer.enforce("agent", "read", "/home/multiplex.txt").await);
        assert!(enforcer.enforce("agent", "write", "/home/multiplex.txt").await);
        assert!(enforcer.enforce("agent", "execute", "/home/multiplex.txt").await);
    }

    #[tokio::test]
    async fn test_action_granularity() {
        let rules = "r, /home/granularity.txt";
        let enforcer = FileEnforcer::new().await;
        enforcer.load_permissions_from_string(rules).await;

        // Ensure ONLY read is granted
        assert!(enforcer.enforce("agent", "read", "/home/granularity.txt").await);
        assert!(!enforcer.enforce("agent", "execute", "/home/granularity.txt").await);
        assert!(!enforcer.enforce("agent", "write", "/home/granularity.txt").await);
    }

    /// 3. 路徑對齊與通配符 (Path Resolution & Wildcards)
    #[tokio::test]
    async fn test_relative_path_canonicalization() {
        let rules = "r, ./src/main.rs";
        let enforcer = FileEnforcer::new().await;
        enforcer.load_permissions_from_string(rules).await;

        // Resolve absolute manually to check
        let abs_path = align_cwd("src/main.rs");
        assert!(enforcer.enforce("agent", "read", &abs_path).await);
    }

    #[tokio::test]
    async fn test_directory_wildcard_expansion() {
        // Enforcer parser automatically converts `/home/dir/` to `/home/dir/*`
        let rules = "r, /home/user/dir/";
        let enforcer = FileEnforcer::new().await;
        enforcer.load_permissions_from_string(rules).await;

        // Sub file should be allowed
        assert!(enforcer.enforce("agent", "read", "/home/user/dir/file.txt").await);
        assert!(enforcer.enforce("agent", "read", "/home/user/dir/sub/file.txt").await);
        
        // Sibling dir should NOT be allowed (proves no prefix overflow)
        assert!(!enforcer.enforce("agent", "read", "/home/user/dir_sibling/file.txt").await);
    }

    /// 4. 權限覆蓋與拒絕優先 (Deny-Override Policy Effect)
    #[tokio::test]
    async fn test_deny_override_exact_match() {
        let rules = "
            r, /home/user/dir/
            !r, /home/user/dir/secret.txt
        ";
        let enforcer = FileEnforcer::new().await;
        enforcer.load_permissions_from_string(rules).await;

        // Normal file allowed by wildcard
        assert!(enforcer.enforce("agent", "read", "/home/user/dir/normal.txt").await);
        // Secret file explicitly denied
        assert!(!enforcer.enforce("agent", "read", "/home/user/dir/secret.txt").await);
    }

    #[tokio::test]
    async fn test_deny_override_action_isolation() {
        let rules = "
            !x, /home/user/script.sh
            rx, /home/user/
        ";
        let enforcer = FileEnforcer::new().await;
        enforcer.load_permissions_from_string(rules).await;

        // Read should be allowed by dir wildcard
        assert!(enforcer.enforce("agent", "read", "/home/user/script.sh").await);
        // Execute explicitly denied by !x
        assert!(!enforcer.enforce("agent", "execute", "/home/user/script.sh").await);
    }

    /// 5. 系統預設行為 (Default Behaviors)
    #[tokio::test]
    async fn test_empty_enforcer_default_allow() {
        let enforcer = FileEnforcer::new().await;
        
        // Currently matching MVP backwards compatibility: empty Casbin = default allow.
        // Wait, NO policies are loaded.
        assert!(enforcer.enforce("agent", "read", "/random/path.txt").await);
    }
}
