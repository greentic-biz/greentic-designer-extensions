# Designer Extension — CLI + Registry Implementation Plan (2 of 4)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax.

**Goal:** Full `gtdx` CLI with install/update/uninstall lifecycle backed by 3 registry implementations (Local filesystem, Greentic Store HTTP, OCI). End state: `gtdx install <name>@<version>` works against all three sources; `gtdx list`, `gtdx doctor`, `gtdx validate`, `gtdx publish` all functional.

**Architecture:** New crate `greentic-ext-registry` adds `ExtensionRegistry` trait + three implementations + install/update/uninstall lifecycle against a storage layout rooted at `~/.greentic/extensions/`. CLI crate expands from 3-command stub to ~20 subcommands delegating to the registry crate. Credentials stored at `~/.greentic/credentials.toml`, config at `~/.greentic/config.toml`.

**Tech Stack (new):** reqwest (HTTP client), oci-distribution or oci-client (OCI client), mime, zip (reuse testing crate helpers), dialoguer or inquire (permission prompts), keyring (optional credential storage).

**Source spec:** `docs/superpowers/specs/2026-04-17-designer-extension-system-design.md` sections 8 + 9.

**Prerequisites:** Plan 1 (`v0.1.0-foundation` tag) shipped. Branches off `feat/foundation` or rebased onto `main` after Plan 1 merges.

---

## Phase A — Registry trait + base types

### Task A.1: Scaffold `greentic-ext-registry` crate

**Files:**
- Create: `crates/greentic-ext-registry/Cargo.toml`
- Create: `crates/greentic-ext-registry/src/lib.rs`
- Modify: root `Cargo.toml` (add member, add workspace deps `reqwest`, `oci-client`, `dialoguer`)

- [ ] **Step 1: Add workspace deps to root `Cargo.toml`**

```toml
reqwest = { version = "0.12", default-features = false, features = ["rustls-tls", "json", "stream"] }
oci-client = { version = "0.14", default-features = false, features = ["rustls-tls"] }
dialoguer = "0.11"
zip = { version = "2", default-features = false, features = ["deflate"] }
```

- [ ] **Step 2: Add `"crates/greentic-ext-registry"` to workspace `members`**

- [ ] **Step 3: Create `crates/greentic-ext-registry/Cargo.toml`**

```toml
[package]
name = "greentic-ext-registry"
version.workspace = true
edition.workspace = true
license.workspace = true
repository.workspace = true
rust-version.workspace = true
description = "Registry client + install lifecycle for Greentic Designer Extensions"
publish = false

[dependencies]
anyhow = { workspace = true }
async-trait = "0.1"
dialoguer = { workspace = true }
greentic-ext-contract = { path = "../greentic-ext-contract" }
oci-client = { workspace = true }
reqwest = { workspace = true }
semver = { workspace = true }
serde = { workspace = true }
serde_json = { workspace = true }
sha2 = { workspace = true }
tempfile = { workspace = true }
thiserror = { workspace = true }
tokio = { workspace = true }
toml = { workspace = true }
tracing = { workspace = true }
zip = { workspace = true }

[dev-dependencies]
greentic-ext-testing = { path = "../greentic-ext-testing" }
wiremock = "0.6"

[lints]
workspace = true
```

- [ ] **Step 4: Add `async-trait` to workspace deps** (add to root `Cargo.toml`)

```toml
async-trait = "0.1"
```

- [ ] **Step 5: Create `src/lib.rs`**

```rust
//! Registry client + install lifecycle for Greentic Designer Extensions.

pub mod error;
pub mod registry;
pub mod types;
pub mod local;
pub mod store;
pub mod oci;
pub mod storage;
pub mod lifecycle;
pub mod credentials;
pub mod config;
pub mod prompt;

pub use self::error::RegistryError;
pub use self::registry::ExtensionRegistry;
pub use self::types::{
    ArtifactBytes, AuthToken, ExtensionArtifact, ExtensionMetadata,
    ExtensionSummary, SearchQuery,
};
```

- [ ] **Step 6: Commit**

```bash
git add Cargo.toml crates/greentic-ext-registry/
git commit -m "feat(registry): scaffold registry crate"
```

### Task A.2: Base types module

**Files:** `crates/greentic-ext-registry/src/types.rs`, `src/error.rs`

- [ ] **Step 1: Write `src/error.rs`**

```rust
use thiserror::Error;

#[derive(Debug, Error)]
pub enum RegistryError {
    #[error("extension not found: {name}@{version}")]
    NotFound { name: String, version: String },

    #[error("signature verification failed: {0}")]
    SignatureInvalid(String),

    #[error("auth required for {0}")]
    AuthRequired(String),

    #[error("auth failed: {0}")]
    AuthFailed(String),

    #[error("incompatible engine version: requires {required}, host provides {host}")]
    IncompatibleEngine { required: String, host: String },

    #[error("storage: {0}")]
    Storage(String),

    #[error("http: {0}")]
    Http(#[from] reqwest::Error),

    #[error("oci: {0}")]
    Oci(String),

    #[error("io: {0}")]
    Io(#[from] std::io::Error),

    #[error("json: {0}")]
    Json(#[from] serde_json::Error),

    #[error("toml: {0}")]
    Toml(#[from] toml::de::Error),

    #[error("contract: {0}")]
    Contract(#[from] greentic_ext_contract::ContractError),
}
```

- [ ] **Step 2: Write `src/types.rs`**

```rust
use greentic_ext_contract::{DescribeJson, ExtensionKind};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchQuery {
    pub kind: Option<ExtensionKind>,
    pub capability: Option<String>,
    pub query: Option<String>,
    pub page: u32,
    pub limit: u32,
}

impl Default for SearchQuery {
    fn default() -> Self {
        Self {
            kind: None,
            capability: None,
            query: None,
            page: 1,
            limit: 20,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtensionSummary {
    pub name: String,
    pub latest_version: String,
    pub kind: ExtensionKind,
    pub summary: String,
    pub downloads: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtensionMetadata {
    pub name: String,
    pub version: String,
    pub describe: DescribeJson,
    pub artifact_sha256: String,
    pub published_at: String,
    pub yanked: bool,
}

pub type ArtifactBytes = Vec<u8>;

#[derive(Debug, Clone)]
pub struct ExtensionArtifact {
    pub name: String,
    pub version: String,
    pub describe: DescribeJson,
    pub bytes: ArtifactBytes,
    pub signature: Option<String>,
}

#[derive(Debug, Clone)]
pub struct AuthToken {
    pub registry: String,
    pub token: String,
}
```

- [ ] **Step 3: Compile**

Run: `cargo check -p greentic-ext-registry`
Expected: missing module files errors (OK) — other modules land in subsequent tasks. Add empty placeholder files first if needed:

```bash
for f in registry local store oci storage lifecycle credentials config prompt; do
  touch "crates/greentic-ext-registry/src/$f.rs"
done
```

Now: `cargo check -p greentic-ext-registry` — should compile (warnings for unused empty modules).

- [ ] **Step 4: Commit**

```bash
git add crates/greentic-ext-registry/src/
git commit -m "feat(registry): add error + types modules"
```

### Task A.3: ExtensionRegistry trait

**Files:** `src/registry.rs`

- [ ] **Step 1: Write `src/registry.rs`**

```rust
use async_trait::async_trait;

use crate::error::RegistryError;
use crate::types::{
    ArtifactBytes, AuthToken, ExtensionArtifact, ExtensionMetadata,
    ExtensionSummary, SearchQuery,
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
        Err(RegistryError::Storage("publish not supported by this registry".into()))
    }

    async fn list_versions(&self, name: &str) -> Result<Vec<String>, RegistryError>;
}
```

- [ ] **Step 2: Commit**

```bash
git add crates/greentic-ext-registry/src/registry.rs
git commit -m "feat(registry): add ExtensionRegistry trait"
```

---

## Phase B — LocalFilesystemRegistry

### Task B.1: Local registry implementation

**Files:** `src/local.rs`, `tests/local_registry.rs`

- [ ] **Step 1: Write failing test `tests/local_registry.rs`**

