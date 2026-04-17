use greentic_ext_contract::ExtensionKind;

pub type BrokerResult<T> = Result<T, BrokerError>;

#[derive(Debug, thiserror::Error)]
pub enum BrokerError {
    #[error("permission denied: {0}")]
    PermissionDenied(String),

    #[error("target extension not loaded: {0}")]
    TargetNotLoaded(String),

    #[error("function not found: {0}")]
    FunctionNotFound(String),

    #[error("max call depth exceeded")]
    MaxDepthExceeded,

    #[error("deadline exceeded")]
    Deadline,
}

pub const MAX_DEPTH: u32 = 8;

#[derive(Debug, Default)]
pub struct Broker;

impl Broker {
    #[must_use]
    pub const fn new() -> Self {
        Self
    }

    pub fn check_permission(
        &self,
        caller_id: &str,
        allowlist: &[String],
        target_kind: ExtensionKind,
    ) -> BrokerResult<()> {
        if allowlist.iter().any(|k| k == target_kind.dir_name()) {
            Ok(())
        } else {
            Err(BrokerError::PermissionDenied(format!(
                "{caller_id} may not call {:?} extensions",
                target_kind
            )))
        }
    }

    pub fn check_depth(&self, depth: u32) -> BrokerResult<()> {
        if depth >= MAX_DEPTH {
            Err(BrokerError::MaxDepthExceeded)
        } else {
            Ok(())
        }
    }
}
