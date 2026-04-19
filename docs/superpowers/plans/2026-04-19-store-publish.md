# Store Publish Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development to implement this plan task-by-task.

**Goal:** Enable `gtdx publish --registry <store-name>` to upload `.gtxpack` to a Greentic Store HTTP server via bearer-authenticated multipart POST, closing the last gap in the Phase 1 happy path (`gtdx new` → `gtdx dev` → `gtdx publish` to Store).

**Architecture:** Port the pre-Track-C multipart POST logic in `GreenticStoreRegistry::publish` to the new `PublishRequest → PublishReceipt` trait signature (Track C). `commands::publish::run_publish` gains a `resolve_registry_backend()` helper that returns a boxed `&dyn ExtensionRegistry`: `local`/`file://` → `LocalFilesystemRegistry`; otherwise look up in `~/.greentic/config.toml` and construct `GreenticStoreRegistry` with bearer token from `~/.greentic/credentials.toml` or env-var override.

**Tech Stack:** Rust 1.94, existing `reqwest` + `async-trait` + `serde_json` + `toml`, `greentic-ext-registry::{config, credentials}` (already shipped in Track A), `reqwest::multipart` (already used by `GreenticStoreRegistry`).

**Parent:** Phase 2 S5 subset — Store HTTP publish only, no OCI, no strict trust policy (those stay Phase 2 full).

---

## File Structure

### Modify

- `crates/greentic-ext-registry/src/store.rs` — replace `NotImplemented` stub with real multipart POST; add `PublishResponseDto` for receipt parsing.
- `crates/greentic-ext-cli/src/publish/mod.rs` — add `resolve_registry_backend()` + thread result into `run_publish`.
- `CHANGELOG.md` — entry for Store publish.
- `docs/getting-started-publish.md` — add "Publishing to the Greentic Store" section.

### No new files needed. Infrastructure (config/credentials loaders, CLI login + registries subcommands) already exists.

---

## Task 1: Rewrite GreenticStoreRegistry::publish

**File:** `crates/greentic-ext-registry/src/store.rs`

The current `publish()` returns `NotImplemented`. Replace with a real multipart POST to `/api/v1/extensions`.

### Step 1: Replace the `publish` override

Find the existing `async fn publish(...)` inside the `impl ExtensionRegistry for GreenticStoreRegistry` block and replace with:

```rust
    async fn publish(
        &self,
        req: crate::publish::PublishRequest,
    ) -> Result<crate::publish::PublishReceipt, RegistryError> {
        let token = self.token.as_deref().ok_or_else(|| {
            RegistryError::AuthRequired(format!(
                "no token configured for registry '{}'; run: gtdx login --registry {}",
                self.name, self.name
            ))
        })?;

        // Describe must be JCS-canonicalized so signatures (if present) round-trip.
        let describe_bytes = serde_json::to_vec(&req.describe)?;
        let metadata = PublishMetadata {
            ext_id: &req.ext_id,
            ext_name: &req.ext_name,
            version: &req.version,
            kind: req.kind,
            artifact_sha256: &req.artifact_sha256,
            describe: &serde_json::from_slice::<serde_json::Value>(&describe_bytes)?,
            signature: req.signature.as_ref(),
            force: req.force,
        };
        let metadata_json = serde_json::to_string(&metadata)?;

        let form = reqwest::multipart::Form::new()
            .text("metadata", metadata_json)
            .part(
                "artifact",
                reqwest::multipart::Part::bytes(req.artifact_bytes)
                    .file_name(format!("{}-{}.gtxpack", req.ext_name, req.version))
                    .mime_str("application/zip")
                    .map_err(|e| RegistryError::Storage(format!("mime: {e}")))?,
            );

        let resp = self
            .client
            .post(self.url("/api/v1/extensions"))
            .bearer_auth(token)
            .multipart(form)
            .send()
            .await?;

        let status = resp.status();
        if status == reqwest::StatusCode::UNAUTHORIZED {
            return Err(RegistryError::AuthRequired(format!(
                "401 from '{}'. Token expired? Re-run: gtdx login --registry {}",
                self.name, self.name
            )));
        }
        if status == reqwest::StatusCode::CONFLICT {
            let body = resp.text().await.unwrap_or_default();
            return Err(RegistryError::VersionExists {
                existing_sha: extract_existing_sha(&body).unwrap_or_else(|| "unknown".into()),
            });
        }
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(RegistryError::Storage(format!(
                "store publish failed: {status} {body}"
            )));
        }
        let dto: PublishResponseDto = resp.json().await?;
        Ok(crate::publish::PublishReceipt {
            url: dto.url.unwrap_or_else(|| {
                format!(
                    "{}/api/v1/extensions/{}/{}",
                    self.base_url.trim_end_matches('/'),
                    req.ext_id,
                    req.version
                )
            }),
            sha256: dto.artifact_sha256.unwrap_or_else(|| req.artifact_sha256.clone()),
            published_at: dto.published_at.unwrap_or_else(chrono::Utc::now),
            signed: req.signature.is_some(),
        })
    }
```