```rust
use greentic_ext_contract::ExtensionKind;
use greentic_ext_registry::local::LocalFilesystemRegistry;
use greentic_ext_registry::{ExtensionRegistry, SearchQuery};
use greentic_ext_testing::{pack_directory, ExtensionFixtureBuilder};
use tempfile::TempDir;

#[tokio::test]
async fn local_registry_finds_and_fetches_packed_extension() {
    let tmp = TempDir::new().unwrap();
    let reg_root = tmp.path().to_path_buf();

    // Create a .gtxpack artifact and place it into the registry root
    let fixture = ExtensionFixtureBuilder::new(
        ExtensionKind::Design,
        "greentic.local-demo",
        "0.1.0",
    )
    .offer("greentic:demo/hi", "1.0.0")
    .with_wasm(b"not-a-real-wasm".to_vec())
    .build()
    .unwrap();
    let pack_path = reg_root.join("greentic.local-demo-0.1.0.gtxpack");
    pack_directory(fixture.root(), &pack_path).unwrap();

    let reg = LocalFilesystemRegistry::new("local", reg_root);

    // search
    let results = reg.search(SearchQuery::default()).await.unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].name, "greentic.local-demo");

    // fetch
    let art = reg.fetch("greentic.local-demo", "0.1.0").await.unwrap();
    assert_eq!(art.version, "0.1.0");
    assert!(!art.bytes.is_empty());

    // list_versions
    let versions = reg.list_versions("greentic.local-demo").await.unwrap();
    assert_eq!(versions, vec!["0.1.0"]);
}
```

- [ ] **Step 2: Write `src/local.rs`**

```rust
use std::path::{Path, PathBuf};

use async_trait::async_trait;
use greentic_ext_contract::DescribeJson;
use serde_json::Value;

use crate::error::RegistryError;
use crate::registry::ExtensionRegistry;
use crate::types::{
    ArtifactBytes, AuthToken, ExtensionArtifact, ExtensionMetadata,
    ExtensionSummary, SearchQuery,
};

pub struct LocalFilesystemRegistry {
    name: String,
    root: PathBuf,
}

impl LocalFilesystemRegistry {
    pub fn new(name: impl Into<String>, root: impl Into<PathBuf>) -> Self {
        Self {
            name: name.into(),
            root: root.into(),
        }
    }

    fn parse_pack_filename(filename: &str) -> Option<(String, String)> {
        let stem = filename.strip_suffix(".gtxpack")?;
        let idx = stem.rfind('-')?;
        let (name, version) = stem.split_at(idx);
        let version = version.strip_prefix('-')?.to_string();
        if !name.is_empty() && !version.is_empty() {
            Some((name.to_string(), version))
        } else {
            None
        }
    }

    fn pack_path(&self, name: &str, version: &str) -> PathBuf {
        self.root.join(format!("{name}-{version}.gtxpack"))
    }

    fn read_describe_from_pack(pack_path: &Path) -> Result<DescribeJson, RegistryError> {
        let file = std::fs::File::open(pack_path)?;
        let mut archive = zip::ZipArchive::new(file)
            .map_err(|e| RegistryError::Storage(format!("zip open: {e}")))?;
        let mut describe_entry = archive
            .by_name("describe.json")
            .map_err(|e| RegistryError::Storage(format!("describe.json missing: {e}")))?;
        let value: Value = serde_json::from_reader(&mut describe_entry)?;
        greentic_ext_contract::schema::validate_describe_json(&value)?;
        let describe: DescribeJson = serde_json::from_value(value)?;
        Ok(describe)
    }

    fn read_artifact_bytes(pack_path: &Path) -> Result<ArtifactBytes, RegistryError> {
        Ok(std::fs::read(pack_path)?)
    }

    fn list_packs(&self) -> std::io::Result<Vec<(String, String, PathBuf)>> {
        if !self.root.exists() {
            return Ok(Vec::new());
        }
        let mut out = Vec::new();
        for entry in std::fs::read_dir(&self.root)? {
            let entry = entry?;
            if !entry.file_type()?.is_file() {
                continue;
            }
            let filename = entry.file_name();
            let filename_str = filename.to_string_lossy();
            if let Some((n, v)) = Self::parse_pack_filename(&filename_str) {
                out.push((n, v, entry.path()));
            }
        }
        Ok(out)
    }
}

#[async_trait]
impl ExtensionRegistry for LocalFilesystemRegistry {
    fn name(&self) -> &str {
        &self.name
    }

    async fn search(&self, query: SearchQuery) -> Result<Vec<ExtensionSummary>, RegistryError> {
        let mut summaries = Vec::new();
        for (name, version, path) in self.list_packs()? {
            if let Some(q) = &query.query {
                if !name.contains(q.as_str()) {
                    continue;
                }
            }
            match Self::read_describe_from_pack(&path) {
                Ok(d) => {
                    if let Some(k) = query.kind {
                        if d.kind != k {
                            continue;
                        }
                    }
                    summaries.push(ExtensionSummary {
                        name: d.metadata.id,
                        latest_version: version,
                        kind: d.kind,
                        summary: d.metadata.summary,
                        downloads: 0,
                    });
                }
                Err(e) => {
                    tracing::warn!(path = %path.display(), error = %e, "skipping invalid pack");
                }
            }
        }
        summaries.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(summaries.into_iter().take(query.limit as usize).collect())
    }

    async fn metadata(
        &self,
        name: &str,
        version: &str,
    ) -> Result<ExtensionMetadata, RegistryError> {
        let path = self.pack_path(name, version);
        if !path.exists() {
            return Err(RegistryError::NotFound {
                name: name.into(),
                version: version.into(),
            });
        }
        let describe = Self::read_describe_from_pack(&path)?;
        let bytes = Self::read_artifact_bytes(&path)?;
        let sha = greentic_ext_contract::artifact_sha256(&bytes);
        Ok(ExtensionMetadata {
            name: describe.metadata.id.clone(),
            version: describe.metadata.version.clone(),
            describe,
            artifact_sha256: sha,
            published_at: String::new(),
            yanked: false,
        })
    }

    async fn fetch(
        &self,
        name: &str,
        version: &str,
    ) -> Result<ExtensionArtifact, RegistryError> {
        let path = self.pack_path(name, version);
        if !path.exists() {
            return Err(RegistryError::NotFound {
                name: name.into(),
                version: version.into(),
            });
        }
        let describe = Self::read_describe_from_pack(&path)?;
        let bytes = Self::read_artifact_bytes(&path)?;
        Ok(ExtensionArtifact {
            name: describe.metadata.id.clone(),
            version: describe.metadata.version.clone(),
            describe,
            bytes,
            signature: None,
        })
    }

    async fn publish(
        &self,
        _artifact: ExtensionArtifact,
        _auth: &AuthToken,
    ) -> Result<(), RegistryError> {
        Err(RegistryError::Storage(
            "local registry does not support publish; use `cp` directly".into(),
        ))
    }

    async fn list_versions(&self, name: &str) -> Result<Vec<String>, RegistryError> {
        let mut versions: Vec<String> = self
            .list_packs()?
            .into_iter()
            .filter(|(n, _, _)| n == name)
            .map(|(_, v, _)| v)
            .collect();
        versions.sort();
        Ok(versions)
    }
}
```

- [ ] **Step 3: Run test**

Run: `cargo test -p greentic-ext-registry --test local_registry`
Expected: 1 passed

- [ ] **Step 4: Commit**

```bash
git add crates/greentic-ext-registry/src/local.rs crates/greentic-ext-registry/tests/local_registry.rs
git commit -m "feat(registry): add LocalFilesystemRegistry with .gtxpack scan"
```

---

## Phase C — GreenticStoreRegistry (HTTP)

### Task C.1: Store HTTP client with wiremock-backed tests

**Files:** `src/store.rs`, `tests/store_registry.rs`

- [ ] **Step 1: Write test against wiremock at `tests/store_registry.rs`**

```rust
use greentic_ext_registry::store::GreenticStoreRegistry;
use greentic_ext_registry::{ExtensionRegistry, SearchQuery};
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn store_registry_search_returns_parsed_results() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v1/extensions"))
        .and(query_param("kind", "design"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
            { "name": "greentic.ac", "latestVersion": "1.6.0", "kind": "DesignExtension",
              "summary": "Adaptive Cards", "downloads": 42 }
        ])))
        .mount(&server)
        .await;

    let reg = GreenticStoreRegistry::new("default", server.uri(), None);
    let q = SearchQuery {
        kind: Some(greentic_ext_contract::ExtensionKind::Design),
        ..Default::default()
    };
    let results = reg.search(q).await.unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].name, "greentic.ac");
}

#[tokio::test]
async fn store_registry_fetch_downloads_artifact() {
    use greentic_ext_contract::{DescribeJson, ExtensionKind};

    let server = MockServer::start().await;
    let describe_json = serde_json::json!({
        "apiVersion": "greentic.ai/v1",
        "kind": "DesignExtension",
        "metadata": {
            "id": "greentic.ac", "name": "AC", "version": "1.6.0",
            "summary": "x", "author": { "name": "G" }, "license": "MIT"
        },
        "engine": { "greenticDesigner": "*", "extRuntime": "*" },
        "capabilities": { "offered": [{"id":"greentic:ac/y","version":"1.0.0"}] },
        "runtime": { "component": "extension.wasm", "permissions": {} },
        "contributions": {}
    });

    Mock::given(method("GET"))
        .and(path("/api/v1/extensions/greentic.ac/1.6.0"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "describe": describe_json,
                "artifactSha256": "deadbeef",
                "publishedAt": "2026-04-17T00:00:00Z",
                "yanked": false
            })),
        )
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/api/v1/extensions/greentic.ac/1.6.0/artifact"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(b"fake-gtxpack-bytes".to_vec()))
        .mount(&server)
        .await;

    let reg = GreenticStoreRegistry::new("default", server.uri(), None);
    let art = reg.fetch("greentic.ac", "1.6.0").await.unwrap();
    assert_eq!(art.name, "greentic.ac");
    assert_eq!(art.bytes, b"fake-gtxpack-bytes");
    // kind suppresses unused-var for ExtensionKind import
    let _ = ExtensionKind::Design;
    let _ = DescribeJson::identity_key;
}
```

