#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExtensionHealth {
    Healthy,
    Degraded(HealthReason),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HealthReason {
    MissingRequiredCap(String),
    SignatureInvalid,
    LoadFailed(String),
    CycleDetected,
}

impl ExtensionHealth {
    #[must_use]
    pub const fn is_healthy(&self) -> bool {
        matches!(self, Self::Healthy)
    }
}