### Step 2: Add the supporting DTOs + helper

Append near the other `#[derive(Serialize)]` / `Deserialize` structs (top of file, before the `impl ExtensionRegistry`):

```rust
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct PublishMetadata<'a> {
    ext_id: &'a str,
    ext_name: &'a str,
    version: &'a str,
    kind: greentic_ext_contract::ExtensionKind,
    artifact_sha256: &'a str,
    describe: &'a serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    signature: Option<&'a crate::publish::SignatureBlob>,
    force: bool,
}

#[derive(Debug, Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct PublishResponseDto {
    url: Option<String>,
    artifact_sha256: Option<String>,
    published_at: Option<chrono::DateTime<chrono::Utc>>,
}

fn extract_existing_sha(body: &str) -> Option<String> {
    let v: serde_json::Value = serde_json::from_str(body).ok()?;
    v.get("existing_sha")
        .or_else(|| v.get("artifactSha256"))
        .and_then(|x| x.as_str())
        .map(str::to_string)
}
```

### Step 3: Verify build

Run: `cargo build -p greentic-ext-registry --quiet 2>&1 | tail -5`
Expected: exit 0.

Run: `cargo test -p greentic-ext-registry --quiet 2>&1 | tail -5`
Expected: all existing green.

### Step 4: Commit

```bash
git add crates/greentic-ext-registry/src/store.rs
git commit -m "feat(ext-registry): GreenticStoreRegistry::publish real multipart POST with PublishRequest"
```

---

## Task 2: Unit test GreenticStoreRegistry::publish with wiremock

**Files:**
- Modify: `crates/greentic-ext-registry/Cargo.toml` — add `wiremock = "0.6"` under `[dev-dependencies]`.
- Create: `crates/greentic-ext-registry/tests/store_publish.rs`.

### Step 1: Add dev-dep

Edit `crates/greentic-ext-registry/Cargo.toml`, under `[dev-dependencies]`:

```toml
wiremock = "0.6"
tokio = { workspace = true, features = ["macros", "rt-multi-thread"] }
```

(`tokio` may already be a regular dep; if so, skip.)

### Step 2: Write tests

