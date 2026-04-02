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
        let model_conf = include_str!("model.conf");
        let m = DefaultModel::from_str(model_conf).await.unwrap();
        let a = MemoryAdapter::default();
        let e = Enforcer::new(m, a).await.unwrap();
        
        Arc::new(Self {
            enforcer: RwLock::new(e),
        })
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

            // Canonicalize if possible to align with scanner, else use raw absolute
            let resolved_path = path_buf.canonicalize().unwrap_or(path_buf);
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
