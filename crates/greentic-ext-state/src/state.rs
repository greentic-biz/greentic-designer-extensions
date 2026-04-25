use std::path::Path;

#[derive(Debug, Default)]
pub struct ExtensionState;

impl ExtensionState {
    pub fn load(_home: &Path) -> Result<Self, crate::StateError> {
        Ok(Self)
    }
}