```rust
use chrono::Utc;
use greentic_ext_contract::{
    DescribeJson, ExtensionKind,
    describe::{Author, Capabilities, Engine, Metadata, Permissions, Runtime},
};
use greentic_ext_registry::publish::PublishRequest;
use greentic_ext_registry::registry::ExtensionRegistry;
use greentic_ext_registry::store::GreenticStoreRegistry;
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn sample_req() -> PublishRequest {
    PublishRequest {
        ext_id: "com.example.demo".into(),
        ext_name: "demo".into(),
        version: "0.1.0".into(),
        kind: ExtensionKind::Design,
        artifact_bytes: b"fake-pack".to_vec(),
        artifact_sha256: "abc".into(),
        describe: DescribeJson {
            schema_ref: None,
            api_version: "greentic.ai/v1".into(),
            kind: ExtensionKind::Design,
            metadata: Metadata {
                id: "com.example.demo".into(),
                name: "demo".into(),
                version: "0.1.0".into(),
                summary: "s".into(),
                description: None,
                author: Author { name: "a".into(), email: None, public_key: None },
                license: "MIT".into(),
                homepage: None,
                repository: None,
                keywords: vec![],
                icon: None,
                screenshots: vec![],
            },
            engine: Engine {
                greentic_designer: "^0.1".into(),
                ext_runtime: "^0.1".into(),
            },
            capabilities: Capabilities { offered: vec![], required: vec![] },
            runtime: Runtime {
                component: "extension.wasm".into(),
                memory_limit_mb: 64,
                permissions: Permissions::default(),
            },
            contributions: serde_json::json!({}),
            signature: None,
        },
        signature: None,
        force: false,
    }
}

#[tokio::test]
async fn publish_without_token_returns_auth_required() {
    let server = MockServer::start().await;
    let reg = GreenticStoreRegistry::new("store", server.uri(), None);
    let err = reg.publish(sample_req()).await.unwrap_err();
    assert!(matches!(err, greentic_ext_registry::RegistryError::AuthRequired(_)));
}

#[tokio::test]
async fn publish_success_parses_receipt() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/v1/extensions"))
        .and(header("authorization", "Bearer test-token"))
        .respond_with(ResponseTemplate::new(201).set_body_json(serde_json::json!({
            "url": "https://store.example.com/api/v1/extensions/com.example.demo/0.1.0",
            "artifactSha256": "abc",
            "publishedAt": Utc::now().to_rfc3339(),
        })))
        .mount(&server)
        .await;
    let reg = GreenticStoreRegistry::new("store", server.uri(), Some("test-token".into()));
    let receipt = reg.publish(sample_req()).await.unwrap();
    assert_eq!(receipt.sha256, "abc");
    assert!(receipt.url.contains("com.example.demo"));
    assert!(!receipt.signed);
}

#[tokio::test]
async fn publish_401_maps_to_auth_required() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/v1/extensions"))
        .respond_with(ResponseTemplate::new(401).set_body_json(serde_json::json!({
            "code": "unauthorized",
            "error": "unauthorized"
        })))
        .mount(&server)
        .await;
    let reg = GreenticStoreRegistry::new("store", server.uri(), Some("bad".into()));
    let err = reg.publish(sample_req()).await.unwrap_err();
    assert!(matches!(err, greentic_ext_registry::RegistryError::AuthRequired(_)));
}

#[tokio::test]
async fn publish_409_maps_to_version_exists() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/v1/extensions"))
        .respond_with(ResponseTemplate::new(409).set_body_json(serde_json::json!({
            "existing_sha": "prev-sha"
        })))
        .mount(&server)
        .await;
    let reg = GreenticStoreRegistry::new("store", server.uri(), Some("tok".into()));
    let err = reg.publish(sample_req()).await.unwrap_err();
    match err {
        greentic_ext_registry::RegistryError::VersionExists { existing_sha } => {
            assert_eq!(existing_sha, "prev-sha");
        }
        other => panic!("expected VersionExists, got {other:?}"),
    }
}
```

### Step 3: Run

Run: `cargo test -p greentic-ext-registry --test store_publish 2>&1 | tail -10`
Expected: 4 passed.

### Step 4: Commit

```bash
git add crates/greentic-ext-registry/Cargo.toml crates/greentic-ext-registry/tests/store_publish.rs
git commit -m "test(ext-registry): wiremock GreenticStoreRegistry::publish — auth, success, 401, 409"
```

---

## Task 3: Backend resolver in publish orchestrator

**File:** `crates/greentic-ext-cli/src/publish/mod.rs`

Replace the local-only `resolve_registry_root` with a backend resolver that returns either `LocalFilesystemRegistry` or `GreenticStoreRegistry`. Thread through `run_publish`.

### Step 1: Add resolver + new enum

Add near the top of `publish/mod.rs` (below the existing `use` imports):

