use log::debug;
use std::sync::Arc;

/// The Security Enforcer for Peon.
/// Future integration point for Casbin.
pub struct FileEnforcer {
    // e: casbin::Enforcer,
}

impl FileEnforcer {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {})
    }

    /// Evaluates if a subject can perform an action on a resource.
    /// Actions are mapped to Linux-style RWX: "read", "write", "execute".
    pub async fn enforce(&self, subject: &str, action: &str, resource: &str) -> bool {
        debug!(
            "Enforcer: subject='{}', action='{}', resource='{}'",
            subject, action, resource
        );

        // TODO: Replace with Casbin policy evaluation
        // self.e.enforce((subject, action, resource)).await.unwrap_or(false)

        // MVP: Default allow all for now, but logs the exact RWX intent.
        true
    }
}