- [ ] **Step 2: Write `src/store.rs`**

```rust
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
    pub fn new(name: impl Into<String>, base_url: impl Into<String>, token: Option<String>) -> Self {
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
            .with_auth(self.client.get(self.url(&format!(
                "/api/v1/extensions/{name}/{version}"
            ))))
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

    async fn fetch(
        &self,
        name: &str,
        version: &str,
    ) -> Result<ExtensionArtifact, RegistryError> {
        let metadata = self.metadata(name, version).await?;
        let bytes = self
            .with_auth(self.client.get(self.url(&format!(
                "/api/v1/extensions/{name}/{version}/artifact"
            ))))
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
            .with_auth(self.client.get(self.url(&format!("/api/v1/extensions/{name}"))))
            .send()
            .await?;
        if resp.status() == reqwest::StatusCode::NOT_FOUND {
            return Ok(Vec::new());
        }
        let dto: Dto = resp.error_for_status()?.json().await?;
        Ok(dto.versions)
    }
}
```

- [ ] **Step 3: Run tests**

Run: `cargo test -p greentic-ext-registry --test store_registry`
Expected: 2 passed

- [ ] **Step 4: Commit**

```bash
git add crates/greentic-ext-registry/src/store.rs crates/greentic-ext-registry/tests/store_registry.rs
git commit -m "feat(registry): add GreenticStoreRegistry HTTP client"
```

---

## Phase D — OciRegistry

### Task D.1: OCI registry with skip-if-no-oci test

**Files:** `src/oci.rs`, `tests/oci_registry.rs` (skip test if no OCI server configured)

- [ ] **Step 1: Write `src/oci.rs`**

```rust
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
        // OCI distribution spec has no search endpoint; return empty list.
        // A real impl can read a manifest index from a well-known path if supported.
        Ok(Vec::new())
    }

    async fn metadata(
        &self,
        name: &str,
        version: &str,
    ) -> Result<ExtensionMetadata, RegistryError> {
        let reference = self.reference(name, version);
        let (manifest, _digest) = self
            .client
            .pull_manifest(&reference, &self.auth)
            .await
            .map_err(|e| RegistryError::Oci(e.to_string()))?;

        // For this MVP: look for a layer labelled describe.json via annotation,
        // else require the consumer to fetch() to obtain describe.
        let _ = manifest;
        Err(RegistryError::Storage(
            "OCI metadata introspection not yet implemented; use fetch()".into(),
        ))
    }

    async fn fetch(
        &self,
        name: &str,
        version: &str,
    ) -> Result<ExtensionArtifact, RegistryError> {
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

        // Extract describe.json from the zip bytes
        let cursor = std::io::Cursor::new(&bytes);
        let mut archive = zip::ZipArchive::new(cursor)
            .map_err(|e| RegistryError::Storage(format!("zip open: {e}")))?;
        let mut describe_entry = archive
            .by_name("describe.json")
            .map_err(|e| RegistryError::Storage(format!("describe missing: {e}")))?;
        let value: serde_json::Value = serde_json::from_reader(&mut describe_entry)?;
        greentic_ext_contract::schema::validate_describe_json(&value)?;
        let describe: greentic_ext_contract::DescribeJson = serde_json::from_value(value)?;

        Ok(ExtensionArtifact {
            name: describe.metadata.id.clone(),
            version: describe.metadata.version.clone(),
            describe,
            bytes,
            signature: None,
        })
    }

    async fn list_versions(&self, name: &str) -> Result<Vec<String>, RegistryError> {
        match self
            .client
            .list_tags(
                &format!("{}/{}/{name}", self.registry_host, self.namespace)
                    .parse()
                    .expect("valid reference"),
                &self.auth,
                None,
                None,
            )
            .await
        {
            Ok(resp) => Ok(resp.tags),
            Err(e) => Err(RegistryError::Oci(e.to_string())),
        }
    }

    async fn publish(
        &self,
        _artifact: ExtensionArtifact,
        _auth: &AuthToken,
    ) -> Result<(), RegistryError> {
        Err(RegistryError::Storage(
            "OCI publish requires external `oras push` for now; gtdx publish covers store only"
                .into(),
        ))
    }
}
```

- [ ] **Step 2: Write `tests/oci_registry.rs` (compile-only; skips at runtime if env var missing)**

```rust
use greentic_ext_registry::oci::OciRegistry;
use greentic_ext_registry::{ExtensionRegistry, SearchQuery};

#[tokio::test]
async fn oci_registry_compiles_and_search_returns_empty() {
    // OCI distribution has no search — verify trait method returns empty list
    let reg = OciRegistry::new("test", "ghcr.io", "greenticai/ext", None);
    let results = reg.search(SearchQuery::default()).await.unwrap();
    assert!(results.is_empty());
}
```

- [ ] **Step 3: Run**

Run: `cargo test -p greentic-ext-registry --test oci_registry`
Expected: 1 passed

- [ ] **Step 4: Commit**

```bash
git add crates/greentic-ext-registry/src/oci.rs crates/greentic-ext-registry/tests/oci_registry.rs
git commit -m "feat(registry): add OciRegistry with pull via oci-client"
```

---

## Phase E — Storage layout + lifecycle

### Task E.1: Storage layout helpers

**Files:** `src/storage.rs`, `tests/storage.rs`

- [ ] **Step 1: Write test at `tests/storage.rs`**

```rust
use greentic_ext_registry::storage::Storage;
use tempfile::TempDir;

#[test]
fn computes_extension_dir_for_kind() {
    let tmp = TempDir::new().unwrap();
    let storage = Storage::new(tmp.path());
    let dir = storage.extension_dir(
        greentic_ext_contract::ExtensionKind::Design,
        "greentic.x",
        "1.2.3",
    );
    assert!(dir.ends_with("design/greentic.x-1.2.3"));
}

#[test]
fn stage_and_commit_atomic_move() {
    let tmp = TempDir::new().unwrap();
    let storage = Storage::new(tmp.path());
    let (staging, final_dir) = storage
        .begin_install(
            greentic_ext_contract::ExtensionKind::Design,
            "greentic.x",
            "1.0.0",
        )
        .unwrap();
    std::fs::write(staging.join("file.txt"), "hello").unwrap();
    storage.commit_install(&staging, &final_dir).unwrap();
    assert!(final_dir.join("file.txt").exists());
    assert!(!staging.exists());
}
```

- [ ] **Step 2: Write `src/storage.rs`**

```rust
use std::path::{Path, PathBuf};

use greentic_ext_contract::ExtensionKind;

use crate::error::RegistryError;

pub struct Storage {
    root: PathBuf,
}

impl Storage {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn extensions_root(&self) -> PathBuf {
        self.root.join("extensions")
    }

    pub fn kind_dir(&self, kind: ExtensionKind) -> PathBuf {
        self.extensions_root().join(kind.dir_name())
    }

    pub fn extension_dir(&self, kind: ExtensionKind, name: &str, version: &str) -> PathBuf {
        self.kind_dir(kind).join(format!("{name}-{version}"))
    }

    pub fn registry_json(&self) -> PathBuf {
        self.root.join("registry.json")
    }

    /// Create a staging directory and the final target path. Caller writes into
    /// staging, then calls `commit_install`.
    pub fn begin_install(
        &self,
        kind: ExtensionKind,
        name: &str,
        version: &str,
    ) -> Result<(PathBuf, PathBuf), RegistryError> {
        let final_dir = self.extension_dir(kind, name, version);
        let staging = final_dir.with_extension("tmp");
        std::fs::create_dir_all(&staging)?;
        Ok((staging, final_dir))
    }

    pub fn commit_install(&self, staging: &Path, final_dir: &Path) -> Result<(), RegistryError> {
        if let Some(parent) = final_dir.parent() {
            std::fs::create_dir_all(parent)?;
        }
        if final_dir.exists() {
            std::fs::remove_dir_all(final_dir)?;
        }
        std::fs::rename(staging, final_dir)?;
        Ok(())
    }

    pub fn abort_install(&self, staging: &Path) {
        let _ = std::fs::remove_dir_all(staging);
    }

    pub fn remove_extension(
        &self,
        kind: ExtensionKind,
        name: &str,
        version: &str,
    ) -> Result<(), RegistryError> {
        let dir = self.extension_dir(kind, name, version);
        if dir.exists() {
            std::fs::remove_dir_all(&dir)?;
        }
        Ok(())
    }
}
```