```rust
use greentic_ext_registry::config::GtdxConfig;
use greentic_ext_registry::credentials::Credentials;
use greentic_ext_registry::store::GreenticStoreRegistry;

enum Backend {
    Local(LocalFilesystemRegistry),
    Store(GreenticStoreRegistry),
}

impl Backend {
    async fn publish(
        &self,
        req: greentic_ext_registry::publish::PublishRequest,
    ) -> Result<greentic_ext_registry::publish::PublishReceipt, greentic_ext_registry::RegistryError>
    {
        match self {
            Backend::Local(r) => r.publish(req).await,
            Backend::Store(r) => r.publish(req).await,
        }
    }

    fn display_url(&self, ext_id: &str, version: &str) -> String {
        match self {
            Backend::Local(r) => format!("file://{}", r.root_path().display()),
            Backend::Store(r) => format!(
                "{}/api/v1/extensions/{ext_id}/{version}",
                r.base_url().trim_end_matches('/')
            ),
        }
    }
}

fn resolve_backend(uri: &str, home: &Path) -> anyhow::Result<Backend> {
    // Local cases first.
    if uri == "local" {
        let root = home.join("registries/local");
        return Ok(Backend::Local(LocalFilesystemRegistry::new(
            "publish-local",
            root,
        )));
    }
    if let Some(rest) = uri.strip_prefix("file://") {
        let root = std::path::PathBuf::from(rest);
        return Ok(Backend::Local(LocalFilesystemRegistry::new("file", root)));
    }

    // Look up in config.toml by name.
    let cfg = GtdxConfig::default();
    let cfg = GtdxConfig::default_or_load(home).unwrap_or(cfg);
    let entry = cfg
        .registries
        .iter()
        .find(|e| e.name == uri)
        .ok_or_else(|| {
            anyhow::anyhow!(
                "no registry named '{uri}' in {}/config.toml. Add one with: gtdx registries add {uri} <url>",
                home.display()
            )
        })?;

    let token = resolve_token(home, entry);
    Ok(Backend::Store(GreenticStoreRegistry::new(
        &entry.name,
        &entry.url,
        token,
    )))
}

fn resolve_token(home: &Path, entry: &greentic_ext_registry::config::RegistryEntry) -> Option<String> {
    // Priority: explicit env override (token-env in config), then credentials.toml.
    if let Some(var) = &entry.token_env
        && let Ok(v) = std::env::var(var)
        && !v.is_empty()
    {
        return Some(v);
    }
    let creds = Credentials::load(&home.join("credentials.toml")).ok()?;
    creds.get(&entry.name).map(str::to_string)
}
```

### Step 2: Add `default_or_load` helper on GtdxConfig if missing

Check if `greentic_ext_registry::config::load(path)` exists. It does. Since there's no `default_or_load`, simplify by inlining:

Replace the `let cfg = GtdxConfig::default_or_load(home).unwrap_or(cfg);` line with:

```rust
    let cfg_path = home.join("config.toml");
    let cfg = greentic_ext_registry::config::load(&cfg_path)
        .map_err(|e| anyhow::anyhow!("load config: {e}"))?;
```

And delete the two lines above `let cfg = GtdxConfig::default();` block (the unused `cfg` default). Final resolver should be:

```rust
fn resolve_backend(uri: &str, home: &Path) -> anyhow::Result<Backend> {
    if uri == "local" {
        let root = home.join("registries/local");
        return Ok(Backend::Local(LocalFilesystemRegistry::new(
            "publish-local",
            root,
        )));
    }
    if let Some(rest) = uri.strip_prefix("file://") {
        let root = std::path::PathBuf::from(rest);
        return Ok(Backend::Local(LocalFilesystemRegistry::new("file", root)));
    }

    let cfg = greentic_ext_registry::config::load(&home.join("config.toml"))
        .map_err(|e| anyhow::anyhow!("load config: {e}"))?;
    let entry = cfg
        .registries
        .iter()
        .find(|e| e.name == uri)
        .ok_or_else(|| {
            anyhow::anyhow!(
                "no registry named '{uri}' in {}/config.toml. Add one with: gtdx registries add {uri} <url>",
                home.display()
            )
        })?;

    let token = resolve_token(home, entry);
    Ok(Backend::Store(GreenticStoreRegistry::new(
        &entry.name,
        &entry.url,
        token,
    )))
}
```

### Step 3: Expose `base_url()` on GreenticStoreRegistry

In `crates/greentic-ext-registry/src/store.rs`, add inside the `impl GreenticStoreRegistry { ... }` block:

```rust
    #[must_use]
    pub fn base_url(&self) -> &str {
        &self.base_url
    }
```

### Step 4: Refactor run_publish to use Backend

Replace the existing body of `run_publish` wherever it constructs `LocalFilesystemRegistry` directly. The current orchestrator:

