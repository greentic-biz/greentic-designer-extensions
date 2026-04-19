use async_trait::async_trait;

use crate::error::RegistryError;
use crate::types::{ExtensionArtifact, ExtensionMetadata, ExtensionSummary, SearchQuery};

#[async_trait]
pub trait ExtensionRegistry: Send + Sync {
    fn name(&self) -> &str;

    async fn search(&self, query: SearchQuery) -> Result<Vec<ExtensionSummary>, RegistryError>;

    async fn metadata(&self, name: &str, version: &str)
    -> Result<ExtensionMetadata, RegistryError>;

    async fn fetch(&self, name: &str, version: &str) -> Result<ExtensionArtifact, RegistryError>;

    async fn publish(
        &self,
        req: crate::publish::PublishRequest,
    ) -> Result<crate::publish::PublishReceipt, RegistryError> {
        let _ = req;
        Err(RegistryError::NotImplemented {
            hint: format!("publish not supported for registry '{}'", self.name()),
        })
    }

    async fn list_versions(&self, name: &str) -> Result<Vec<String>, RegistryError>;
}