- [ ] **Step 3: Run + commit**

```bash
cargo test -p greentic-ext-registry --test storage
git add crates/greentic-ext-registry/src/storage.rs crates/greentic-ext-registry/tests/storage.rs
git commit -m "feat(registry): add Storage layout helper"
```

### Task E.2: Install lifecycle

**Files:** `src/lifecycle.rs`, `tests/lifecycle.rs`

- [ ] **Step 1: Write test at `tests/lifecycle.rs`**

```rust
use greentic_ext_contract::ExtensionKind;
use greentic_ext_registry::lifecycle::{InstallOptions, Installer};
use greentic_ext_registry::local::LocalFilesystemRegistry;
use greentic_ext_registry::storage::Storage;
use greentic_ext_testing::{pack_directory, ExtensionFixtureBuilder};
use tempfile::TempDir;

#[tokio::test]
async fn installs_from_local_registry() {
    let tmp_reg = TempDir::new().unwrap();
    let tmp_home = TempDir::new().unwrap();

    let fixture = ExtensionFixtureBuilder::new(
        ExtensionKind::Design,
        "greentic.install-me",
        "0.1.0",
    )
    .offer("greentic:im/hi", "1.0.0")
    .with_wasm(b"wasm".to_vec())
    .build()
    .unwrap();
    let pack = tmp_reg.path().join("greentic.install-me-0.1.0.gtxpack");
    pack_directory(fixture.root(), &pack).unwrap();

    let reg = LocalFilesystemRegistry::new("local", tmp_reg.path());
    let storage = Storage::new(tmp_home.path());
    let installer = Installer::new(storage, &reg);

    installer
        .install(
            "greentic.install-me",
            "0.1.0",
            InstallOptions {
                trust_policy: greentic_ext_registry::lifecycle::TrustPolicy::Loose,
                accept_permissions: true,
            },
        )
        .await
        .unwrap();

    let dir = tmp_home
        .path()
        .join("extensions/design/greentic.install-me-0.1.0");
    assert!(dir.join("describe.json").exists());
    assert!(dir.join("extension.wasm").exists());
}
```

- [ ] **Step 2: Write `src/lifecycle.rs`**

```rust
use std::io::Cursor;

use crate::error::RegistryError;
use crate::registry::ExtensionRegistry;
use crate::storage::Storage;
use crate::types::ExtensionArtifact;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrustPolicy {
    Strict,
    Normal,
    Loose,
}

#[derive(Debug, Clone)]
pub struct InstallOptions {
    pub trust_policy: TrustPolicy,
    pub accept_permissions: bool,
}

impl Default for InstallOptions {
    fn default() -> Self {
        Self {
            trust_policy: TrustPolicy::Normal,
            accept_permissions: false,
        }
    }
}

pub struct Installer<'a, R: ExtensionRegistry + ?Sized> {
    storage: Storage,
    registry: &'a R,
}

impl<'a, R: ExtensionRegistry + ?Sized> Installer<'a, R> {
    pub fn new(storage: Storage, registry: &'a R) -> Self {
        Self { storage, registry }
    }

    pub async fn install(
        &self,
        name: &str,
        version: &str,
        opts: InstallOptions,
    ) -> Result<(), RegistryError> {
        let artifact = self.registry.fetch(name, version).await?;
        self.verify_signature(&artifact, opts.trust_policy)?;
        self.install_artifact(artifact, opts).await
    }

    pub async fn install_artifact(
        &self,
        artifact: ExtensionArtifact,
        _opts: InstallOptions,
    ) -> Result<(), RegistryError> {
        let kind = artifact.describe.kind;
        let (staging, final_dir) =
            self.storage.begin_install(kind, &artifact.name, &artifact.version)?;

        let result = self.extract_to_staging(&artifact, &staging);
        if result.is_err() {
            self.storage.abort_install(&staging);
            result?;
        }
        self.storage.commit_install(&staging, &final_dir)?;
        tracing::info!(
            name = %artifact.name,
            version = %artifact.version,
            kind = ?kind,
            "extension installed"
        );
        Ok(())
    }

    fn extract_to_staging(
        &self,
        artifact: &ExtensionArtifact,
        staging: &std::path::Path,
    ) -> Result<(), RegistryError> {
        let cursor = Cursor::new(&artifact.bytes);
        let mut archive = zip::ZipArchive::new(cursor)
            .map_err(|e| RegistryError::Storage(format!("zip open: {e}")))?;
        for i in 0..archive.len() {
            let mut entry = archive
                .by_index(i)
                .map_err(|e| RegistryError::Storage(format!("zip entry: {e}")))?;
            let out_path = staging.join(entry.mangled_name());
            if entry.is_dir() {
                std::fs::create_dir_all(&out_path)?;
                continue;
            }
            if let Some(parent) = out_path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            let mut out = std::fs::File::create(&out_path)?;
            std::io::copy(&mut entry, &mut out)?;
        }
        Ok(())
    }

    fn verify_signature(
        &self,
        artifact: &ExtensionArtifact,
        policy: TrustPolicy,
    ) -> Result<(), RegistryError> {
        match policy {
            TrustPolicy::Loose => Ok(()),
            TrustPolicy::Strict | TrustPolicy::Normal => {
                let Some(sig) = &artifact.describe.signature else {
                    return Err(RegistryError::SignatureInvalid(
                        "missing signature".into(),
                    ));
                };
                let payload = serde_json::to_vec(&artifact.describe)?;
                greentic_ext_contract::verify_ed25519(
                    &sig.public_key,
                    &sig.value,
                    &payload,
                )
                .map_err(|e| RegistryError::SignatureInvalid(e.to_string()))
            }
        }
    }

    pub fn uninstall(
        &self,
        kind: greentic_ext_contract::ExtensionKind,
        name: &str,
        version: &str,
    ) -> Result<(), RegistryError> {
        self.storage.remove_extension(kind, name, version)
    }
}
```

- [ ] **Step 3: Run test**

Run: `cargo test -p greentic-ext-registry --test lifecycle`
Expected: 1 passed

- [ ] **Step 4: Commit**

```bash
git add crates/greentic-ext-registry/src/lifecycle.rs crates/greentic-ext-registry/tests/lifecycle.rs
git commit -m "feat(registry): add install/uninstall lifecycle"
```

---

## Phase F — Config + credentials + permission prompt

### Task F.1: `config.rs` — `~/.greentic/config.toml`

**Files:** `src/config.rs`, `tests/config.rs`

- [ ] **Step 1: Write `src/config.rs`**

```rust
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::RegistryError;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GtdxConfig {
    pub default: DefaultSection,
    #[serde(default, rename = "registries")]
    pub registries: Vec<RegistryEntry>,
    #[serde(default, rename = "extensions")]
    pub extensions: std::collections::BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DefaultSection {
    pub registry: String,
    #[serde(rename = "trust-policy")]
    pub trust_policy: String,
}

impl Default for DefaultSection {
    fn default() -> Self {
        Self {
            registry: "greentic-store".into(),
            trust_policy: "normal".into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistryEntry {
    pub name: String,
    pub url: String,
    #[serde(rename = "token-env", default)]
    pub token_env: Option<String>,
}

pub fn load(path: &Path) -> Result<GtdxConfig, RegistryError> {
    if !path.exists() {
        return Ok(GtdxConfig::default());
    }
    let bytes = std::fs::read_to_string(path)?;
    Ok(toml::from_str(&bytes)?)
}

pub fn save(path: &Path, cfg: &GtdxConfig) -> Result<(), RegistryError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let s = toml::to_string_pretty(cfg)
        .map_err(|e| RegistryError::Storage(format!("toml ser: {e}")))?;
    std::fs::write(path, s)?;
    Ok(())
}
```

- [ ] **Step 2: Write test**

```rust
use greentic_ext_registry::config::{load, save, GtdxConfig, RegistryEntry};
use tempfile::TempDir;

#[test]
fn load_missing_returns_default() {
    let tmp = TempDir::new().unwrap();
    let cfg = load(&tmp.path().join("config.toml")).unwrap();
    assert_eq!(cfg.default.registry, "greentic-store");
}

#[test]
fn save_and_reload_preserves_registries() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("config.toml");
    let mut cfg = GtdxConfig::default();
    cfg.registries.push(RegistryEntry {
        name: "custom".into(),
        url: "https://example.com".into(),
        token_env: Some("MY_TOKEN".into()),
    });
    save(&path, &cfg).unwrap();

    let reloaded = load(&path).unwrap();
    assert_eq!(reloaded.registries.len(), 1);
    assert_eq!(reloaded.registries[0].name, "custom");
}
```

