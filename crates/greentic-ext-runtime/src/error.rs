use thiserror::Error;

#[derive(Debug, Error)]
pub enum RuntimeError {
    #[error("extension '{0}' already loaded")]
    AlreadyLoaded(String),

    #[error("extension '{0}' not found")]
    NotFound(String),

    #[error(
        "signature verification failed for extension '{extension_id}': {reason}\n\
         hint: reinstall a signed extension, or set GREENTIC_EXT_ALLOW_UNSIGNED=1 for dev"
    )]
    SignatureInvalid {
        extension_id: String,
        reason: String,
    },

    #[error("contract error: {0}")]
    Contract(#[from] greentic_ext_contract::ContractError),

    #[error("wasmtime: {0}")]
    Wasmtime(#[from] anyhow::Error),

    #[error("io: {0}")]
    Io(#[from] std::io::Error),

    #[error("json: {0}")]
    Json(#[from] serde_json::Error),

    #[error("watcher: {0}")]
    Watcher(String),

    #[error("permission denied: {0}")]
    PermissionDenied(String),
}
