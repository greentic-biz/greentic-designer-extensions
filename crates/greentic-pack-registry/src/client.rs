use crate::error::RegistryError;
use serde::Deserialize;

/// A parsed pack reference of the form `<full_name>@<version>`,
/// e.g. `greentic.dentist-template@1.2.0`.
///
/// `name` retains the full dotted publisher.name form because the
/// greentic-store-server routes treat that as a single path segment
/// (`/api/v1/packs/{name}/{version}`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackRef {
    /// Full dotted name like `greentic.dentist-template`.
    pub name: String,
    /// Semver version string like `1.2.0`.
    pub version: String,
}

impl PackRef {
    /// Parse a string of the form `<full_name>@<version>`.
    pub fn parse(s: &str) -> Result<Self, RegistryError> {
        let (name, version) = s
            .split_once('@')
            .ok_or_else(|| RegistryError::BadRef("missing @version".into()))?;
        if name.is_empty() || version.is_empty() {
            return Err(RegistryError::BadRef("empty segment".into()));
        }
        Ok(PackRef {
            name: name.to_string(),
            version: version.to_string(),
        })
    }
}

/// Manifest metadata returned by the store server alongside the artifact.
#[derive(Debug, Clone, Deserialize)]
pub struct PackVersionMetadata {
    #[serde(rename = "manifest")]
    pub manifest: serde_json::Value,
    #[serde(rename = "artifactSha256")]
    pub artifact_sha256: String,
    pub yanked: bool,
}

#[async_trait::async_trait]
pub trait PackRegistryClient: Send + Sync {
    /// Fetch the metadata + signature info for a specific pack version.
    async fn fetch_metadata(
        &self,
        pack_ref: &PackRef,
    ) -> Result<PackVersionMetadata, RegistryError>;

    /// Fetch the raw `.gtpack` artifact bytes.
    async fn fetch_artifact(&self, pack_ref: &PackRef) -> Result<Vec<u8>, RegistryError>;
}

/// HTTP client backed by a greentic-store-server base URL.
#[derive(Debug, Clone)]
pub struct StoreServerClient {
    base_url: String,
    http: reqwest::Client,
}

impl StoreServerClient {
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into().trim_end_matches('/').to_string(),
            http: reqwest::Client::new(),
        }
    }

    /// Build the full URL `<base>/api/v1/packs/<name>/<version><suffix>`.
    fn endpoint(&self, pack_ref: &PackRef, suffix: &str) -> String {
        format!(
            "{}/api/v1/packs/{}/{}{}",
            self.base_url, pack_ref.name, pack_ref.version, suffix
        )
    }
}

#[async_trait::async_trait]
impl PackRegistryClient for StoreServerClient {
    async fn fetch_metadata(
        &self,
        pack_ref: &PackRef,
    ) -> Result<PackVersionMetadata, RegistryError> {
        let url = self.endpoint(pack_ref, "");
        let resp = self.http.get(&url).send().await?;
        let status = resp.status();
        if !status.is_success() {
            return Err(RegistryError::Status(status.as_u16()));
        }
        let metadata = resp.json::<PackVersionMetadata>().await?;
        Ok(metadata)
    }

    async fn fetch_artifact(&self, pack_ref: &PackRef) -> Result<Vec<u8>, RegistryError> {
        let url = self.endpoint(pack_ref, "/artifact");
        let resp = self.http.get(&url).send().await?;
        let status = resp.status();
        if !status.is_success() {
            return Err(RegistryError::Status(status.as_u16()));
        }
        Ok(resp.bytes().await?.to_vec())
    }
}