- [ ] **Step 3: Commit**

```bash
git add crates/greentic-ext-registry/src/config.rs crates/greentic-ext-registry/tests/config.rs
git commit -m "feat(registry): add GtdxConfig TOML load/save"
```

### Task F.2: Credentials storage

**Files:** `src/credentials.rs`

- [ ] **Step 1: Write `src/credentials.rs`**

```rust
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::RegistryError;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Credentials {
    #[serde(default)]
    pub tokens: std::collections::BTreeMap<String, String>,
}

impl Credentials {
    pub fn load(path: &Path) -> Result<Self, RegistryError> {
        if !path.exists() {
            return Ok(Self::default());
        }
        let s = std::fs::read_to_string(path)?;
        Ok(toml::from_str(&s)?)
    }

    pub fn save(&self, path: &Path) -> Result<(), RegistryError> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let s = toml::to_string_pretty(self)
            .map_err(|e| RegistryError::Storage(format!("toml ser: {e}")))?;
        std::fs::write(path, s)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perm = std::fs::metadata(path)?.permissions();
            perm.set_mode(0o600);
            std::fs::set_permissions(path, perm)?;
        }
        Ok(())
    }

    pub fn set(&mut self, registry: &str, token: &str) {
        self.tokens.insert(registry.into(), token.into());
    }

    pub fn get(&self, registry: &str) -> Option<&str> {
        self.tokens.get(registry).map(String::as_str)
    }

    pub fn remove(&mut self, registry: &str) -> Option<String> {
        self.tokens.remove(registry)
    }
}
```

- [ ] **Step 2: Commit**

```bash
git add crates/greentic-ext-registry/src/credentials.rs
git commit -m "feat(registry): add Credentials TOML with 0600 perm on unix"
```

### Task F.3: Permission prompt

**Files:** `src/prompt.rs`

- [ ] **Step 1: Write `src/prompt.rs`**

```rust
use greentic_ext_contract::DescribeJson;

/// Prints a prompt showing the extension's requested permissions and returns
/// user's y/n answer. When `auto_accept` is true, always returns true (for
/// CI / scripting).
pub fn confirm_install(describe: &DescribeJson, auto_accept: bool) -> bool {
    if auto_accept {
        return true;
    }
    let perms = &describe.runtime.permissions;
    eprintln!();
    eprintln!("⚠️  Extension {} v{} requests:",
        describe.metadata.id, describe.metadata.version);
    if !perms.network.is_empty() {
        eprintln!("  Network: {}", perms.network.join(", "));
    }
    if !perms.secrets.is_empty() {
        eprintln!("  Secrets: {}", perms.secrets.join(", "));
    }
    if !perms.call_extension_kinds.is_empty() {
        eprintln!(
            "  Cross-extension: may call {} extensions",
            perms.call_extension_kinds.join(", ")
        );
    }
    if perms.network.is_empty() && perms.secrets.is_empty() && perms.call_extension_kinds.is_empty()
    {
        eprintln!("  (no special permissions)");
    }
    eprintln!();

    dialoguer::Confirm::new()
        .with_prompt("Install?")
        .default(false)
        .interact()
        .unwrap_or(false)
}
```

- [ ] **Step 2: Commit**

```bash
git add crates/greentic-ext-registry/src/prompt.rs
git commit -m "feat(registry): add permission prompt helper"
```

---

## Phase G — CLI expansion

### Task G.1: Restructure CLI with command modules

**Files:**
- Create: `crates/greentic-ext-cli/src/commands/` (mod.rs + one file per group)
- Modify: `crates/greentic-ext-cli/src/main.rs`
- Modify: `crates/greentic-ext-cli/Cargo.toml` (add greentic-ext-registry dep)

- [ ] **Step 1: Add dep**

Edit `crates/greentic-ext-cli/Cargo.toml`:

```toml
greentic-ext-registry = { path = "../greentic-ext-registry" }
dialoguer = { workspace = true }
directories = "5"
```

Add `directories` to workspace deps in root `Cargo.toml`:

```toml
directories = "5"
```

- [ ] **Step 2: Restructure `src/main.rs`**

```rust
mod commands;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "gtdx", version, about = "Greentic Designer Extensions CLI")]
struct Cli {
    /// Override greentic home directory (default: ~/.greentic)
    #[arg(long, env = "GREENTIC_HOME", global = true)]
    home: Option<String>,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Validate an extension directory against the describe.json schema
    Validate(commands::validate::Args),
    /// List installed extensions
    List(commands::list::Args),
    /// Install an extension from a registry or local path
    Install(commands::install::Args),
    /// Remove an installed extension
    Uninstall(commands::uninstall::Args),
    /// Search a registry
    Search(commands::search::Args),
    /// Show metadata for an extension
    Info(commands::info::Args),
    /// Log in to a registry (stores token at ~/.greentic/credentials.toml)
    Login(commands::login::Args),
    /// Log out of a registry
    Logout(commands::login::LogoutArgs),
    /// Show/modify configured registries
    Registries(commands::registries::Args),
    /// Diagnose installed extensions
    Doctor(commands::doctor::Args),
    /// Print version
    Version,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let cli = Cli::parse();
    let home = resolve_home(cli.home.as_deref())?;

    match cli.command {
        Command::Validate(args) => commands::validate::run(args, &home).await,
        Command::List(args) => commands::list::run(args, &home).await,
        Command::Install(args) => commands::install::run(args, &home).await,
        Command::Uninstall(args) => commands::uninstall::run(args, &home).await,
        Command::Search(args) => commands::search::run(args, &home).await,
        Command::Info(args) => commands::info::run(args, &home).await,
        Command::Login(args) => commands::login::run_login(args, &home).await,
        Command::Logout(args) => commands::login::run_logout(args, &home).await,
        Command::Registries(args) => commands::registries::run(args, &home).await,
        Command::Doctor(args) => commands::doctor::run(args, &home).await,
        Command::Version => {
            println!("gtdx {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
    }
}

fn resolve_home(override_path: Option<&str>) -> anyhow::Result<std::path::PathBuf> {
    if let Some(p) = override_path {
        return Ok(p.into());
    }
    directories::BaseDirs::new()
        .map(|d| d.home_dir().join(".greentic"))
        .ok_or_else(|| anyhow::anyhow!("no home directory"))
}
```

- [ ] **Step 3: Create `src/commands/mod.rs`**

```rust
pub mod doctor;
pub mod info;
pub mod install;
pub mod list;
pub mod login;
pub mod registries;
pub mod search;
pub mod uninstall;
pub mod validate;

use anyhow::Result;
use greentic_ext_registry::config;
use std::path::Path;

pub fn load_config(home: &Path) -> Result<config::GtdxConfig> {
    config::load(&home.join("config.toml"))
        .map_err(|e| anyhow::anyhow!("config: {e}"))
}

pub fn save_config(home: &Path, cfg: &config::GtdxConfig) -> Result<()> {
    config::save(&home.join("config.toml"), cfg)
        .map_err(|e| anyhow::anyhow!("config save: {e}"))
}
```

- [ ] **Step 4: Commit skeleton**

```bash
git add crates/greentic-ext-cli/ Cargo.toml Cargo.lock
git commit -m "feat(cli): restructure with command modules"
```

### Task G.2: `validate` command (moved from stub) + `list`

**Files:** `src/commands/validate.rs`, `src/commands/list.rs`

- [ ] **Step 1: `validate.rs`**

```rust
use std::path::Path;

use clap::Args as ClapArgs;

#[derive(ClapArgs, Debug)]
pub struct Args {
    /// Path to an extension source directory containing describe.json
    #[arg(default_value = ".")]
    pub path: String,
}

pub async fn run(args: Args, _home: &Path) -> anyhow::Result<()> {
    let describe_path = Path::new(&args.path).join("describe.json");
    let bytes = std::fs::read(&describe_path)?;
    let value: serde_json::Value = serde_json::from_slice(&bytes)?;
    greentic_ext_contract::schema::validate_describe_json(&value)
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    let _: greentic_ext_contract::DescribeJson = serde_json::from_value(value)?;
    println!("✓ {} valid", describe_path.display());
    Ok(())
}
```

- [ ] **Step 2: `list.rs`**

