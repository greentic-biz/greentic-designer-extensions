use async_trait::async_trait;
use oci_client::client::ClientConfig;
use oci_client::secrets::RegistryAuth;
use oci_client::{Client, Reference};

use crate::error::RegistryError;
use crate::registry::ExtensionRegistry;
use crate::types::{
    AuthToken, ExtensionArtifact, ExtensionMetadata, ExtensionSummary, SearchQuery,
};

pub struct OciRegistry {
    name: String,
    registry_host: String,
    namespace: String,
    auth: RegistryAuth,
    client: Client,
}

impl OciRegistry {
    pub fn new(
        name: impl Into<String>,
        registry_host: impl Into<String>,
        namespace: impl Into<String>,
        auth: Option<(String, String)>,
    ) -> Self {
        let client = Client::new(ClientConfig::default());
        Self {
            name: name.into(),
            registry_host: registry_host.into(),
            namespace: namespace.into(),
            auth: auth
                .map(|(u, p)| RegistryAuth::Basic(u, p))
                .unwrap_or(RegistryAuth::Anonymous),
            client,
        }
    }

    fn reference(&self, name: &str, version: &str) -> Reference {
        format!("{}/{}/{name}:{version}", self.registry_host, self.namespace)
            .parse()
            .expect("valid reference")
    }
}

#[async_trait]
impl ExtensionRegistry for OciRegistry {
    fn name(&self) -> &str {
        &self.name
    }

    async fn search(&self, _query: SearchQuery) -> Result<Vec<ExtensionSummary>, RegistryError> {
        Ok(Vec::new())
    }

    async fn metadata(
        &self,
        _name: &str,
        _version: &str,
    ) -> Result<ExtensionMetadata, RegistryError> {
        Err(RegistryError::Storage(
            "OCI metadata introspection not yet implemented; use fetch() to obtain describe.json"
                .into(),
        ))
    }

    async fn fetch(&self, name: &str, version: &str) -> Result<ExtensionArtifact, RegistryError> {
        let reference = self.reference(name, version);
        let pulled = self
            .client
            .pull(
                &reference,
                &self.auth,
                vec!["application/vnd.greentic.extension.v1+zip"],
            )
            .await
            .map_err(|e| RegistryError::Oci(e.to_string()))?;

        let first_layer = pulled
            .layers
            .into_iter()
            .next()
            .ok_or_else(|| RegistryError::Storage("no layers in manifest".into()))?;

        let bytes = first_layer.data;

        let describe = {
            let cursor = std::io::Cursor::new(&bytes);
            let mut archive = zip::ZipArchive::new(cursor)
                .map_err(|e| RegistryError::Storage(format!("zip open: {e}")))?;
            let mut describe_entry = archive
                .by_name("describe.json")
                .map_err(|e| RegistryError::Storage(format!("describe missing: {e}")))?;
            let value: serde_json::Value = serde_json::from_reader(&mut describe_entry)?;
            greentic_ext_contract::schema::validate_describe_json(&value)?;
            serde_json::from_value::<greentic_ext_contract::DescribeJson>(value)?
        };

        Ok(ExtensionArtifact {
            name: describe.metadata.id.clone(),
            version: describe.metadata.version.clone(),
            describe,
            bytes,
            signature: None,
        })
    }

    async fn list_versions(&self, _name: &str) -> Result<Vec<String>, RegistryError> {
        // Real implementation would call client.list_tags — which requires an
        // authenticated, reachable registry. For Plan 2 we ship an empty-list
        // stub to keep the trait total.
        Ok(Vec::new())
    }

    async fn publish(
        &self,
        _artifact: ExtensionArtifact,
        _auth: &AuthToken,
    ) -> Result<(), RegistryError> {
        Err(RegistryError::Storage(
            "OCI publish requires external `oras push`; gtdx publish covers Greentic Store only"
                .into(),
        ))
    }
}
