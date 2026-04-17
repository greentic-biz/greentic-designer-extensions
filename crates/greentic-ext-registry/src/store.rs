use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};

use crate::error::RegistryError;
use crate::registry::ExtensionRegistry;
use crate::types::{
    AuthToken, ExtensionArtifact, ExtensionMetadata, ExtensionSummary, SearchQuery,
};

pub struct GreenticStoreRegistry {
    name: String,
    base_url: String,
    token: Option<String>,
    client: Client,
}

impl GreenticStoreRegistry {
    pub fn new(
        name: impl Into<String>,
        base_url: impl Into<String>,
        token: Option<String>,
    ) -> Self {
        Self {
            name: name.into(),
            base_url: base_url.into(),
            token,
            client: Client::builder()
                .user_agent(concat!("gtdx/", env!("CARGO_PKG_VERSION")))
                .build()
                .expect("reqwest client"),
        }
    }

    fn url(&self, path: &str) -> String {
        format!("{}{path}", self.base_url.trim_end_matches('/'))
    }

    fn with_auth(&self, req: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        if let Some(token) = &self.token {
            req.bearer_auth(token)
        } else {
            req
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SummaryDto {
    name: String,
    latest_version: String,
    kind: greentic_ext_contract::ExtensionKind,
    summary: String,
    #[serde(default)]
    downloads: u64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MetadataDto {
    describe: greentic_ext_contract::DescribeJson,
    artifact_sha256: String,
    #[serde(default)]
    published_at: String,
    #[serde(default)]
    yanked: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct PublishRequest<'a> {
    describe: &'a greentic_ext_contract::DescribeJson,
    signature: Option<&'a str>,
    artifact_sha256: String,
}

#[async_trait]
impl ExtensionRegistry for GreenticStoreRegistry {
    fn name(&self) -> &str {
        &self.name
    }

    async fn search(&self, query: SearchQuery) -> Result<Vec<ExtensionSummary>, RegistryError> {
        let mut req = self.client.get(self.url("/api/v1/extensions"));
        if let Some(k) = query.kind {
            req = req.query(&[("kind", k.dir_name())]);
        }
        if let Some(cap) = &query.capability {
            req = req.query(&[("capability", cap.as_str())]);
        }
        if let Some(q) = &query.query {
            req = req.query(&[("q", q.as_str())]);
        }
        req = req.query(&[("page", query.page), ("limit", query.limit)]);

        let resp = self.with_auth(req).send().await?.error_for_status()?;
        let dtos: Vec<SummaryDto> = resp.json().await?;
        Ok(dtos
            .into_iter()
            .map(|d| ExtensionSummary {
                name: d.name,
                latest_version: d.latest_version,
                kind: d.kind,
                summary: d.summary,
                downloads: d.downloads,
            })
            .collect())
    }

    async fn metadata(
        &self,
        name: &str,
        version: &str,
    ) -> Result<ExtensionMetadata, RegistryError> {
        let resp = self
            .with_auth(
                self.client
                    .get(self.url(&format!("/api/v1/extensions/{name}/{version}"))),
            )
            .send()
            .await?;
        if resp.status() == reqwest::StatusCode::NOT_FOUND {
            return Err(RegistryError::NotFound {
                name: name.into(),
                version: version.into(),
            });
        }
        let dto: MetadataDto = resp.error_for_status()?.json().await?;
        Ok(ExtensionMetadata {
            name: dto.describe.metadata.id.clone(),
            version: dto.describe.metadata.version.clone(),
            describe: dto.describe,
            artifact_sha256: dto.artifact_sha256,
            published_at: dto.published_at,
            yanked: dto.yanked,
        })
    }

    async fn fetch(&self, name: &str, version: &str) -> Result<ExtensionArtifact, RegistryError> {
        let metadata = self.metadata(name, version).await?;
        let bytes = self
            .with_auth(
                self.client
                    .get(self.url(&format!("/api/v1/extensions/{name}/{version}/artifact"))),
            )
            .send()
            .await?
            .error_for_status()?
            .bytes()
            .await?
            .to_vec();
        Ok(ExtensionArtifact {
            name: metadata.name,
            version: metadata.version,
            describe: metadata.describe,
            bytes,
            signature: None,
        })
    }

    async fn publish(
        &self,
        artifact: ExtensionArtifact,
        auth: &AuthToken,
    ) -> Result<(), RegistryError> {
        let sha = greentic_ext_contract::artifact_sha256(&artifact.bytes);
        let body = PublishRequest {
            describe: &artifact.describe,
            signature: artifact.signature.as_deref(),
            artifact_sha256: sha,
        };

        let form = reqwest::multipart::Form::new()
            .text("metadata", serde_json::to_string(&body)?)
            .part(
                "artifact",
                reqwest::multipart::Part::bytes(artifact.bytes)
                    .file_name("artifact.gtxpack")
                    .mime_str("application/zip")
                    .map_err(|e| RegistryError::Storage(format!("mime: {e}")))?,
            );

        self.client
            .post(self.url("/api/v1/extensions"))
            .bearer_auth(&auth.token)
            .multipart(form)
            .send()
            .await?
            .error_for_status()?;
        Ok(())
    }

    async fn list_versions(&self, name: &str) -> Result<Vec<String>, RegistryError> {
        #[derive(Deserialize)]
        struct Dto {
            versions: Vec<String>,
        }
        let resp = self
            .with_auth(
                self.client
                    .get(self.url(&format!("/api/v1/extensions/{name}"))),
            )
            .send()
            .await?;
        if resp.status() == reqwest::StatusCode::NOT_FOUND {
            return Ok(Vec::new());
        }
        let dto: Dto = resp.error_for_status()?.json().await?;
        Ok(dto.versions)
    }
}