```rust
use std::path::Path;

use clap::Args as ClapArgs;
use greentic_ext_contract::ExtensionKind;
use greentic_ext_registry::storage::Storage;

#[derive(ClapArgs, Debug)]
pub struct Args;

pub async fn run(_args: Args, home: &Path) -> anyhow::Result<()> {
    let storage = Storage::new(home);
    for kind in [ExtensionKind::Design, ExtensionKind::Bundle, ExtensionKind::Deploy] {
        let dir = storage.kind_dir(kind);
        if !dir.exists() {
            continue;
        }
        let mut any = false;
        for entry in std::fs::read_dir(&dir)? {
            let entry = entry?;
            if !entry.file_type()?.is_dir() {
                continue;
            }
            let describe_path = entry.path().join("describe.json");
            if !describe_path.exists() {
                continue;
            }
            let bytes = std::fs::read(&describe_path)?;
            let value: serde_json::Value = serde_json::from_slice(&bytes)?;
            let d: greentic_ext_contract::DescribeJson = serde_json::from_value(value)?;
            if !any {
                println!("[{}]", kind.dir_name());
                any = true;
            }
            println!("  {}@{}  {}", d.metadata.id, d.metadata.version, d.metadata.summary);
        }
    }
    Ok(())
}
```

- [ ] **Step 3: Commit**

```bash
git add crates/greentic-ext-cli/src/commands/validate.rs \
        crates/greentic-ext-cli/src/commands/list.rs
git commit -m "feat(cli): add validate + list commands"
```

### Task G.3: `install` / `uninstall` commands

**Files:** `src/commands/install.rs`, `src/commands/uninstall.rs`

- [ ] **Step 1: `install.rs`**

```rust
use std::path::{Path, PathBuf};

use clap::Args as ClapArgs;
use greentic_ext_registry::{
    lifecycle::{InstallOptions, Installer, TrustPolicy},
    local::LocalFilesystemRegistry,
    storage::Storage,
    store::GreenticStoreRegistry,
    ExtensionRegistry,
};

#[derive(ClapArgs, Debug)]
pub struct Args {
    /// Name (from registry) or path to a local .gtxpack file
    pub target: String,
    /// Version (required for registry install, ignored for local)
    #[arg(long)]
    pub version: Option<String>,
    /// Registry name from config
    #[arg(long)]
    pub registry: Option<String>,
    /// Skip permission prompt
    #[arg(long, short = 'y')]
    pub yes: bool,
    /// Trust policy override (strict | normal | loose)
    #[arg(long)]
    pub trust: Option<String>,
}

pub async fn run(args: Args, home: &Path) -> anyhow::Result<()> {
    let cfg = super::load_config(home)?;
    let storage = Storage::new(home);
    let trust_policy = parse_trust(args.trust.as_deref(), &cfg.default.trust_policy)?;

    let target_path = Path::new(&args.target);
    if target_path.exists() {
        install_from_local_file(&storage, target_path, trust_policy, args.yes).await
    } else {
        let version = args
            .version
            .clone()
            .ok_or_else(|| anyhow::anyhow!("--version required for registry install"))?;
        install_from_registry(&cfg, &args, &storage, &version, trust_policy).await
    }
}

async fn install_from_local_file(
    storage: &Storage,
    pack_path: &Path,
    trust: TrustPolicy,
    yes: bool,
) -> anyhow::Result<()> {
    let parent = pack_path
        .parent()
        .ok_or_else(|| anyhow::anyhow!("no parent dir"))?;
    let reg = LocalFilesystemRegistry::new("cli-local", parent);

    let filename = pack_path
        .file_name()
        .and_then(|s| s.to_str())
        .ok_or_else(|| anyhow::anyhow!("bad filename"))?;
    let (name, version) = parse_pack_name(filename)?;

    let installer = Installer::new(storage.clone_shallow(), &reg);
    installer
        .install(
            &name,
            &version,
            InstallOptions {
                trust_policy: trust,
                accept_permissions: yes,
            },
        )
        .await
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    println!("✓ installed {name}@{version}");
    Ok(())
}

async fn install_from_registry(
    cfg: &greentic_ext_registry::config::GtdxConfig,
    args: &Args,
    storage: &Storage,
    version: &str,
    trust: TrustPolicy,
) -> anyhow::Result<()> {
    let reg_name = args.registry.as_deref().unwrap_or(&cfg.default.registry);
    let entry = cfg
        .registries
        .iter()
        .find(|r| r.name == reg_name)
        .ok_or_else(|| anyhow::anyhow!("no such registry: {reg_name}"))?;

    let token = entry
        .token_env
        .as_deref()
        .and_then(|e| std::env::var(e).ok());
    let reg = GreenticStoreRegistry::new(&entry.name, &entry.url, token);
    let installer = Installer::new(storage.clone_shallow(), &reg);
    installer
        .install(
            &args.target,
            version,
            InstallOptions {
                trust_policy: trust,
                accept_permissions: args.yes,
            },
        )
        .await
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    println!("✓ installed {}@{version}", args.target);
    Ok(())
}

fn parse_trust(
    override_val: Option<&str>,
    default_val: &str,
) -> anyhow::Result<TrustPolicy> {
    let raw = override_val.unwrap_or(default_val);
    match raw {
        "strict" => Ok(TrustPolicy::Strict),
        "normal" => Ok(TrustPolicy::Normal),
        "loose" => Ok(TrustPolicy::Loose),
        x => Err(anyhow::anyhow!("unknown trust policy: {x}")),
    }
}

fn parse_pack_name(filename: &str) -> anyhow::Result<(String, String)> {
    let stem = filename
        .strip_suffix(".gtxpack")
        .ok_or_else(|| anyhow::anyhow!("not a .gtxpack file: {filename}"))?;
    let idx = stem
        .rfind('-')
        .ok_or_else(|| anyhow::anyhow!("no version in filename: {filename}"))?;
    let (name, rest) = stem.split_at(idx);
    let version = rest.strip_prefix('-').unwrap_or(rest);
    Ok((name.into(), version.into()))
}
```

Also add a `clone_shallow` method to `Storage` (to allow moving into Installer while keeping the original):

Edit `crates/greentic-ext-registry/src/storage.rs`:

```rust
impl Storage {
    pub fn clone_shallow(&self) -> Self {
        Self { root: self.root.clone() }
    }
}
```

- [ ] **Step 2: `uninstall.rs`**

```rust
use std::path::Path;

use clap::Args as ClapArgs;
use greentic_ext_contract::ExtensionKind;
use greentic_ext_registry::storage::Storage;

#[derive(ClapArgs, Debug)]
pub struct Args {
    /// Name of the installed extension (with or without version suffix)
    pub name: String,
    /// Optional version (if omitted, uninstalls all versions of the name)
    #[arg(long)]
    pub version: Option<String>,
}

pub async fn run(args: Args, home: &Path) -> anyhow::Result<()> {
    let storage = Storage::new(home);
    let mut removed_any = false;
    for kind in [ExtensionKind::Design, ExtensionKind::Bundle, ExtensionKind::Deploy] {
        let dir = storage.kind_dir(kind);
        if !dir.exists() {
            continue;
        }
        for entry in std::fs::read_dir(&dir)? {
            let entry = entry?;
            let dname = entry.file_name();
            let dname_str = dname.to_string_lossy();
            if let Some((n, v)) = split_name_version(&dname_str) {
                if n == args.name {
                    if let Some(want_v) = &args.version {
                        if want_v != v {
                            continue;
                        }
                    }
                    std::fs::remove_dir_all(entry.path())?;
                    println!("✓ removed {n}@{v}");
                    removed_any = true;
                }
            }
        }
    }
    if !removed_any {
        eprintln!("nothing to remove for {}", args.name);
    }
    Ok(())
}

fn split_name_version(dirname: &str) -> Option<(&str, &str)> {
    let idx = dirname.rfind('-')?;
    let (n, rest) = dirname.split_at(idx);
    Some((n, rest.strip_prefix('-')?))
}
```

- [ ] **Step 3: Commit**

```bash
git add crates/greentic-ext-cli/src/commands/install.rs \
        crates/greentic-ext-cli/src/commands/uninstall.rs \
        crates/greentic-ext-registry/src/storage.rs
git commit -m "feat(cli): add install + uninstall commands"
```

### Task G.4: `search` / `info` / `registries` / `login` / `doctor`

**Files:** 5 command modules

- [ ] **Step 1: `search.rs`**