```rust
    let registry_root = resolve_registry_root(&cfg.registry_uri, &cfg.home)?;
    let registry = LocalFilesystemRegistry::new("publish-local", registry_root.clone());
    // ... uses registry.publish(req) and registry_root for verify_only
```

Replace with:

```rust
    let backend = resolve_backend(&cfg.registry_uri, &cfg.home)?;
    if cfg.verify_only {
        return verify_only(&backend, &describe, cfg.force);
    }
```

Move the verify_only body into its own fn:

```rust
fn verify_only(
    backend: &Backend,
    describe: &DescribeJson,
    force: bool,
) -> anyhow::Result<PublishOutcome> {
    match backend {
        Backend::Local(r) => {
            let ver_dir = r
                .root_path()
                .join(&describe.metadata.id)
                .join(&describe.metadata.version);
            if ver_dir.exists() && !force {
                anyhow::bail!(
                    "version {} already exists at {}",
                    describe.metadata.version,
                    ver_dir.display()
                );
            }
            Ok(PublishOutcome::VerifyOnly {
                ext_id: describe.metadata.id.clone(),
                version: describe.metadata.version.clone(),
                registry: r.root_path().display().to_string(),
            })
        }
        Backend::Store(_) => {
            // Server-side conflict check lands here in a future iteration;
            // for now, verify_only on a remote registry is a no-op success.
            Ok(PublishOutcome::VerifyOnly {
                ext_id: describe.metadata.id.clone(),
                version: describe.metadata.version.clone(),
                registry: backend.display_url(&describe.metadata.id, &describe.metadata.version),
            })
        }
    }
}
```

Swap the `registry.publish(req).await` call for:

```rust
    let receipt = backend
        .publish(req)
        .await
        .map_err(|e| anyhow::anyhow!("publish: {e}"))?;
```

Remove the old `resolve_registry_root` fn — it's superseded.

### Step 5: Verify build + tests

Run: `cargo build -p greentic-ext-cli 2>&1 | tail -10`
Expected: exit 0.

Run: `cargo test -p greentic-ext-cli --bins 2>&1 | tail -5`
Expected: 55 passed.

### Step 6: Commit

```bash
git add crates/greentic-ext-cli/src/publish/mod.rs crates/greentic-ext-registry/src/store.rs
git commit -m "feat(ext-cli): publish backend resolver — local/file:// vs Store via config.toml + credentials.toml"
```

---

## Task 4: Docs + CHANGELOG

**Files:**
- Modify: `CHANGELOG.md`
- Modify: `docs/getting-started-publish.md`

### Step 1: CHANGELOG

Under Unreleased → Added:

```
- `gtdx publish --registry <name>` now uploads `.gtxpack` artifacts to a
  Greentic Store HTTP server via multipart POST to `/api/v1/extensions` with
  bearer-token auth. Registry URL is resolved from `~/.greentic/config.toml`
  (add with `gtdx registries add <name> <url>`); token is read from
  `~/.greentic/credentials.toml` (`gtdx login --registry <name>`) or the
  env-var named in the registry's `token-env` entry. 401 → `AuthRequired`
  with actionable hint; 409 → `VersionExists`; 2xx → parsed `PublishReceipt`.
```

### Step 2: getting-started-publish.md

Append a new section after the existing "Flags" table:

```markdown
## Publishing to the Greentic Store

`gtdx publish --registry local` writes to the local filesystem. To push to a
Store HTTP server:

1. Register the Store URL once:

   ```bash
   gtdx registries add mystore https://store.example.com
   ```

2. Log in (saves a bearer token at `~/.greentic/credentials.toml` with
   mode 0600):

   ```bash
   gtdx login --registry mystore
   # paste the JWT when prompted
   ```

3. Publish:

   ```bash
   gtdx publish --registry mystore
   ```

Token resolution order on publish:

1. Env var named in the registry's `token-env` entry (configured via
   `gtdx registries add <name> <url> --token-env MYSTORE_TOKEN`).
2. `~/.greentic/credentials.toml` entry for the registry name.
3. None → `gtdx publish` aborts with an `AuthRequired` hint.

Publisher handles and allowed-prefix policies are enforced server-side;
you can only publish extensions whose `metadata.id` matches a prefix
allowed for your account.
```

