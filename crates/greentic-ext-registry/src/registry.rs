use async_trait::async_trait;

use crate::error::RegistryError;
use crate::types::{
    AuthToken, ExtensionArtifact, ExtensionMetadata, ExtensionSummary, SearchQuery,
};
use greentic_ext_contract::ExtensionKind;

#[async_trait]
pub trait ExtensionRegistry: Send + Sync {
    fn name(&self) -> &str;

    async fn search(&self, query: SearchQuery) -> Result<Vec<ExtensionSummary>, RegistryError>;

    async fn metadata(&self, name: &str, version: &str)
    -> Result<ExtensionMetadata, RegistryError>;

    async fn fetch(&self, name: &str, version: &str) -> Result<ExtensionArtifact, RegistryError>;

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

    async fn list_by_kind(
        &self,
        kind: ExtensionKind,
    ) -> Result<Vec<ExtensionSummary>, RegistryError> {
        let all = self.search(SearchQuery::default()).await?;
        Ok(all.into_iter().filter(|s| s.kind == kind).collect())
    }

    async fn get_describe(
        &self,
        name: &str,
        version: &str,
    ) -> Result<greentic_ext_contract::DescribeJson, RegistryError> {
        let metadata = self.metadata(name, version).await?;
        Ok(metadata.describe)
    }
}