```rust
use std::path::Path;

use clap::Args as ClapArgs;
use greentic_ext_registry::{
    store::GreenticStoreRegistry, ExtensionRegistry, SearchQuery,
};

#[derive(ClapArgs, Debug)]
pub struct Args {
    pub query: String,
    #[arg(long)]
    pub registry: Option<String>,
    #[arg(long)]
    pub kind: Option<String>,
    #[arg(long, default_value_t = 20)]
    pub limit: u32,
}

pub async fn run(args: Args, home: &Path) -> anyhow::Result<()> {
    let cfg = super::load_config(home)?;
    let reg_name = args.registry.as_deref().unwrap_or(&cfg.default.registry);
    let entry = cfg
        .registries
        .iter()
        .find(|r| r.name == reg_name)
        .ok_or_else(|| anyhow::anyhow!("no such registry: {reg_name}"))?;
    let token = entry
        .token_env
        .as_deref()
        .and_then(|e| std::env::var(e).ok());
    let reg = GreenticStoreRegistry::new(&entry.name, &entry.url, token);

    let kind = match args.kind.as_deref() {
        Some("design") => Some(greentic_ext_contract::ExtensionKind::Design),
        Some("bundle") => Some(greentic_ext_contract::ExtensionKind::Bundle),
        Some("deploy") => Some(greentic_ext_contract::ExtensionKind::Deploy),
        Some(x) => return Err(anyhow::anyhow!("unknown kind: {x}")),
        None => None,
    };

    let results = reg
        .search(SearchQuery {
            kind,
            query: Some(args.query),
            limit: args.limit,
            ..Default::default()
        })
        .await?;
    for r in results {
        println!(
            "{:<40}  {:>10}  {:?}  {}",
            r.name, r.latest_version, r.kind, r.summary
        );
    }
    Ok(())
}
```

- [ ] **Step 2: `info.rs`**

```rust
use std::path::Path;

use clap::Args as ClapArgs;
use greentic_ext_registry::{store::GreenticStoreRegistry, ExtensionRegistry};

#[derive(ClapArgs, Debug)]
pub struct Args {
    pub name: String,
    #[arg(long)]
    pub version: Option<String>,
    #[arg(long)]
    pub registry: Option<String>,
}

pub async fn run(args: Args, home: &Path) -> anyhow::Result<()> {
    let cfg = super::load_config(home)?;
    let reg_name = args.registry.as_deref().unwrap_or(&cfg.default.registry);
    let entry = cfg
        .registries
        .iter()
        .find(|r| r.name == reg_name)
        .ok_or_else(|| anyhow::anyhow!("no such registry: {reg_name}"))?;
    let reg = GreenticStoreRegistry::new(&entry.name, &entry.url, None);

    let versions = reg.list_versions(&args.name).await?;
    let version = match args.version {
        Some(v) => v,
        None => versions
            .last()
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("no versions published"))?,
    };
    let meta = reg.metadata(&args.name, &version).await?;
    println!("name:     {}", meta.name);
    println!("version:  {}", meta.version);
    println!("kind:     {:?}", meta.describe.kind);
    println!("license:  {}", meta.describe.metadata.license);
    println!("summary:  {}", meta.describe.metadata.summary);
    println!("sha256:   {}", meta.artifact_sha256);
    println!("versions:");
    for v in versions {
        println!("  {v}");
    }
    Ok(())
}
```

- [ ] **Step 3: `login.rs`**

```rust
use std::path::Path;

use clap::Args as ClapArgs;
use greentic_ext_registry::credentials::Credentials;

#[derive(ClapArgs, Debug)]
pub struct Args {
    /// Registry name from config
    #[arg(long)]
    pub registry: Option<String>,
}

#[derive(ClapArgs, Debug)]
pub struct LogoutArgs {
    #[arg(long)]
    pub registry: Option<String>,
}

pub async fn run_login(args: Args, home: &Path) -> anyhow::Result<()> {
    let cfg = super::load_config(home)?;
    let reg_name = args.registry.as_deref().unwrap_or(&cfg.default.registry);
    let token = dialoguer::Password::new()
        .with_prompt(format!("Token for {reg_name}"))
        .interact()?;
    let creds_path = home.join("credentials.toml");
    let mut creds = Credentials::load(&creds_path).unwrap_or_default();
    creds.set(reg_name, &token);
    creds.save(&creds_path).map_err(|e| anyhow::anyhow!("{e}"))?;
    println!("✓ logged in to {reg_name}");
    Ok(())
}

pub async fn run_logout(args: LogoutArgs, home: &Path) -> anyhow::Result<()> {
    let cfg = super::load_config(home)?;
    let reg_name = args.registry.as_deref().unwrap_or(&cfg.default.registry);
    let creds_path = home.join("credentials.toml");
    let mut creds = Credentials::load(&creds_path).unwrap_or_default();
    if creds.remove(reg_name).is_some() {
        creds.save(&creds_path).map_err(|e| anyhow::anyhow!("{e}"))?;
        println!("✓ logged out of {reg_name}");
    } else {
        println!("no credentials for {reg_name}");
    }
    Ok(())
}
```

- [ ] **Step 4: `registries.rs`**

```rust
use std::path::Path;

use clap::{Args as ClapArgs, Subcommand};
use greentic_ext_registry::config::RegistryEntry;

#[derive(ClapArgs, Debug)]
pub struct Args {
    #[command(subcommand)]
    pub op: Op,
}

#[derive(Subcommand, Debug)]
pub enum Op {
    /// List configured registries
    List,
    /// Add a registry
    Add {
        name: String,
        url: String,
        #[arg(long)]
        token_env: Option<String>,
    },
    /// Remove a registry
    Remove { name: String },
    /// Set default registry
    SetDefault { name: String },
}

pub async fn run(args: Args, home: &Path) -> anyhow::Result<()> {
    let mut cfg = super::load_config(home)?;
    match args.op {
        Op::List => {
            println!("default: {}", cfg.default.registry);
            for r in &cfg.registries {
                println!("  {}  {}", r.name, r.url);
            }
        }
        Op::Add { name, url, token_env } => {
            cfg.registries.push(RegistryEntry { name: name.clone(), url, token_env });
            super::save_config(home, &cfg)?;
            println!("✓ added {name}");
        }
        Op::Remove { name } => {
            cfg.registries.retain(|r| r.name != name);
            super::save_config(home, &cfg)?;
            println!("✓ removed {name}");
        }
        Op::SetDefault { name } => {
            if !cfg.registries.iter().any(|r| r.name == name) {
                return Err(anyhow::anyhow!("registry {name} not configured"));
            }
            cfg.default.registry = name.clone();
            super::save_config(home, &cfg)?;
            println!("✓ default = {name}");
        }
    }
    Ok(())
}
```

- [ ] **Step 5: `doctor.rs`**

```rust
use std::path::Path;

use clap::Args as ClapArgs;
use greentic_ext_contract::ExtensionKind;
use greentic_ext_registry::storage::Storage;

#[derive(ClapArgs, Debug)]
pub struct Args;

pub async fn run(_args: Args, home: &Path) -> anyhow::Result<()> {
    let storage = Storage::new(home);
    let mut total = 0;
    let mut bad = 0;
    for kind in [ExtensionKind::Design, ExtensionKind::Bundle, ExtensionKind::Deploy] {
        let dir = storage.kind_dir(kind);
        if !dir.exists() {
            continue;
        }
        for entry in std::fs::read_dir(&dir)? {
            let entry = entry?;
            if !entry.file_type()?.is_dir() {
                continue;
            }
            total += 1;
            let describe_path = entry.path().join("describe.json");
            if !describe_path.exists() {
                println!("✗ {} (no describe.json)", entry.path().display());
                bad += 1;
                continue;
            }
            let bytes = std::fs::read(&describe_path)?;
            let value: serde_json::Value = match serde_json::from_slice(&bytes) {
                Ok(v) => v,
                Err(e) => {
                    println!("✗ {}: invalid JSON: {e}", describe_path.display());
                    bad += 1;
                    continue;
                }
            };
            if let Err(e) = greentic_ext_contract::schema::validate_describe_json(&value) {
                println!("✗ {}: {e}", describe_path.display());
                bad += 1;
            } else {
                println!("✓ {}", describe_path.display());
            }
        }
    }
    println!();
    println!("{} total, {} bad", total, bad);
    if bad > 0 {
        std::process::exit(1);
    }
    Ok(())
}
```

- [ ] **Step 6: Build + commit**

Run: `cargo build -p greentic-ext-cli`
Expected: builds

```bash
git add crates/greentic-ext-cli/src/commands/
git commit -m "feat(cli): add search + info + login + registries + doctor commands"
```

---

## Phase H — End-to-end CLI integration test

### Task H.1: CLI e2e test

**Files:** `crates/greentic-ext-cli/tests/cli_e2e.rs`

- [ ] **Step 1: Write test**

