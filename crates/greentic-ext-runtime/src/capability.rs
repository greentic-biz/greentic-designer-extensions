//! Capability registry — implemented in Phase 6 (Task 6.1).

use std::collections::HashMap;

use greentic_ext_contract::{CapabilityId, CapabilityRef, ExtensionKind};
use semver::Version;

#[derive(Debug, Clone)]
pub struct OfferedBinding {
    pub extension_id: String,
    pub cap_id: CapabilityId,
    pub version: Version,
    pub kind: ExtensionKind,
    pub export_path: String,
}

#[derive(Debug, Clone, Default)]
pub struct ResolutionPlan {
    pub consumer: String,
    pub resolved: HashMap<CapabilityId, OfferedBinding>,
    pub unresolved: Vec<CapabilityRef>,
}

#[derive(Debug, Default)]
pub struct CapabilityRegistry {
    #[allow(dead_code)]
    offerings: HashMap<CapabilityId, Vec<OfferedBinding>>,
}

impl CapabilityRegistry {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}
