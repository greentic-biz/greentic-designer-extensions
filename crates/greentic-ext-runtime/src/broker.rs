//! Host broker — implemented in Phase 6 (Task 6.4).

pub type BrokerResult<T> = Result<T, BrokerError>;

#[derive(Debug, thiserror::Error)]
pub enum BrokerError {
    #[error("stub not yet implemented")]
    Stub,
}

pub struct Broker;