```rust
use std::process::Command;

use greentic_ext_contract::ExtensionKind;
use greentic_ext_testing::{pack_directory, ExtensionFixtureBuilder};
use tempfile::TempDir;

fn gtdx_bin() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_BIN_EXE_gtdx"))
}

#[test]
fn validate_command_accepts_valid_extension() {
    let fixture = ExtensionFixtureBuilder::new(
        ExtensionKind::Design,
        "greentic.cli-test",
        "0.1.0",
    )
    .offer("greentic:cli/y", "1.0.0")
    .with_wasm(b"wasm".to_vec())
    .build()
    .unwrap();

    let output = Command::new(gtdx_bin())
        .arg("validate")
        .arg(fixture.root())
        .output()
        .unwrap();
    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));
}

#[test]
fn install_from_local_pack_copies_into_home() {
    let tmp = TempDir::new().unwrap();
    let home = tmp.path().join("home");
    let pack_dir = tmp.path().join("packs");
    std::fs::create_dir_all(&pack_dir).unwrap();

    let fixture = ExtensionFixtureBuilder::new(
        ExtensionKind::Design,
        "greentic.cli-install",
        "0.1.0",
    )
    .offer("greentic:ci/y", "1.0.0")
    .with_wasm(b"wasm".to_vec())
    .build()
    .unwrap();
    let pack = pack_dir.join("greentic.cli-install-0.1.0.gtxpack");
    pack_directory(fixture.root(), &pack).unwrap();

    let output = Command::new(gtdx_bin())
        .arg("--home").arg(&home)
        .arg("install")
        .arg(&pack)
        .arg("-y")
        .arg("--trust").arg("loose")
        .output()
        .unwrap();
    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));

    assert!(home.join("extensions/design/greentic.cli-install-0.1.0/describe.json").exists());
}

#[test]
fn list_shows_installed_extensions() {
    let tmp = TempDir::new().unwrap();
    let home = tmp.path().join("home");
    let design_dir = home.join("extensions/design/greentic.demo-0.1.0");
    std::fs::create_dir_all(&design_dir).unwrap();

    let fixture = ExtensionFixtureBuilder::new(
        ExtensionKind::Design,
        "greentic.demo",
        "0.1.0",
    )
    .offer("greentic:d/y", "1.0.0")
    .build()
    .unwrap();
    std::fs::copy(
        fixture.root().join("describe.json"),
        design_dir.join("describe.json"),
    )
    .unwrap();

    let output = Command::new(gtdx_bin())
        .arg("--home").arg(&home)
        .arg("list")
        .output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("greentic.demo@0.1.0"), "got: {stdout}");
}
```

Add dev dep to cli Cargo.toml:

```toml
[dev-dependencies]
greentic-ext-testing = { path = "../greentic-ext-testing" }
tempfile = { workspace = true }
```

- [ ] **Step 2: Run**

Run: `cargo test -p greentic-ext-cli --test cli_e2e`
Expected: 3 passed

- [ ] **Step 3: Commit**

```bash
git add crates/greentic-ext-cli/tests/cli_e2e.rs crates/greentic-ext-cli/Cargo.toml
git commit -m "test(cli): end-to-end validate/install/list"
```

---

## Phase I — OpenAPI spec for Greentic Store

### Task I.1: Write OpenAPI 3.1 spec

**Files:** `docs/greentic-store-api.openapi.yaml`

- [ ] **Step 1: Write OpenAPI file**

```yaml
openapi: 3.1.0
info:
  title: Greentic Store API
  version: "1.0"
  license: { name: "MIT" }
  description: |
    HTTP API contract for the Greentic Store. Implemented by the
    `greentic-store-server` repo. Consumed by `gtdx` via the
    `GreenticStoreRegistry` client.

servers:
  - url: https://store.greentic.ai

paths:
  /api/v1/extensions:
    get:
      summary: Search / list extensions
      parameters:
        - in: query
          name: kind
          schema: { enum: [design, bundle, deploy] }
        - in: query
          name: capability
          schema: { type: string }
        - in: query
          name: q
          schema: { type: string }
        - in: query
          name: page
          schema: { type: integer, default: 1 }
        - in: query
          name: limit
          schema: { type: integer, default: 20 }
      responses:
        "200":
          description: list
          content:
            application/json:
              schema:
                type: array
                items: { $ref: '#/components/schemas/ExtensionSummary' }

    post:
      summary: Publish an extension
      security:
        - bearerAuth: []
      requestBody:
        required: true
        content:
          multipart/form-data:
            schema:
              type: object
              properties:
                metadata:
                  type: string
                  description: JSON-encoded PublishRequest
                artifact:
                  type: string
                  format: binary
      responses:
        "201": { description: created }
        "401": { description: unauthorized }

  /api/v1/extensions/{name}:
    get:
      summary: Get extension overview + all versions
      parameters:
        - in: path
          name: name
          required: true
          schema: { type: string }
      responses:
        "200":
          description: ok
          content:
            application/json:
              schema: { $ref: '#/components/schemas/ExtensionDetail' }
        "404": { description: not found }

  /api/v1/extensions/{name}/{version}:
    get:
      summary: Get metadata for a specific version
      parameters:
        - in: path
          name: name
          required: true
          schema: { type: string }
        - in: path
          name: version
          required: true
          schema: { type: string }
      responses:
        "200":
          description: ok
          content:
            application/json:
              schema: { $ref: '#/components/schemas/ExtensionMetadata' }
        "404": { description: not found }

  /api/v1/extensions/{name}/{version}/artifact:
    get:
      summary: Download the raw .gtxpack artifact
      parameters:
        - in: path
          name: name
          required: true
          schema: { type: string }
        - in: path
          name: version
          required: true
          schema: { type: string }
      responses:
        "200":
          description: artifact bytes (application/zip)
          content:
            application/octet-stream:
              schema: { type: string, format: binary }
        "404": { description: not found }

  /api/v1/auth/login:
    post:
      summary: Exchange credentials for a token
      requestBody:
        required: true
        content:
          application/json:
            schema:
              type: object
              properties:
                email: { type: string }
                password: { type: string }
      responses:
        "200":
          description: token
          content:
            application/json:
              schema:
                type: object
                properties:
                  token: { type: string }

components:
  securitySchemes:
    bearerAuth:
      type: http
      scheme: bearer

  schemas:
    ExtensionSummary:
      type: object
      required: [name, latestVersion, kind, summary]
      properties:
        name: { type: string }
        latestVersion: { type: string }
        kind: { enum: [DesignExtension, BundleExtension, DeployExtension] }
        summary: { type: string }
        downloads: { type: integer, default: 0 }

    ExtensionDetail:
      type: object
      required: [name, versions]
      properties:
        name: { type: string }
        versions: { type: array, items: { type: string } }
        latestVersion: { type: string }
        kind: { enum: [DesignExtension, BundleExtension, DeployExtension] }

    ExtensionMetadata:
      type: object
      required: [describe, artifactSha256]
      properties:
        describe:
          type: object
          description: Full describe.json content
        artifactSha256: { type: string }
        publishedAt: { type: string, format: date-time }
        yanked: { type: boolean, default: false }
```

- [ ] **Step 2: Commit**

```bash
git add docs/greentic-store-api.openapi.yaml
git commit -m "docs: add Greentic Store OpenAPI 3.1 spec"
```

---

## Phase J — Final verification + milestone

### Task J.1: Full CI + tag

- [ ] Run: `bash ci/local_check.sh 2>&1 | tail -5`
  Expected: All checks passed

- [ ] Tag milestone:

```bash
git tag -a v0.2.0-cli-registry -m "CLI + 3 registry impls + install lifecycle"
```

- [ ] Report summary:
  - Total commits since `v0.1.0-foundation`
  - Total tests (workspace-wide)
  - `gtdx --help` output

---

## Self-review against spec

- [ ] Spec §8 (CLI subcommands) → Phases G + H
- [ ] Spec §9.1 (Registry trait + 3 impls) → Phases A-D
- [ ] Spec §9.2 (OpenAPI spec) → Phase I
- [ ] Spec §9.3 (Store hosts WASM) → implicit in Store client design
- [ ] Spec §9.4 (Auth + trust policy) → Phases E.2 + F
- [ ] Spec §7.3 install lifecycle → Phase E

Gaps for Plan 3/4:
- `gtdx new` / `init` (scaffolding for extension authors) — Plan 3 (needs an AC
  extension template to scaffold against)
- `gtdx build` (WASM component build) — Plan 3
- `gtdx publish` — partially in Phase C (Store.publish wired) but no CLI binding
  yet; add `commands/publish.rs` in Plan 3 alongside `gtdx build`
- `gtdx update` — incremental upgrade of installed extensions (Plan 3 or 4)

---

## Execution Handoff

Plan 2 saved to `docs/superpowers/plans/2026-04-17-cli-and-registry.md`.

Two execution options:

1. **Subagent-Driven (recommended)** — fresh subagent per task (or per phase)
2. **Inline Execution** — batch with checkpoints

Same pattern as Plan 1.
