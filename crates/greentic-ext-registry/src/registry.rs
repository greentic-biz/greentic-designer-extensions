use async_trait::async_trait;

use crate::error::RegistryError;
use crate::types::{
    AuthToken, ExtensionArtifact, ExtensionMetadata, ExtensionSummary, SearchQuery,
};

#[async_trait]
pub trait ExtensionRegistry: Send + Sync {
    fn name(&self) -> &str;

    async fn search(&self, query: SearchQuery) -> Result<Vec<ExtensionSummary>, RegistryError>;

    async fn metadata(
        &self,
        name: &str,
        version: &str,
    ) -> Result<ExtensionMetadata, RegistryError>;

    async fn fetch(
        &self,
        name: &str,
        version: &str,
    ) -> Result<ExtensionArtifact, RegistryError>;

    async fn publish(
        &self,
        artifact: ExtensionArtifact,
        auth: &AuthToken,
    ) -> Result<(), RegistryError> {
        let _ = (artifact, auth);
        Err(RegistryError::Storage(
            "publish not supported by this registry".into(),
        ))
    }

    async fn list_versions(&self, name: &str) -> Result<Vec<String>, RegistryError>;
}
