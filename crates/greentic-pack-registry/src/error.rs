#[derive(Debug, thiserror::Error)]
pub enum RegistryError {
    #[error("bad pack ref: {0}")]
    BadRef(String),
    #[error("http: {0}")]
    Http(#[from] reqwest::Error),
    #[error("status code {0}")]
    Status(u16),
}