### Step 3: Commit

```bash
git add CHANGELOG.md docs/getting-started-publish.md
git commit -m "docs: Store publish workflow + CHANGELOG entry"
```

---

## Task 5: Author-run smoke against live server

Controller runs this directly.

- [ ] **Step 1: Build**

```bash
cargo build -p greentic-ext-cli --quiet
```

- [ ] **Step 2: Setup**

```bash
TMP=$(mktemp -d); export GREENTIC_HOME="$TMP/home"
./target/debug/gtdx registries add smoke http://62.171.174.152:3030
```

Then save the stored JWT (from earlier register/login step) manually to
`$GREENTIC_HOME/credentials.toml`:

```toml
[tokens]
smoke = "eyJ0eXA...<PASTE-FROM-LOGIN>"
```

- [ ] **Step 3: Scaffold + publish**

Use an id that matches the publisher's allowed prefix (`gtdx-smoke-<ts>.`):

```bash
./target/debug/gtdx new mytest \
  --id gtdx-smoke-1776568049.mytest \
  --dir "$TMP/mytest" \
  --author tester -y --no-git
./target/debug/gtdx publish --registry smoke \
  --manifest "$TMP/mytest/Cargo.toml" \
  --dist "$TMP/mytest/dist"
```

Expected: `✓ published gtdx-smoke-1776568049.mytest@0.1.0` with URL pointing at
`http://62.171.174.152:3030/api/v1/extensions/...`.

- [ ] **Step 4: Verify via API**

```bash
curl -sS http://62.171.174.152:3030/api/v1/extensions/gtdx-smoke-1776568049.mytest
```

Expected: JSON with the published version.

- [ ] **Step 5: No commit — record observations in PR**

---

## Task 6: Final gate + PR

Controller runs this directly.

- [ ] **Step 1: Format** — `cargo fmt --all`
- [ ] **Step 2: Clippy** — `cargo clippy --workspace --all-targets --all-features -- -D warnings 2>&1 | tail -20`; exit 0.
- [ ] **Step 3: Full test** — `cargo test --workspace --all-targets 2>&1 | tail -20`; all green.
- [ ] **Step 4: Commit stragglers** — `git add -A && git commit -m "style: cargo fmt post-store-publish"` if diff non-empty.
- [ ] **Step 5: Push + PR**.

---

## Acceptance

1. `gtdx publish --registry <name>` uploads via multipart POST to the Store's `/api/v1/extensions` with bearer auth (Task 1).
2. Missing token → `AuthRequired` error with hint to run `gtdx login` (Task 1, test Task 2).
3. 401 response → `AuthRequired` (Task 2 test).
4. 409 response → `VersionExists { existing_sha }` (Task 2 test).
5. 2xx response → parsed `PublishReceipt` with server-provided URL (Task 2 test).
6. `gtdx publish --registry local` still works unchanged (Task 3 non-regression).
7. Token resolution honors `token-env` first, then `credentials.toml` (Task 3).
8. Full workspace fmt + clippy + test green (Task 6).
9. Live server smoke uploads `.gtxpack` successfully to `62.171.174.152:3030` (Task 5).

---

## Self-Review

**1. Spec coverage vs goal:** The goal says "close the Phase 1 happy path". Tasks 1-3 wire publish to Store; Task 5 proves it works against the real server. Local-filesystem publish (Track C) is untouched. ✓

**2. Placeholder scan:** No TBD/TODO in task bodies.

**3. Type consistency:** `Backend` enum variants `Local`/`Store` used identically in Tasks 3 + 3. `PublishMetadata` DTO (Task 1) mirrors the keys the server expects per OpenAPI spec. `PublishResponseDto` fields are all optional with fallback to computed values — tolerates partial server responses.

**4. Known deferrals:**
- `Backend::Store::verify_only` is a no-op success — proper server-side conflict check (e.g., `HEAD /api/v1/extensions/<id>/<version>`) is a follow-up.
- Token management: no refresh flow (JWT expires in 24h per server probe). User re-runs `gtdx login`.
- `--format json` still human-only (plan-wide deferral).
- OCI publish stays `NotImplemented` — out of scope.
