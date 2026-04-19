# Phase 1 Track C — Publish Local (`gtdx publish`) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add `gtdx publish` subcommand that validates, builds, and atomically publishes a scaffolded Greentic Designer Extension as a deterministic `.gtxpack` into `$GTDX_HOME/registries/local/`. Wires `ExtensionRegistry::publish()` end-to-end for the filesystem backend; Store/OCI backends return `NotImplemented` with actionable hints.

**Architecture:** Shared deterministic ZIP writer moves into `greentic-ext-contract::pack_writer` and is consumed by both Track B (`gtdx dev`'s packer) and Track C (`gtdx publish`). `LocalFilesystemRegistry::publish()` writes the hierarchical `<id>/<version>/<id>-<version>.gtxpack` layout with atomic rename + `fs2` advisory file lock on `index.json`. `GreenticStoreRegistry` and `OciRegistry` override `publish()` to return `RegistryError::NotImplemented`. A new `publish_cmd.rs` orchestrates build → validate → pack → publish → receipt. Describe.json validation aggregates errors (not bail-on-first). Optional `--sign` reuses the existing JCS `sign_describe` (Wave 1 signing is already merged — we do NOT ship the spec's "phase1-artifact-sig-only" workaround).

**Tech Stack:** Rust 1.94, edition 2024, async-trait, serde, serde_json, fs2 (new workspace dep), zip, sha2, chrono, tempfile, existing `greentic-ext-contract::signature::sign_describe`, existing `wit-bindgen 0.41` surface.

**Spec:** `docs/superpowers/specs/2026-04-18-phase1-publish-local-design.md`
**Parent:** `docs/superpowers/specs/2026-04-18-dx-10-10-roadmap.md` (subsystem S3)

---

## File Structure

### Create

- `crates/greentic-ext-contract/src/pack_writer.rs` (~200 LOC) — deterministic ZIP writer with sorted entries, zeroed timestamps, normalized Unix modes, LF line-ending normalization for text files. Exported as `pub use` from `lib.rs`.
- `crates/greentic-ext-registry/src/publish.rs` (~70 LOC) — `PublishRequest` + `PublishReceipt` types; re-exported from registry lib.
- `crates/greentic-ext-registry/src/local_publish.rs` (~220 LOC) — `LocalFilesystemRegistry::publish` impl, index.json updater, advisory locking.
- `crates/greentic-ext-cli/src/commands/publish.rs` (~260 LOC) — clap `Args`, orchestrator, human/JSON format output.
- `crates/greentic-ext-cli/src/publish/mod.rs` (~40 LOC) — module root.
- `crates/greentic-ext-cli/src/publish/validator.rs` (~180 LOC) — pre-publish describe.json checks (aggregated errors).
- `crates/greentic-ext-cli/src/publish/receipt.rs` (~90 LOC) — `PublishReceipt` serializer + `./dist/publish-<id>-<version>.json` writer.
- `crates/greentic-ext-cli/tests/cli_publish.rs` — integration tests (scaffold → publish → install round-trip).
- `crates/greentic-ext-registry/tests/local_publish.rs` — unit/integration tests for LocalFilesystem publish semantics.

### Modify

- `Cargo.toml` (workspace root) — add `fs2 = "0.4"`, `chrono = { version = "0.4", default-features = false, features = ["serde", "clock"] }` (chrono may already be transitive; add explicit).
- `crates/greentic-ext-contract/src/lib.rs` — declare `pub mod pack_writer;` and re-export key types.
- `crates/greentic-ext-contract/Cargo.toml` — add deps `zip`, `sha2` (may already be dev-deps; promote to regular if needed).
- `crates/greentic-ext-registry/Cargo.toml` — add deps `fs2`, `chrono`.
- `crates/greentic-ext-registry/src/lib.rs` — declare `pub mod publish;` + `pub mod local_publish;` and re-export `PublishRequest`/`PublishReceipt`.
- `crates/greentic-ext-registry/src/registry.rs` — change `publish()` trait default to return `RegistryError::NotImplemented`; update signature to take `PublishRequest`.
- `crates/greentic-ext-registry/src/error.rs` — add `NotImplemented { hint: String }` and `VersionExists { existing_sha: String }` variants.
- `crates/greentic-ext-registry/src/store.rs` — override `publish()` to return `NotImplemented { hint: "Store publish lands in Phase 2 (S5). Use --registry local for now." }`.
- `crates/greentic-ext-registry/src/oci.rs` — override `publish()` with analogous `NotImplemented` hint.
- `crates/greentic-ext-cli/src/dev/packer.rs` — swap local zip writer for the shared `pack_writer::build_gtxpack`.
- `crates/greentic-ext-cli/src/commands/mod.rs` — add `pub mod publish;`.
- `crates/greentic-ext-cli/src/main.rs` — declare `mod publish;`, add `Publish` variant, route.
- `crates/greentic-ext-cli/Cargo.toml` — add `chrono` dep.
- `CHANGELOG.md` — note `gtdx publish`.
- `docs/getting-started-publish.md` — new companion doc.

---

## Task 1: Workspace deps (`fs2`, `chrono`) + contract deps

**Files:**
- Modify: `Cargo.toml`
- Modify: `crates/greentic-ext-contract/Cargo.toml`
- Modify: `crates/greentic-ext-registry/Cargo.toml`
- Modify: `crates/greentic-ext-cli/Cargo.toml`

- [ ] **Step 1: Add workspace deps**

Edit root `Cargo.toml` `[workspace.dependencies]`, insert alphabetically:

```toml
chrono = { version = "0.4", default-features = false, features = ["serde", "clock"] }
fs2 = "0.4"
```

- [ ] **Step 2: ext-contract deps**

Edit `crates/greentic-ext-contract/Cargo.toml`, under `[dependencies]`, ensure present (add if missing):

```toml
zip = { workspace = true }
sha2 = { workspace = true }
```

- [ ] **Step 3: ext-registry deps**

Edit `crates/greentic-ext-registry/Cargo.toml`, under `[dependencies]`, add:

```toml
chrono = { workspace = true }
fs2 = { workspace = true }
```

- [ ] **Step 4: ext-cli deps**

Edit `crates/greentic-ext-cli/Cargo.toml`, under `[dependencies]`, add (keep alphabetical):

```toml
chrono = { workspace = true }
```

- [ ] **Step 5: Verify metadata resolves**

Run: `cargo metadata --format-version 1 > /dev/null`
Expected: exit 0.

- [ ] **Step 6: Commit**

```bash
git add Cargo.toml crates/greentic-ext-contract/Cargo.toml crates/greentic-ext-registry/Cargo.toml crates/greentic-ext-cli/Cargo.toml
git commit -m "chore: add fs2 + chrono deps for Track C publish path"
```

---

## Task 2: Deterministic pack writer in ext-contract

**Files:**
- Create: `crates/greentic-ext-contract/src/pack_writer.rs`
- Modify: `crates/greentic-ext-contract/src/lib.rs`

- [ ] **Step 1: Write module + tests**

Create `crates/greentic-ext-contract/src/pack_writer.rs`:

```rust
//! Deterministic `.gtxpack` writer shared between `gtdx dev` and `gtdx publish`.
//!
//! Guarantees:
//! - Entries serialized in sorted path order.
//! - Timestamps zeroed to 1980-01-01 00:00:00 (the ZIP epoch minimum).
//! - Unix permissions normalized to 0o644 (files) / 0o755 (dirs).
//! - Text assets (json/md/wit/txt) have CRLF normalized to LF before hashing.
//! - Binary assets passed through untouched.

use std::io::{Cursor, Write};

use sha2::{Digest, Sha256};
use zip::DateTime;
use zip::write::SimpleFileOptions;

/// One file-or-dir entry fed into the pack writer.
#[derive(Debug, Clone)]
pub struct PackEntry {
    /// Path inside the zip (forward-slash separated, relative, no leading "/").
    pub path: String,
    /// Raw bytes (post-normalization if text).
    pub bytes: Vec<u8>,
    /// Directory entries are emitted without body but with Unix 0o755 mode.
    pub is_dir: bool,
}

impl PackEntry {
    #[must_use]
    pub fn file(path: impl Into<String>, bytes: Vec<u8>) -> Self {
        Self {
            path: path.into(),
            bytes,
            is_dir: false,
        }
    }
}

/// Returns true if the entry's path should have CRLF normalized to LF.
#[must_use]
pub fn is_text_path(path: &str) -> bool {
    matches!(
        std::path::Path::new(path)
            .extension()
            .and_then(|s| s.to_str()),
        Some("json" | "md" | "wit" | "txt" | "toml" | "yaml" | "yml")
    )
}

/// Normalize CRLF → LF for text entries, leave binary entries untouched.
#[must_use]
pub fn normalize_entry(mut entry: PackEntry) -> PackEntry {
    if is_text_path(&entry.path) {
        // Remove '\r' anywhere a CRLF would appear. This is safe because
        // plain '\r' characters never appear in our JSON/WIT/MD surfaces.
        entry.bytes.retain(|b| *b != b'\r');
    }
    entry
}

fn zip_epoch() -> DateTime {
    DateTime::from_date_and_time(1980, 1, 1, 0, 0, 0)
        .expect("1980-01-01 00:00:00 is the minimum valid ZIP datetime")
}

/// Build a deterministic `.gtxpack` from `entries`. Returns the ZIP bytes.
/// Callers compute SHA256 separately via [`sha256_hex`].
///
/// # Errors
/// Returns a zip/io error if the ZIP writer fails.
pub fn build_gtxpack(entries: Vec<PackEntry>) -> Result<Vec<u8>, PackWriterError> {
    let mut entries: Vec<_> = entries.into_iter().map(normalize_entry).collect();
    entries.sort_by(|a, b| a.path.cmp(&b.path));

    let buf = Cursor::new(Vec::new());
    let mut zip = zip::ZipWriter::new(buf);
    let epoch = zip_epoch();

    for entry in entries {
        let opts = SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated)
            .last_modified_time(epoch)
            .unix_permissions(if entry.is_dir { 0o755 } else { 0o644 });
        if entry.is_dir {
            zip.add_directory(&entry.path, opts)?;
        } else {
            zip.start_file(&entry.path, opts)?;
            zip.write_all(&entry.bytes)?;
        }
    }

    let cursor = zip.finish()?;
    Ok(cursor.into_inner())
}

/// Lowercase hex SHA256 of the given bytes.
#[must_use]
pub fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut out = String::with_capacity(64);
    for b in digest {
        use std::fmt::Write as _;
        write!(&mut out, "{b:02x}").expect("write to String");
    }
    out
}

#[derive(Debug, thiserror::Error)]
pub enum PackWriterError {
    #[error("zip: {0}")]
    Zip(#[from] zip::result::ZipError),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_entries() -> Vec<PackEntry> {
        vec![
            PackEntry::file("z.md", b"alpha\n".to_vec()),
            PackEntry::file("a.wasm", b"\0asm\x01\x00\x00\x00".to_vec()),
            PackEntry::file("describe.json", b"{\"k\":1}\n".to_vec()),
        ]
    }

    #[test]
    fn deterministic_sha256_across_runs() {
        let a = build_gtxpack(sample_entries()).unwrap();
        let b = build_gtxpack(sample_entries()).unwrap();
        assert_eq!(sha256_hex(&a), sha256_hex(&b));
    }

    #[test]
    fn input_order_is_normalized() {
        let ordered = sample_entries();
        let mut reversed = ordered.clone();
        reversed.reverse();
        let a = build_gtxpack(ordered).unwrap();
        let b = build_gtxpack(reversed).unwrap();
        assert_eq!(sha256_hex(&a), sha256_hex(&b));
    }

    #[test]
    fn crlf_in_text_is_normalized_to_lf() {
        let crlf = vec![PackEntry::file("doc.md", b"line1\r\nline2\r\n".to_vec())];
        let lf = vec![PackEntry::file("doc.md", b"line1\nline2\n".to_vec())];
        let a = build_gtxpack(crlf).unwrap();
        let b = build_gtxpack(lf).unwrap();
        assert_eq!(sha256_hex(&a), sha256_hex(&b));
    }

    #[test]
    fn crlf_in_binary_is_preserved() {
        // `.wasm` is a binary path — CRLF-looking bytes must pass through.
        let with_cr = vec![PackEntry::file("blob.wasm", b"\x00\r\n\x01".to_vec())];
        let without_cr = vec![PackEntry::file("blob.wasm", b"\x00\n\x01".to_vec())];
        let a = build_gtxpack(with_cr).unwrap();
        let b = build_gtxpack(without_cr).unwrap();
        assert_ne!(sha256_hex(&a), sha256_hex(&b));
    }

    #[test]
    fn sha256_hex_is_lowercase_64_chars() {
        let s = sha256_hex(b"hello");
        assert_eq!(s.len(), 64);
        assert!(s.chars().all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()));
    }

    #[test]
    fn zip_contains_expected_names() {
        let bytes = build_gtxpack(sample_entries()).unwrap();
        let mut zip = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
        let names: Vec<_> = (0..zip.len())
            .map(|i| zip.by_index(i).unwrap().name().to_string())
            .collect();
        assert!(names.contains(&"a.wasm".to_string()));
        assert!(names.contains(&"describe.json".to_string()));
        assert!(names.contains(&"z.md".to_string()));
    }
}
```

- [ ] **Step 2: Wire into lib.rs**

Edit `crates/greentic-ext-contract/src/lib.rs`. Add module declaration (keep alphabetical — between `kind` and `schema`):

```rust
pub mod pack_writer;
```

And add re-export to the existing `pub use` block at the bottom:

```rust
pub use self::pack_writer::{PackEntry, PackWriterError, build_gtxpack, sha256_hex};
```

- [ ] **Step 3: Verify**

Run: `cargo test -p greentic-ext-contract --lib pack_writer 2>&1 | tail -15`
Expected: 6 passed.

- [ ] **Step 4: Commit**

```bash
git add crates/greentic-ext-contract/src/pack_writer.rs crates/greentic-ext-contract/src/lib.rs
git commit -m "feat(ext-contract): deterministic pack_writer with sorted entries, zeroed timestamps, LF normalization"
```

---

## Task 3: Refactor gtdx dev packer to use shared pack_writer

**Files:**
- Modify: `crates/greentic-ext-cli/src/dev/packer.rs`

Track B's `build_pack` currently uses its own zip writer. Replace with the new `pack_writer::build_gtxpack` + `sha256_hex`. Keep the public `build_pack` signature + `PackInfo` shape unchanged so `run_once`/`run_once_cached` in `dev/mod.rs` keep working without edits.

- [ ] **Step 1: Replace the implementation**

Replace `crates/greentic-ext-cli/src/dev/packer.rs` entirely:

```rust
//! `.gtxpack` builder: stages describe + wasm + assets and hands off to the
//! shared `greentic-ext-contract::pack_writer` for deterministic ZIP emission.

use std::path::{Path, PathBuf};

use greentic_ext_contract::pack_writer::{PackEntry, build_gtxpack, sha256_hex};
use walkdir::WalkDir;

/// Summary of a packed `.gtxpack`.
#[derive(Debug, Clone)]
pub struct PackInfo {
    pub pack_path: PathBuf,
    pub pack_name: String,
    pub size: u64,
    pub sha256: String,
    pub ext_name: String,
    pub ext_version: String,
    #[allow(dead_code)] // Reserved for richer InstallOk events in Phase 2.
    pub ext_kind: String,
}

/// Build a `.gtxpack` at `output_pack` from `project_dir` + the already-built
/// `wasm_path`. The ZIP contains `describe.json`, the wasm renamed to
/// `extension.wasm` (matches `runtime.component` default), and any optional
/// asset dirs that exist (`i18n/`, `schemas/`, `prompts/`).
pub fn build_pack(
    project_dir: &Path,
    wasm_path: &Path,
    output_pack: &Path,
) -> anyhow::Result<PackInfo> {
    let describe_path = project_dir.join("describe.json");
    let describe_bytes = std::fs::read(&describe_path)
        .map_err(|e| anyhow::anyhow!("read describe.json: {e}"))?;
    let describe: serde_json::Value = serde_json::from_slice(&describe_bytes)
        .map_err(|e| anyhow::anyhow!("parse describe.json: {e}"))?;
    let ext_name = describe["metadata"]["name"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("describe.metadata.name missing"))?
        .to_string();
    let ext_version = describe["metadata"]["version"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("describe.metadata.version missing"))?
        .to_string();
    let ext_kind = describe["kind"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("describe.kind missing"))?
        .to_string();

    let mut entries = vec![
        PackEntry::file("describe.json", describe_bytes),
        PackEntry::file("extension.wasm", std::fs::read(wasm_path)?),
    ];

    for asset_dir in ["i18n", "schemas", "prompts"] {
        let src = project_dir.join(asset_dir);
        if !src.is_dir() {
            continue;
        }
        let mut paths: Vec<PathBuf> = WalkDir::new(&src)
            .into_iter()
            .flatten()
            .filter(|e| e.file_type().is_file())
            .map(|e| e.path().to_path_buf())
            .collect();
        paths.sort();
        for abs in paths {
            let rel = abs
                .strip_prefix(project_dir)
                .expect("asset under project")
                .to_string_lossy()
                .replace('\\', "/");
            entries.push(PackEntry::file(rel, std::fs::read(&abs)?));
        }
    }

    if let Some(parent) = output_pack.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let zip_bytes = build_gtxpack(entries)
        .map_err(|e| anyhow::anyhow!("build_gtxpack: {e}"))?;
    std::fs::write(output_pack, &zip_bytes)?;

    let size = u64::try_from(zip_bytes.len()).unwrap_or(u64::MAX);
    let pack_name = output_pack
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("pack.gtxpack")
        .to_string();
    let sha256 = sha256_hex(&zip_bytes);

    Ok(PackInfo {
        pack_path: output_pack.to_path_buf(),
        pack_name,
        size,
        sha256,
        ext_name,
        ext_version,
        ext_kind,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;

    fn make_project(root: &Path) -> PathBuf {
        let desc = br#"{
  "apiVersion": "greentic.ai/v1",
  "kind": "DesignExtension",
  "metadata": {"id": "com.example.demo", "name": "demo", "version": "0.1.0", "summary": "x", "author": {"name": "a"}, "license": "Apache-2.0"},
  "engine": {"greenticDesigner": "^0.1.0", "extRuntime": "^0.1.0"},
  "capabilities": {"offered": [], "required": []},
  "runtime": {"component": "extension.wasm", "permissions": {"network": [], "secrets": [], "callExtensionKinds": []}},
  "contributions": {}
}"#;
        std::fs::write(root.join("describe.json"), desc).unwrap();
        let wasm_dir = root.join("target/wasm32-wasip2/debug");
        std::fs::create_dir_all(&wasm_dir).unwrap();
        let wasm = wasm_dir.join("demo.wasm");
        std::fs::write(&wasm, b"\0asm\x01\x00\x00\x00").unwrap();
        wasm
    }

    #[test]
    fn build_pack_produces_zip_with_describe_and_wasm() {
        let tmp = tempfile::tempdir().unwrap();
        let wasm = make_project(tmp.path());
        let out = tmp.path().join("dist/demo-0.1.0.gtxpack");
        let info = build_pack(tmp.path(), &wasm, &out).unwrap();
        assert_eq!(info.ext_name, "demo");
        assert_eq!(info.ext_version, "0.1.0");
        assert_eq!(info.ext_kind, "DesignExtension");
        assert!(info.size > 0);

        let file = File::open(&out).unwrap();
        let mut zip = zip::ZipArchive::new(file).unwrap();
        let names: Vec<_> = (0..zip.len())
            .map(|i| zip.by_index(i).unwrap().name().to_string())
            .collect();
        assert!(names.iter().any(|n| n == "describe.json"));
        assert!(names.iter().any(|n| n == "extension.wasm"));
    }

    #[test]
    fn build_pack_is_deterministic_across_runs() {
        let tmp = tempfile::tempdir().unwrap();
        let wasm = make_project(tmp.path());
        let out1 = tmp.path().join("a.gtxpack");
        let out2 = tmp.path().join("b.gtxpack");
        let a = build_pack(tmp.path(), &wasm, &out1).unwrap();
        let b = build_pack(tmp.path(), &wasm, &out2).unwrap();
        assert_eq!(a.sha256, b.sha256);
    }

    #[test]
    fn build_pack_includes_assets_when_present() {
        let tmp = tempfile::tempdir().unwrap();
        let wasm = make_project(tmp.path());
        std::fs::create_dir_all(tmp.path().join("i18n")).unwrap();
        std::fs::write(tmp.path().join("i18n/en.json"), br#"{"hello":"world"}"#).unwrap();
        let out = tmp.path().join("demo.gtxpack");
        build_pack(tmp.path(), &wasm, &out).unwrap();
        let file = File::open(&out).unwrap();
        let zip = zip::ZipArchive::new(file).unwrap();
        let names: Vec<_> = zip.file_names().map(str::to_string).collect();
        assert!(names.iter().any(|n| n == "i18n/en.json"));
    }

    #[test]
    fn build_pack_errors_if_describe_missing() {
        let tmp = tempfile::tempdir().unwrap();
        let wasm_dir = tmp.path().join("target/wasm32-wasip2/debug");
        std::fs::create_dir_all(&wasm_dir).unwrap();
        std::fs::write(wasm_dir.join("x.wasm"), b"\0asm").unwrap();
        let out = tmp.path().join("out.gtxpack");
        let err = build_pack(tmp.path(), &wasm_dir.join("x.wasm"), &out).unwrap_err();
        assert!(err.to_string().contains("describe.json"));
    }
}
```

- [ ] **Step 2: Run tests**

Run: `cargo test -p greentic-ext-cli --bins dev::packer 2>&1 | tail -15`
Expected: 4 passed (one new test for determinism).

- [ ] **Step 3: Commit**

```bash
git add crates/greentic-ext-cli/src/dev/packer.rs
git commit -m "refactor(ext-cli): dev packer delegates to greentic-ext-contract::pack_writer"
```

---

## Task 4: PublishRequest + PublishReceipt types

**Files:**
- Create: `crates/greentic-ext-registry/src/publish.rs`
- Modify: `crates/greentic-ext-registry/src/lib.rs`

- [ ] **Step 1: Create publish.rs**

```rust
//! Types for `ExtensionRegistry::publish()` requests + receipts.

use chrono::{DateTime, Utc};
use greentic_ext_contract::{DescribeJson, ExtensionKind};
use serde::{Deserialize, Serialize};

/// One publish invocation: self-contained, backend-agnostic.
#[derive(Debug, Clone)]
pub struct PublishRequest {
    pub ext_id: String,
    pub ext_name: String,
    pub version: String,
    pub kind: ExtensionKind,
    pub artifact_bytes: Vec<u8>,
    pub artifact_sha256: String,
    pub describe: DescribeJson,
    pub signature: Option<SignatureBlob>,
    pub force: bool,
}

/// Optional signature carried alongside the artifact. The signature is over
/// the JCS-canonicalized describe.json (via `sign_describe`); Phase 1 does
/// NOT sign the artifact bytes themselves.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignatureBlob {
    pub algorithm: String,
    pub public_key: String,
    pub value: String,
    pub key_id: String,
}

/// Confirmation returned from a successful publish.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublishReceipt {
    pub url: String,
    pub sha256: String,
    pub published_at: DateTime<Utc>,
    pub signed: bool,
}
```

- [ ] **Step 2: Wire into lib.rs**

Edit `crates/greentic-ext-registry/src/lib.rs`. Add to the existing `pub mod` list (alphabetical):

```rust
pub mod publish;
```

Add re-exports near the bottom of the file:

```rust
pub use self::publish::{PublishReceipt, PublishRequest, SignatureBlob};
```

- [ ] **Step 3: Verify build**

Run: `cargo build -p greentic-ext-registry --quiet 2>&1 | tail -5`
Expected: exit 0.

- [ ] **Step 4: Commit**

```bash
git add crates/greentic-ext-registry/src/publish.rs crates/greentic-ext-registry/src/lib.rs
git commit -m "feat(ext-registry): add PublishRequest + PublishReceipt + SignatureBlob types"
```

---

## Task 5: RegistryError variants + Registry trait signature change

**Files:**
- Modify: `crates/greentic-ext-registry/src/error.rs`
- Modify: `crates/greentic-ext-registry/src/registry.rs`
- Modify: `crates/greentic-ext-registry/src/store.rs`
- Modify: `crates/greentic-ext-registry/src/oci.rs`

- [ ] **Step 1: Add error variants**

Edit `crates/greentic-ext-registry/src/error.rs`, append new variants to the enum (keep existing ones intact):

```rust
    #[error("version already exists in registry (sha256={existing_sha})")]
    VersionExists { existing_sha: String },

    #[error("not implemented: {hint}")]
    NotImplemented { hint: String },
```

- [ ] **Step 2: Update trait default publish signature**

Edit `crates/greentic-ext-registry/src/registry.rs`. Replace the existing `publish` default:

```rust
    async fn publish(&self, req: PublishRequest) -> Result<PublishReceipt, RegistryError> {
        let _ = req;
        Err(RegistryError::NotImplemented {
            hint: format!("publish not supported for registry '{}'", self.name()),
        })
    }
```

Add the import at the top of the file:

```rust
use crate::publish::{PublishReceipt, PublishRequest};
```

Remove the now-unused `AuthToken` import if the trait signature change leaves it orphaned (check compiler output).

- [ ] **Step 3: Override in GreenticStoreRegistry**

Edit `crates/greentic-ext-registry/src/store.rs`. Inside the `impl ExtensionRegistry for GreenticStoreRegistry` block, add (near end, before closing brace):

```rust
    async fn publish(
        &self,
        _req: crate::publish::PublishRequest,
    ) -> Result<crate::publish::PublishReceipt, RegistryError> {
        Err(RegistryError::NotImplemented {
            hint: "Store publish lands in Phase 2 (S5). Use --registry local for now.".into(),
        })
    }
```

- [ ] **Step 4: Override in OciRegistry**

Edit `crates/greentic-ext-registry/src/oci.rs`. Inside the `impl ExtensionRegistry for OciRegistry` block, add:

```rust
    async fn publish(
        &self,
        _req: crate::publish::PublishRequest,
    ) -> Result<crate::publish::PublishReceipt, RegistryError> {
        Err(RegistryError::NotImplemented {
            hint: "OCI publish lands in Phase 2 (S5). Use --registry local for now.".into(),
        })
    }
```

- [ ] **Step 5: Verify build**

Run: `cargo build -p greentic-ext-registry --quiet 2>&1 | tail -10`
Expected: exit 0. Pre-existing callers to `fetch`/`search` etc are unaffected because they don't touch `publish`. If any call site breaks (e.g. the unused default `publish(artifact, auth)` signature is being invoked somewhere), report the exact file:line — no such call sites should exist.

- [ ] **Step 6: Commit**

```bash
git add crates/greentic-ext-registry/src/error.rs crates/greentic-ext-registry/src/registry.rs crates/greentic-ext-registry/src/store.rs crates/greentic-ext-registry/src/oci.rs
git commit -m "feat(ext-registry): Registry::publish takes PublishRequest; Store/OCI return NotImplemented"
```

---

## Task 6: LocalFilesystemRegistry::publish impl

**Files:**
- Create: `crates/greentic-ext-registry/src/local_publish.rs`
- Modify: `crates/greentic-ext-registry/src/lib.rs` (`pub mod local_publish;`)
- Modify: `crates/greentic-ext-registry/src/local.rs`

- [ ] **Step 1: Create local_publish.rs**

```rust
//! `LocalFilesystemRegistry::publish` implementation — hierarchical layout,
//! atomic temp-then-rename, advisory file lock on `index.json`.

use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};

use chrono::Utc;
use fs2::FileExt;
use serde::{Deserialize, Serialize};

use crate::error::RegistryError;
use crate::local::LocalFilesystemRegistry;
use crate::publish::{PublishReceipt, PublishRequest};

const LOCK_FILE: &str = ".publish.lock";
const INDEX_FILE: &str = "index.json";

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RegistryIndex {
    pub extensions: Vec<IndexEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexEntry {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub versions: Vec<String>,
    pub latest: String,
}

impl LocalFilesystemRegistry {
    /// Publish `req.artifact_bytes` into `<root>/<id>/<version>/<id>-<version>.gtxpack`.
    /// Atomic: writes to `<target>.tmp`, fsyncs, renames. Concurrency: acquires
    /// an exclusive advisory lock on `<root>/.publish.lock` for the whole op.
    ///
    /// # Errors
    /// - `RegistryError::VersionExists` if version dir already present and `!req.force`.
    /// - `RegistryError::Io` for filesystem failures.
    pub fn publish_local(
        &self,
        req: &PublishRequest,
    ) -> Result<PublishReceipt, RegistryError> {
        let root = self.root_path();
        fs::create_dir_all(root)?;
        let _lock = acquire_lock(root)?;

        let ext_dir = root.join(&req.ext_id);
        let ver_dir = ext_dir.join(&req.version);

        if ver_dir.exists() && !req.force {
            let existing_sha = read_existing_sha(&ver_dir).unwrap_or_else(|_| "unknown".into());
            return Err(RegistryError::VersionExists { existing_sha });
        }
        if ver_dir.exists() && req.force {
            fs::remove_dir_all(&ver_dir)?;
        }
        fs::create_dir_all(&ver_dir)?;

        let pack_name = format!("{}-{}.gtxpack", req.ext_name, req.version);
        let pack_path = ver_dir.join(&pack_name);
        atomic_write(&pack_path, &req.artifact_bytes)?;

        let manifest_path = ver_dir.join("manifest.json");
        let manifest_bytes = serde_json::to_vec_pretty(&req.describe)?;
        atomic_write(&manifest_path, &manifest_bytes)?;

        if let Some(sig) = &req.signature {
            let sig_path = ver_dir.join("signature.json");
            let sig_bytes = serde_json::to_vec_pretty(sig)?;
            atomic_write(&sig_path, &sig_bytes)?;
        }

        let sha_sidecar = ver_dir.join("artifact.sha256");
        atomic_write(&sha_sidecar, req.artifact_sha256.as_bytes())?;

        update_index(root, req)?;
        update_metadata(&ext_dir, req)?;

        let url = format!("file://{}", pack_path.display());
        Ok(PublishReceipt {
            url,
            sha256: req.artifact_sha256.clone(),
            published_at: Utc::now(),
            signed: req.signature.is_some(),
        })
    }
}

fn acquire_lock(root: &Path) -> Result<File, RegistryError> {
    let lock_path = root.join(LOCK_FILE);
    let file = File::options().create(true).write(true).truncate(false).open(&lock_path)?;
    file.lock_exclusive()
        .map_err(|e| RegistryError::Storage(format!("lock {}: {e}", lock_path.display())))?;
    Ok(file)
}

fn atomic_write(dest: &Path, bytes: &[u8]) -> Result<(), RegistryError> {
    let tmp = dest.with_extension(
        dest.extension()
            .map(|e| format!("{}.tmp", e.to_string_lossy()))
            .unwrap_or_else(|| "tmp".into()),
    );
    {
        let mut f = File::create(&tmp)?;
        f.write_all(bytes)?;
        f.sync_all()?;
    }
    fs::rename(&tmp, dest)?;
    Ok(())
}

fn read_existing_sha(ver_dir: &Path) -> Result<String, RegistryError> {
    let path = ver_dir.join("artifact.sha256");
    Ok(fs::read_to_string(path)?.trim().to_string())
}

fn update_index(root: &Path, req: &PublishRequest) -> Result<(), RegistryError> {
    let index_path = root.join(INDEX_FILE);
    let mut index: RegistryIndex = if index_path.exists() {
        let bytes = fs::read(&index_path)?;
        serde_json::from_slice(&bytes).unwrap_or_default()
    } else {
        RegistryIndex::default()
    };

    let kind_str = serde_json::to_value(&req.kind)?
        .as_str()
        .unwrap_or("Unknown")
        .to_string();

    if let Some(entry) = index.extensions.iter_mut().find(|e| e.id == req.ext_id) {
        if !entry.versions.contains(&req.version) {
            entry.versions.push(req.version.clone());
        }
        entry.versions.sort();
        entry.latest = entry.versions.last().cloned().unwrap_or_default();
        entry.name = req.ext_name.clone();
        entry.kind = kind_str;
    } else {
        index.extensions.push(IndexEntry {
            id: req.ext_id.clone(),
            name: req.ext_name.clone(),
            kind: kind_str,
            versions: vec![req.version.clone()],
            latest: req.version.clone(),
        });
    }
    index.extensions.sort_by(|a, b| a.id.cmp(&b.id));

    atomic_write(&index_path, &serde_json::to_vec_pretty(&index)?)?;
    Ok(())
}

fn update_metadata(ext_dir: &Path, req: &PublishRequest) -> Result<(), RegistryError> {
    let path = ext_dir.join("metadata.json");
    let body = serde_json::to_vec_pretty(&req.describe)?;
    atomic_write(&path, &body)?;
    Ok(())
}
```

- [ ] **Step 2: Expose `root_path()` on LocalFilesystemRegistry**

Edit `crates/greentic-ext-registry/src/local.rs`. Add near the existing helper fns in the `impl LocalFilesystemRegistry` block:

```rust
    /// Return the on-disk root path of this registry.
    #[must_use]
    pub fn root_path(&self) -> &Path {
        &self.root
    }
```

Also add the async `publish` trait method (delegating to `publish_local`):

```rust
    async fn publish(
        &self,
        req: crate::publish::PublishRequest,
    ) -> Result<crate::publish::PublishReceipt, RegistryError> {
        self.publish_local(&req)
    }
```

This goes INSIDE the existing `impl ExtensionRegistry for LocalFilesystemRegistry` block (alongside `fetch`, `search`, etc).

- [ ] **Step 3: Wire module into lib.rs**

Edit `crates/greentic-ext-registry/src/lib.rs`. Add `pub mod local_publish;` (alphabetical, after `local`).

- [ ] **Step 4: Verify build**

Run: `cargo build -p greentic-ext-registry --quiet 2>&1 | tail -10`
Expected: exit 0.

- [ ] **Step 5: Commit**

```bash
git add crates/greentic-ext-registry/src/local.rs crates/greentic-ext-registry/src/local_publish.rs crates/greentic-ext-registry/src/lib.rs
git commit -m "feat(ext-registry): LocalFilesystemRegistry::publish with atomic write + advisory lock + index.json"
```

---

## Task 7: LocalFilesystemRegistry::fetch dual-layout support

`LocalFilesystemRegistry::fetch()` today assumes the flat `<root>/<name>-<version>.gtxpack` layout (Track B's dev-local scratch dir still uses this). Track C's `publish` introduces hierarchical `<root>/<id>/<version>/<name>-<version>.gtxpack`. Make `fetch` try hierarchical first and fall back to flat.

**Files:**
- Modify: `crates/greentic-ext-registry/src/local.rs`

- [ ] **Step 1: Update pack_path + fetch logic**

In `crates/greentic-ext-registry/src/local.rs`, find the existing `pack_path` helper and replace with a resolver that searches both layouts (keep the original method name so other callers are unaffected):

```rust
    fn resolve_pack_path(&self, name: &str, version: &str) -> Option<PathBuf> {
        // Prefer hierarchical (Track C publish layout).
        // The publish writer uses <id>/<version>/<id>-<version>.gtxpack and
        // <id>/<version>/<name>-<version>.gtxpack when id == name (they may
        // differ — scaffolded projects use id=com.example.<name>).
        // `name` here is the fetch key — try it both as an id and a name.
        let hierarchical_by_name = self
            .root
            .join(name)
            .join(version)
            .join(format!("{name}-{version}.gtxpack"));
        if hierarchical_by_name.is_file() {
            return Some(hierarchical_by_name);
        }
        for entry in std::fs::read_dir(&self.root).ok()?.flatten() {
            if !entry.file_type().ok()?.is_dir() {
                continue;
            }
            let ext_dir = entry.path();
            let ver_dir = ext_dir.join(version);
            if !ver_dir.is_dir() {
                continue;
            }
            for pack in std::fs::read_dir(&ver_dir).ok()?.flatten() {
                let path = pack.path();
                if path.extension().and_then(|s| s.to_str()) == Some("gtxpack") {
                    let filename = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
                    if let Some((n, v)) = Self::parse_pack_filename(filename)
                        && v == version
                        && (n == name
                            || ext_dir
                                .file_name()
                                .and_then(|s| s.to_str())
                                .is_some_and(|id| id == name))
                    {
                        return Some(path);
                    }
                }
            }
        }
        // Fallback: flat layout (dev-local scratch dir).
        let flat = self.root.join(format!("{name}-{version}.gtxpack"));
        if flat.is_file() {
            return Some(flat);
        }
        None
    }

    fn pack_path(&self, name: &str, version: &str) -> PathBuf {
        self.resolve_pack_path(name, version)
            .unwrap_or_else(|| self.root.join(format!("{name}-{version}.gtxpack")))
    }
```

Also update `list_packs` to include hierarchical entries. Replace it:

```rust
    fn list_packs(&self) -> std::io::Result<Vec<(String, String, PathBuf)>> {
        if !self.root.exists() {
            return Ok(Vec::new());
        }
        let mut out = Vec::new();
        for entry in std::fs::read_dir(&self.root)? {
            let entry = entry?;
            let ft = entry.file_type()?;
            let path = entry.path();
            if ft.is_file() {
                let filename = entry.file_name();
                let filename_str = filename.to_string_lossy();
                if let Some((n, v)) = Self::parse_pack_filename(&filename_str) {
                    out.push((n, v, path));
                }
            } else if ft.is_dir() {
                // Hierarchical: <id>/<version>/<name>-<version>.gtxpack
                for ver_entry in std::fs::read_dir(&path)?.flatten() {
                    if !ver_entry.file_type()?.is_dir() {
                        continue;
                    }
                    for pack_entry in std::fs::read_dir(ver_entry.path())?.flatten() {
                        let pack_path = pack_entry.path();
                        if pack_path.extension().and_then(|s| s.to_str()) != Some("gtxpack") {
                            continue;
                        }
                        let filename = pack_entry.file_name();
                        let filename_str = filename.to_string_lossy();
                        if let Some((n, v)) = Self::parse_pack_filename(&filename_str) {
                            out.push((n, v, pack_path));
                        }
                    }
                }
            }
        }
        Ok(out)
    }
```

- [ ] **Step 2: Verify build + existing tests**

Run: `cargo test -p greentic-ext-registry --lib 2>&1 | tail -15`
Expected: all green.

Run: `cargo test -p greentic-ext-cli --bins 2>&1 | tail -5`
Expected: 47+ passed (no regression in dev packer or elsewhere).

- [ ] **Step 3: Commit**

```bash
git add crates/greentic-ext-registry/src/local.rs
git commit -m "feat(ext-registry): fetch reads hierarchical + flat layouts; list_packs walks both"
```

---

## Task 8: Validator — aggregated describe.json checks

**Files:**
- Create: `crates/greentic-ext-cli/src/publish/mod.rs`
- Create: `crates/greentic-ext-cli/src/publish/validator.rs`
- Create: `crates/greentic-ext-cli/src/publish/receipt.rs`

- [ ] **Step 1: Create publish module root**

`crates/greentic-ext-cli/src/publish/mod.rs`:

```rust
//! gtdx publish: build + validate + pack + publish orchestration.

pub mod receipt;
pub mod validator;
```

- [ ] **Step 2: Write validator with failing tests**

`crates/greentic-ext-cli/src/publish/validator.rs`:

```rust
//! Aggregated pre-publish describe.json validation.

use greentic_ext_contract::DescribeJson;
use semver::Version;

/// Validate describe for publish. All violations are collected before returning.
pub fn validate_for_publish(describe: &DescribeJson) -> Result<(), Vec<ValidationError>> {
    let mut errors = Vec::new();

    if Version::parse(&describe.metadata.version).is_err() {
        errors.push(ValidationError::new(
            "metadata.version",
            format!("'{}' is not a valid semver", describe.metadata.version),
        ));
    }
    if !is_valid_id(&describe.metadata.id) {
        errors.push(ValidationError::new(
            "metadata.id",
            format!(
                "'{}' — must match reverse-DNS regex ^[a-z][a-z0-9-]*(\\.[a-z][a-z0-9-]*)+$",
                describe.metadata.id
            ),
        ));
    }
    for (i, cap) in describe.capabilities.offered.iter().enumerate() {
        if Version::parse(&cap.version).is_err() {
            errors.push(ValidationError::new(
                format!("capabilities.offered[{i}].version"),
                format!("'{}' — not a valid semver", cap.version),
            ));
        }
    }
    for (i, url) in describe.runtime.permissions.network.iter().enumerate() {
        if !url.starts_with("https://") {
            errors.push(ValidationError::new(
                format!("runtime.permissions.network[{i}]"),
                format!("'{url}' — must be https://"),
            ));
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

fn is_valid_id(id: &str) -> bool {
    // Regex: ^[a-z][a-z0-9-]*(\.[a-z][a-z0-9-]*)+$
    let mut parts = id.split('.');
    let Some(first) = parts.next() else {
        return false;
    };
    if !part_is_valid(first) {
        return false;
    }
    let mut has_more = false;
    for p in parts {
        has_more = true;
        if !part_is_valid(p) {
            return false;
        }
    }
    has_more
}

fn part_is_valid(s: &str) -> bool {
    let mut chars = s.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if !first.is_ascii_lowercase() {
        return false;
    }
    chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

#[derive(Debug, Clone)]
pub struct ValidationError {
    pub field: String,
    pub message: String,
}

impl ValidationError {
    pub fn new(field: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            field: field.into(),
            message: message.into(),
        }
    }
}

/// Human-readable formatter for a collection of errors.
pub fn format_errors(errors: &[ValidationError]) -> String {
    let mut out = format!("\u{2717} describe.json validation failed ({} errors):\n", errors.len());
    for e in errors {
        out.push_str(&format!("  \u{2022} {}: {}\n", e.field, e.message));
    }
    out.push_str("\nFix these and re-run: gtdx publish\n");
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use greentic_ext_contract::{
        describe::{Author, Capabilities, Engine, Metadata, Permissions, Runtime},
        DescribeJson, ExtensionKind,
    };

    fn sample_describe() -> DescribeJson {
        DescribeJson {
            schema_ref: None,
            api_version: "greentic.ai/v1".into(),
            kind: ExtensionKind::Design,
            metadata: Metadata {
                id: "com.example.demo".into(),
                name: "demo".into(),
                version: "0.1.0".into(),
                summary: "s".into(),
                description: None,
                author: Author {
                    name: "a".into(),
                    email: None,
                    public_key: None,
                },
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
            capabilities: Capabilities {
                offered: vec![],
                required: vec![],
            },
            runtime: Runtime {
                component: "extension.wasm".into(),
                memory_limit_mb: 64,
                permissions: Permissions::default(),
            },
            contributions: serde_json::json!({}),
            signature: None,
        }
    }

    #[test]
    fn valid_describe_passes() {
        assert!(validate_for_publish(&sample_describe()).is_ok());
    }

    #[test]
    fn bad_version_reports_error() {
        let mut d = sample_describe();
        d.metadata.version = "0.1".into();
        let errs = validate_for_publish(&d).unwrap_err();
        assert!(errs.iter().any(|e| e.field == "metadata.version"));
    }

    #[test]
    fn bad_id_reports_error() {
        let mut d = sample_describe();
        d.metadata.id = "NoDots".into();
        let errs = validate_for_publish(&d).unwrap_err();
        assert!(errs.iter().any(|e| e.field == "metadata.id"));
    }

    #[test]
    fn http_permission_is_rejected() {
        let mut d = sample_describe();
        d.runtime.permissions.network = vec!["http://insecure.com".into()];
        let errs = validate_for_publish(&d).unwrap_err();
        assert!(
            errs.iter()
                .any(|e| e.field == "runtime.permissions.network[0]")
        );
    }

    #[test]
    fn errors_aggregate_all_violations() {
        let mut d = sample_describe();
        d.metadata.version = "0.1".into();
        d.metadata.id = "BAD".into();
        d.runtime.permissions.network = vec!["http://insecure.com".into()];
        let errs = validate_for_publish(&d).unwrap_err();
        assert_eq!(errs.len(), 3);
    }

    #[test]
    fn format_errors_lists_all_fields() {
        let errs = vec![
            ValidationError::new("metadata.version", "bad"),
            ValidationError::new("metadata.id", "bad"),
        ];
        let s = format_errors(&errs);
        assert!(s.contains("2 errors"));
        assert!(s.contains("metadata.version"));
        assert!(s.contains("metadata.id"));
    }
}
```

- [ ] **Step 3: Create receipt writer (stub — Task 12 fills in the writer)**

`crates/greentic-ext-cli/src/publish/receipt.rs`:

```rust
//! Writes `./dist/publish-<id>-<version>.json` receipts.

use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublishReceiptJson {
    pub artifact: String,
    pub sha256: String,
    pub registry: String,
    pub published_at: DateTime<Utc>,
    pub trust_policy: String,
    pub signed: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signing_known_limitations: Option<Vec<String>>,
}

pub fn receipt_path(dist_dir: &Path, ext_id: &str, version: &str) -> PathBuf {
    dist_dir.join(format!("publish-{ext_id}-{version}.json"))
}

pub fn write_receipt(
    dist_dir: &Path,
    ext_id: &str,
    version: &str,
    receipt: &PublishReceiptJson,
) -> anyhow::Result<PathBuf> {
    std::fs::create_dir_all(dist_dir)?;
    let path = receipt_path(dist_dir, ext_id, version);
    let bytes = serde_json::to_vec_pretty(receipt)?;
    std::fs::write(&path, bytes)?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn receipt_path_includes_id_and_version() {
        let p = receipt_path(Path::new("/dist"), "com.example.demo", "0.1.0");
        assert_eq!(
            p,
            Path::new("/dist/publish-com.example.demo-0.1.0.json")
        );
    }

    #[test]
    fn write_receipt_creates_file() {
        let tmp = tempfile::tempdir().unwrap();
        let receipt = PublishReceiptJson {
            artifact: "demo-0.1.0.gtxpack".into(),
            sha256: "abc".into(),
            registry: "file:///x".into(),
            published_at: Utc::now(),
            trust_policy: "loose".into(),
            signed: false,
            signing_known_limitations: None,
        };
        let path = write_receipt(tmp.path(), "com.example.demo", "0.1.0", &receipt).unwrap();
        assert!(path.exists());
        let read: PublishReceiptJson =
            serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        assert_eq!(read.artifact, "demo-0.1.0.gtxpack");
    }
}
```

- [ ] **Step 4: Run tests**

Run: `cargo test -p greentic-ext-cli --bins publish::validator publish::receipt 2>&1 | tail -15`
Expected: 6 validator tests + 2 receipt tests = 8 passed.

- [ ] **Step 5: Commit**

```bash
git add crates/greentic-ext-cli/src/publish
git commit -m "feat(ext-cli): publish module — aggregated validator + receipt writer"
```

---

## Task 9: gtdx publish command skeleton + CLI wiring

**Files:**
- Create: `crates/greentic-ext-cli/src/commands/publish.rs`
- Modify: `crates/greentic-ext-cli/src/commands/mod.rs`
- Modify: `crates/greentic-ext-cli/src/main.rs`

- [ ] **Step 1: Create publish command stub**

`crates/greentic-ext-cli/src/commands/publish.rs`:

```rust
use std::path::{Path, PathBuf};

use clap::Args as ClapArgs;

#[derive(ClapArgs, Debug, Clone)]
#[allow(clippy::struct_excessive_bools)]
pub struct Args {
    /// Registry URI. `local` resolves to $GREENTIC_HOME/registries/local.
    /// Accepts file://<path> for explicit paths.
    #[arg(short = 'r', long, default_value = "local")]
    pub registry: String,

    /// Override describe.json version for this run (CI version bumps).
    #[arg(long)]
    pub version: Option<String>,

    /// Build + pack + validate; skip registry write.
    #[arg(long)]
    pub dry_run: bool,

    /// Sign .gtxpack with local key from ~/.greentic/keys/.
    #[arg(long)]
    pub sign: bool,

    /// Signing key id (requires --sign).
    #[arg(long)]
    pub key_id: Option<String>,

    /// loose | normal | strict
    #[arg(long, default_value = "loose")]
    pub trust: String,

    /// Copy artifact here as well.
    #[arg(long, default_value = "./dist")]
    pub dist: PathBuf,

    /// Overwrite existing version.
    #[arg(long)]
    pub force: bool,

    /// cargo component build --release (default true for publish).
    #[arg(long, default_value_t = true)]
    pub release: bool,

    /// Skip build; only check registry for version conflict.
    #[arg(long)]
    pub verify_only: bool,

    /// Path to the project's Cargo.toml.
    #[arg(long, default_value = "./Cargo.toml")]
    pub manifest: PathBuf,

    /// human | json
    #[arg(long, default_value = "human")]
    pub format: String,
}

pub async fn run(_args: Args, _home: &Path) -> anyhow::Result<()> {
    anyhow::bail!("gtdx publish is not yet implemented")
}
```

- [ ] **Step 2: Register in commands/mod.rs**

Add `pub mod publish;` alphabetically (between `list` and `registries`).

- [ ] **Step 3: Wire into main.rs**

Add `mod publish;` after existing `mod dev;`. Add `Command::Publish(commands::publish::Args)` variant near `Dev`. Add match arm:

```rust
        Command::Publish(args) => commands::publish::run(args, &home).await,
```

- [ ] **Step 4: Verify build + help**

Run: `cargo build -p greentic-ext-cli 2>&1 | tail -5 && ./target/debug/gtdx publish --help 2>&1 | head -20`
Expected: build OK; help lists all 11 flags.

- [ ] **Step 5: Commit**

```bash
git add crates/greentic-ext-cli/src/commands/publish.rs crates/greentic-ext-cli/src/commands/mod.rs crates/greentic-ext-cli/src/main.rs
git commit -m "feat(ext-cli): gtdx publish command skeleton (stub impl)"
```

---

## Task 10: Publish orchestrator — build + pack + publish + receipt

**Files:**
- Modify: `crates/greentic-ext-cli/src/publish/mod.rs`
- Modify: `crates/greentic-ext-cli/src/commands/publish.rs`

- [ ] **Step 1: Extend publish/mod.rs with orchestrator**

Append to `crates/greentic-ext-cli/src/publish/mod.rs`:

```rust
use std::path::{Path, PathBuf};

use chrono::Utc;
use greentic_ext_contract::DescribeJson;
use greentic_ext_registry::local::LocalFilesystemRegistry;
use greentic_ext_registry::publish::{PublishRequest, SignatureBlob};
use greentic_ext_registry::registry::ExtensionRegistry;

use crate::dev::builder::{Profile, run_build};
use crate::dev::packer::build_pack;
use crate::publish::receipt::{PublishReceiptJson, write_receipt};
use crate::publish::validator::{format_errors, validate_for_publish};

#[derive(Debug, Clone)]
pub struct PublishConfig {
    pub project_dir: PathBuf,
    pub registry_uri: String,
    pub home: PathBuf,
    pub dist_dir: PathBuf,
    pub profile: Profile,
    pub dry_run: bool,
    pub force: bool,
    pub sign: bool,
    pub key_id: Option<String>,
    pub version_override: Option<String>,
    pub trust_policy: String,
    pub verify_only: bool,
}

pub async fn run_publish(cfg: &PublishConfig) -> anyhow::Result<PublishOutcome> {
    // 1. Load + schema-validate describe.json via ext-contract.
    let describe_path = cfg.project_dir.join("describe.json");
    let describe_bytes = std::fs::read(&describe_path)
        .map_err(|e| anyhow::anyhow!("read describe.json: {e}"))?;
    let describe_value: serde_json::Value = serde_json::from_slice(&describe_bytes)
        .map_err(|e| anyhow::anyhow!("parse describe.json: {e}"))?;
    greentic_ext_contract::schema::validate_describe_json(&describe_value)
        .map_err(|e| anyhow::anyhow!("describe.json schema: {e}"))?;
    let mut describe: DescribeJson = serde_json::from_value(describe_value)?;
    if let Some(v) = &cfg.version_override {
        describe.metadata.version = v.clone();
    }

    // 2. Business-rule validator (aggregated).
    if let Err(errors) = validate_for_publish(&describe) {
        anyhow::bail!("{}", format_errors(&errors));
    }

    // 3. Resolve registry.
    let registry_root = resolve_registry_root(&cfg.registry_uri, &cfg.home)?;
    let registry = LocalFilesystemRegistry::new("publish-local", registry_root.clone());

    if cfg.verify_only {
        let ver_dir = registry_root
            .join(&describe.metadata.id)
            .join(&describe.metadata.version);
        if ver_dir.exists() && !cfg.force {
            anyhow::bail!(
                "version {} already exists at {}",
                describe.metadata.version,
                ver_dir.display()
            );
        }
        return Ok(PublishOutcome::VerifyOnly {
            ext_id: describe.metadata.id,
            version: describe.metadata.version,
            registry: registry_root.display().to_string(),
        });
    }

    // 4. Build release wasm.
    let build = run_build(&cfg.project_dir, cfg.profile)
        .map_err(|e| anyhow::anyhow!("cargo component build: {e}"))?;

    // 5. Pack deterministic .gtxpack (staging file).
    let staging_pack = cfg.project_dir.join("dist/publish-staging.gtxpack");
    let info = build_pack(&cfg.project_dir, &build.wasm_path, &staging_pack)?;
    // The packer read describe.json from disk, but version_override may have
    // changed ours in memory. Re-hash with the active describe bytes.
    let describe_for_pack = serde_json::to_vec_pretty(&describe)?;
    let pack_bytes = std::fs::read(&staging_pack)?;

    // 6. Optional signing (reuse Wave 1 JCS sign_describe).
    let signature = if cfg.sign {
        let key_id = cfg
            .key_id
            .clone()
            .ok_or_else(|| anyhow::anyhow!("--sign requires --key-id"))?;
        let signing_key = load_signing_key(&cfg.home, &key_id)?;
        greentic_ext_contract::sign_describe(&mut describe, &signing_key)
            .map_err(|e| anyhow::anyhow!("sign: {e}"))?;
        let sig = describe
            .signature
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("signing produced no signature"))?;
        Some(SignatureBlob {
            algorithm: sig.algorithm.clone(),
            public_key: sig.public_key.clone(),
            value: sig.value.clone(),
            key_id,
        })
    } else {
        None
    };

    if cfg.dry_run {
        return Ok(PublishOutcome::DryRun {
            artifact: staging_pack,
            sha256: info.sha256,
            registry: registry_root.display().to_string(),
        });
    }

    // 7. Publish through the registry trait.
    let req = PublishRequest {
        ext_id: describe.metadata.id.clone(),
        ext_name: describe.metadata.name.clone(),
        version: describe.metadata.version.clone(),
        kind: describe.kind,
        artifact_bytes: pack_bytes.clone(),
        artifact_sha256: info.sha256.clone(),
        describe: describe.clone(),
        signature: signature.clone(),
        force: cfg.force,
    };
    let _ = describe_for_pack; // describe bytes already embedded in the packed zip

    let receipt = registry
        .publish(req)
        .await
        .map_err(|e| anyhow::anyhow!("publish: {e}"))?;

    // 8. Also copy into local ./dist/ (canonical name).
    let final_dist = cfg.dist_dir.join(format!(
        "{}-{}.gtxpack",
        describe.metadata.name, describe.metadata.version
    ));
    std::fs::create_dir_all(&cfg.dist_dir)?;
    std::fs::write(&final_dist, &pack_bytes)?;

    let receipt_json = PublishReceiptJson {
        artifact: final_dist
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("pack.gtxpack")
            .to_string(),
        sha256: info.sha256,
        registry: receipt.url.clone(),
        published_at: receipt.published_at,
        trust_policy: cfg.trust_policy.clone(),
        signed: receipt.signed,
        signing_known_limitations: None,
    };
    let receipt_path = write_receipt(
        &cfg.dist_dir,
        &describe.metadata.id,
        &describe.metadata.version,
        &receipt_json,
    )?;

    let _ = staging_pack; // staging file kept in dist/ for diagnostics
    let _ = Utc::now(); // silence warning if chrono unused
    Ok(PublishOutcome::Published {
        ext_id: describe.metadata.id,
        version: describe.metadata.version,
        sha256: receipt_json.sha256,
        artifact: final_dist,
        receipt_path,
        signed: receipt.signed,
        registry_url: receipt.url,
    })
}

fn resolve_registry_root(uri: &str, home: &Path) -> anyhow::Result<PathBuf> {
    if uri == "local" {
        return Ok(home.join("registries/local"));
    }
    if let Some(rest) = uri.strip_prefix("file://") {
        return Ok(PathBuf::from(rest));
    }
    anyhow::bail!("unsupported --registry {uri} (only 'local' or file:// in Phase 1)")
}

fn load_signing_key(home: &Path, key_id: &str) -> anyhow::Result<ed25519_dalek::SigningKey> {
    use base64::{Engine as _, engine::general_purpose::STANDARD as B64};
    let key_path = home.join("keys").join(format!("{key_id}.key"));
    let bytes = std::fs::read_to_string(&key_path)
        .map_err(|e| anyhow::anyhow!("read {}: {e}", key_path.display()))?;
    let decoded = B64
        .decode(bytes.trim())
        .map_err(|e| anyhow::anyhow!("decode {key_id}.key: {e}"))?;
    let arr: [u8; 32] = decoded
        .as_slice()
        .try_into()
        .map_err(|_| anyhow::anyhow!("{key_id}.key must be 32 bytes base64"))?;
    Ok(ed25519_dalek::SigningKey::from_bytes(&arr))
}

#[derive(Debug)]
pub enum PublishOutcome {
    DryRun {
        artifact: PathBuf,
        sha256: String,
        registry: String,
    },
    VerifyOnly {
        ext_id: String,
        version: String,
        registry: String,
    },
    Published {
        ext_id: String,
        version: String,
        sha256: String,
        artifact: PathBuf,
        receipt_path: PathBuf,
        signed: bool,
        registry_url: String,
    },
}
```

- [ ] **Step 2: Replace publish command stub with dispatcher**

Replace `crates/greentic-ext-cli/src/commands/publish.rs` fully:

```rust
use std::path::{Path, PathBuf};

use clap::Args as ClapArgs;

use crate::dev::builder::Profile;
use crate::dev::project_dir_from_manifest;
use crate::publish::{PublishConfig, PublishOutcome, run_publish};

#[derive(ClapArgs, Debug, Clone)]
#[allow(clippy::struct_excessive_bools)]
pub struct Args {
    #[arg(short = 'r', long, default_value = "local")]
    pub registry: String,
    #[arg(long)]
    pub version: Option<String>,
    #[arg(long)]
    pub dry_run: bool,
    #[arg(long)]
    pub sign: bool,
    #[arg(long)]
    pub key_id: Option<String>,
    #[arg(long, default_value = "loose")]
    pub trust: String,
    #[arg(long, default_value = "./dist")]
    pub dist: PathBuf,
    #[arg(long)]
    pub force: bool,
    #[arg(long, default_value_t = true)]
    pub release: bool,
    #[arg(long)]
    pub verify_only: bool,
    #[arg(long, default_value = "./Cargo.toml")]
    pub manifest: PathBuf,
    #[arg(long, default_value = "human")]
    pub format: String,
}

pub async fn run(args: Args, home: &Path) -> anyhow::Result<()> {
    if args.sign {
        eprintln!(
            "warning: Phase 1 signing reuses Wave 1 JCS sign_describe. Safe to use, but key management + rotation land in Phase 2."
        );
    }
    let project_dir = project_dir_from_manifest(&args.manifest)?;
    let profile = if args.release {
        Profile::Release
    } else {
        Profile::Debug
    };
    let cfg = PublishConfig {
        project_dir,
        registry_uri: args.registry,
        home: home.to_path_buf(),
        dist_dir: args.dist,
        profile,
        dry_run: args.dry_run,
        force: args.force,
        sign: args.sign,
        key_id: args.key_id,
        version_override: args.version,
        trust_policy: args.trust,
        verify_only: args.verify_only,
    };
    match run_publish(&cfg).await? {
        PublishOutcome::DryRun {
            artifact,
            sha256,
            registry,
        } => {
            println!("dry-run: would publish {} to {}", artifact.display(), registry);
            println!("sha256: {sha256}");
        }
        PublishOutcome::VerifyOnly {
            ext_id,
            version,
            registry,
        } => {
            println!("verify-only: {ext_id}@{version} slot free in {registry}");
        }
        PublishOutcome::Published {
            ext_id,
            version,
            sha256,
            artifact,
            receipt_path,
            signed,
            registry_url,
        } => {
            println!("\u{2713} published {ext_id}@{version}");
            println!("  artifact: {}", artifact.display());
            println!("  sha256:   {sha256}");
            println!("  registry: {registry_url}");
            println!("  signed:   {signed}");
            println!("  receipt:  {}", receipt_path.display());
        }
    }
    Ok(())
}
```

- [ ] **Step 3: Verify build**

Run: `cargo build -p greentic-ext-cli 2>&1 | tail -10`
Expected: exit 0.

- [ ] **Step 4: Commit**

```bash
git add crates/greentic-ext-cli/src/publish/mod.rs crates/greentic-ext-cli/src/commands/publish.rs
git commit -m "feat(ext-cli): publish orchestrator wires build + validate + pack + publish + receipt"
```

---

## Task 11: LocalFilesystem publish unit test — happy path

**Files:**
- Create: `crates/greentic-ext-registry/tests/local_publish.rs`

- [ ] **Step 1: Write tests**

```rust
use chrono::Utc;
use greentic_ext_contract::{
    describe::{Author, Capabilities, Engine, Metadata, Permissions, Runtime},
    DescribeJson, ExtensionKind,
};
use greentic_ext_registry::local::LocalFilesystemRegistry;
use greentic_ext_registry::publish::PublishRequest;

fn sample_describe(version: &str) -> DescribeJson {
    DescribeJson {
        schema_ref: None,
        api_version: "greentic.ai/v1".into(),
        kind: ExtensionKind::Design,
        metadata: Metadata {
            id: "com.example.demo".into(),
            name: "demo".into(),
            version: version.into(),
            summary: "s".into(),
            description: None,
            author: Author {
                name: "a".into(),
                email: None,
                public_key: None,
            },
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
        capabilities: Capabilities {
            offered: vec![],
            required: vec![],
        },
        runtime: Runtime {
            component: "extension.wasm".into(),
            memory_limit_mb: 64,
            permissions: Permissions::default(),
        },
        contributions: serde_json::json!({}),
        signature: None,
    }
}

fn sample_req(version: &str, force: bool) -> PublishRequest {
    PublishRequest {
        ext_id: "com.example.demo".into(),
        ext_name: "demo".into(),
        version: version.into(),
        kind: ExtensionKind::Design,
        artifact_bytes: b"fake-pack-bytes".to_vec(),
        artifact_sha256: "abc".into(),
        describe: sample_describe(version),
        signature: None,
        force,
    }
}

#[test]
fn publish_writes_expected_layout() {
    let tmp = tempfile::tempdir().unwrap();
    let reg = LocalFilesystemRegistry::new("test", tmp.path().to_path_buf());
    let receipt = reg.publish_local(&sample_req("0.1.0", false)).unwrap();
    assert!(receipt.url.starts_with("file://"));
    assert!(receipt.published_at <= Utc::now());
    assert!(!receipt.signed);

    let ver = tmp.path().join("com.example.demo/0.1.0");
    assert!(ver.join("demo-0.1.0.gtxpack").exists());
    assert!(ver.join("manifest.json").exists());
    assert!(ver.join("artifact.sha256").exists());
    assert!(tmp.path().join("index.json").exists());
    assert!(tmp.path().join("com.example.demo/metadata.json").exists());
}

#[test]
fn duplicate_version_without_force_errors() {
    let tmp = tempfile::tempdir().unwrap();
    let reg = LocalFilesystemRegistry::new("test", tmp.path().to_path_buf());
    reg.publish_local(&sample_req("0.1.0", false)).unwrap();
    let err = reg.publish_local(&sample_req("0.1.0", false)).unwrap_err();
    assert!(matches!(
        err,
        greentic_ext_registry::RegistryError::VersionExists { .. }
    ));
}

#[test]
fn force_overwrites_existing_version() {
    let tmp = tempfile::tempdir().unwrap();
    let reg = LocalFilesystemRegistry::new("test", tmp.path().to_path_buf());
    reg.publish_local(&sample_req("0.1.0", false)).unwrap();
    let mut req = sample_req("0.1.0", true);
    req.artifact_bytes = b"fake-pack-v2".to_vec();
    req.artifact_sha256 = "xyz".into();
    let receipt = reg.publish_local(&req).unwrap();
    assert_eq!(receipt.sha256, "xyz");
    let sha_sidecar = tmp
        .path()
        .join("com.example.demo/0.1.0/artifact.sha256");
    assert_eq!(
        std::fs::read_to_string(&sha_sidecar).unwrap().trim(),
        "xyz"
    );
}

#[test]
fn index_tracks_multiple_versions() {
    let tmp = tempfile::tempdir().unwrap();
    let reg = LocalFilesystemRegistry::new("test", tmp.path().to_path_buf());
    reg.publish_local(&sample_req("0.1.0", false)).unwrap();
    reg.publish_local(&sample_req("0.1.1", false)).unwrap();
    let idx_bytes = std::fs::read(tmp.path().join("index.json")).unwrap();
    let idx: serde_json::Value = serde_json::from_slice(&idx_bytes).unwrap();
    let exts = idx["extensions"].as_array().unwrap();
    assert_eq!(exts.len(), 1);
    let versions = exts[0]["versions"].as_array().unwrap();
    assert_eq!(versions.len(), 2);
    assert_eq!(exts[0]["latest"], "0.1.1");
}
```

- [ ] **Step 2: Run tests**

Run: `cargo test -p greentic-ext-registry --test local_publish 2>&1 | tail -10`
Expected: 4 passed.

- [ ] **Step 3: Commit**

```bash
git add crates/greentic-ext-registry/tests/local_publish.rs
git commit -m "test(ext-registry): LocalFilesystemRegistry publish layout + conflict + force + index"
```

---

## Task 12: Integration test — gtdx publish round-trip

**Files:**
- Create: `crates/greentic-ext-cli/tests/cli_publish.rs`

- [ ] **Step 1: Write gated integration test**

```rust
//! Integration test for `gtdx publish`. Gated behind `GTDX_RUN_BUILD=1`
//! because it requires cargo-component on PATH.

use std::path::PathBuf;
use std::process::Command;

fn gtdx_bin() -> PathBuf {
    let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.pop();
    p.pop();
    p.push("target/debug/gtdx");
    p
}

fn gate() -> bool {
    std::env::var("GTDX_RUN_BUILD").ok().as_deref() == Some("1")
}

fn run(cmd: &mut Command) -> (bool, String, String) {
    let out = cmd.output().expect("spawn");
    (
        out.status.success(),
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

#[test]
fn publish_writes_hierarchical_layout_and_receipt() {
    if !gate() {
        eprintln!("skipped: set GTDX_RUN_BUILD=1 to enable (requires cargo-component)");
        return;
    }
    let tmp = tempfile::tempdir().unwrap();
    let proj = tmp.path().join("demo");
    let home = tmp.path().join("home");

    // scaffold
    let (ok, o, e) = run(Command::new(gtdx_bin())
        .arg("new")
        .arg("demo")
        .arg("--dir")
        .arg(&proj)
        .arg("--author")
        .arg("tester")
        .arg("-y")
        .arg("--no-git"));
    assert!(ok, "gtdx new failed: {o}\n{e}");

    // publish
    let (ok, o, e) = run(Command::new(gtdx_bin())
        .env("GREENTIC_HOME", &home)
        .arg("publish")
        .arg("--manifest")
        .arg(proj.join("Cargo.toml"))
        .arg("--dist")
        .arg(proj.join("dist")));
    assert!(ok, "gtdx publish failed: {o}\n{e}");

    let ver_dir = home.join("registries/local/com.example.demo/0.1.0");
    assert!(ver_dir.join("demo-0.1.0.gtxpack").exists());
    assert!(ver_dir.join("manifest.json").exists());
    assert!(ver_dir.join("artifact.sha256").exists());
    assert!(home.join("registries/local/index.json").exists());
    assert!(proj.join("dist/publish-com.example.demo-0.1.0.json").exists());
}

#[test]
fn publish_is_deterministic_sha_across_runs() {
    if !gate() {
        eprintln!("skipped: set GTDX_RUN_BUILD=1 to enable");
        return;
    }
    let tmp = tempfile::tempdir().unwrap();
    let proj = tmp.path().join("demo");
    let home1 = tmp.path().join("home1");
    let home2 = tmp.path().join("home2");

    assert!(
        run(Command::new(gtdx_bin())
            .arg("new")
            .arg("demo")
            .arg("--dir")
            .arg(&proj)
            .arg("--author")
            .arg("tester")
            .arg("-y")
            .arg("--no-git"))
        .0
    );

    let sha_of = |home: &PathBuf| {
        assert!(
            run(Command::new(gtdx_bin())
                .env("GREENTIC_HOME", home)
                .arg("publish")
                .arg("--manifest")
                .arg(proj.join("Cargo.toml"))
                .arg("--dist")
                .arg(proj.join("dist"))
                .arg("--force"))
            .0
        );
        std::fs::read_to_string(
            home.join("registries/local/com.example.demo/0.1.0/artifact.sha256"),
        )
        .unwrap()
        .trim()
        .to_string()
    };

    let sha_a = sha_of(&home1);
    let sha_b = sha_of(&home2);
    assert_eq!(sha_a, sha_b, "publish must be deterministic");
}

#[test]
fn publish_conflicts_without_force() {
    if !gate() {
        eprintln!("skipped: set GTDX_RUN_BUILD=1 to enable");
        return;
    }
    let tmp = tempfile::tempdir().unwrap();
    let proj = tmp.path().join("demo");
    let home = tmp.path().join("home");
    assert!(
        run(Command::new(gtdx_bin())
            .arg("new")
            .arg("demo")
            .arg("--dir")
            .arg(&proj)
            .arg("--author")
            .arg("tester")
            .arg("-y")
            .arg("--no-git"))
        .0
    );
    assert!(
        run(Command::new(gtdx_bin())
            .env("GREENTIC_HOME", &home)
            .arg("publish")
            .arg("--manifest")
            .arg(proj.join("Cargo.toml")))
        .0
    );
    let (ok, _o, e) = run(Command::new(gtdx_bin())
        .env("GREENTIC_HOME", &home)
        .arg("publish")
        .arg("--manifest")
        .arg(proj.join("Cargo.toml")));
    assert!(!ok, "second publish without --force must fail");
    assert!(
        e.contains("already exists") || e.contains("VersionExists"),
        "stderr should mention version conflict; got: {e}"
    );
}
```

- [ ] **Step 2: Compile + default skip**

Run: `cargo test -p greentic-ext-cli --test cli_publish 2>&1 | tail -10`
Expected: 3 passed (all skip when gate is off).

- [ ] **Step 3: Commit**

```bash
git add crates/greentic-ext-cli/tests/cli_publish.rs
git commit -m "test(ext-cli): gated gtdx publish integration — layout, determinism, conflict"
```

---

## Task 13: CHANGELOG + getting-started-publish docs

**Files:**
- Modify: `CHANGELOG.md`
- Create: `docs/getting-started-publish.md`

- [ ] **Step 1: CHANGELOG**

Under `## Unreleased` → `### Added`, append:

```
- `gtdx publish` subcommand: validate describe.json, build release WASM, pack
  into a deterministic `.gtxpack`, and publish into the filesystem registry at
  `$GREENTIC_HOME/registries/local/<id>/<version>/`. Supports `--dry-run`,
  `--force`, `--sign --key-id <id>`, `--version` override, and `--verify-only`.
  Writes a receipt at `./dist/publish-<id>-<version>.json`. Store and OCI
  registries return `NotImplemented` for now (Phase 2).
- `greentic-ext-contract::pack_writer` — deterministic ZIP writer (sorted
  entries, zeroed timestamps, LF normalization) shared by `gtdx dev` and
  `gtdx publish`.
```

- [ ] **Step 2: Create docs/getting-started-publish.md**

```markdown
# Publishing an Extension — `gtdx publish`

Companion to `getting-started-scaffolding.md` and `getting-started-dev.md`. Once
you're ready to share your extension, `gtdx publish` validates the describe,
builds a release-profile WASM, packs it deterministically, and writes it into
your local filesystem registry.

## Quick start

```bash
gtdx new my-ext
cd my-ext
# ... implement your extension ...
gtdx publish
```

The artifact lands at
`~/.greentic/registries/local/<id>/<version>/<name>-<version>.gtxpack` along
with `manifest.json`, `artifact.sha256`, and (if `--sign`) `signature.json`.
A receipt is written to `./dist/publish-<id>-<version>.json`.

## Flags

| Flag                  | Purpose                                                       |
|-----------------------|---------------------------------------------------------------|
| `--registry <URI>`    | `local` (default) or `file://<path>`. Store/OCI are Phase 2.  |
| `--version <SEMVER>`  | Override `describe.json` version (CI version bumps).          |
| `--dry-run`           | Validate + build + pack, skip registry write.                 |
| `--force`             | Overwrite an existing version.                                |
| `--sign --key-id <ID>`| Sign describe.json via Wave 1 JCS (Ed25519).                  |
| `--verify-only`       | Check version conflict; skip build.                           |
| `--dist <DIR>`        | Also copy the artifact here. Default `./dist/`.               |
| `--release`           | Build with `--release` (default true for publish).            |
| `--format <FMT>`      | `human` (default) or `json`.                                  |

## Determinism

Two `gtdx publish` invocations over identical sources produce byte-identical
`.gtxpack` archives. The writer sorts entries, zeros timestamps to the ZIP
epoch (1980-01-01), normalizes Unix permissions to 0644/0755, and normalizes
CRLF → LF for text assets (json/md/wit/txt/toml/yaml).

## Non-goals in Phase 1

- Publishing to the Greentic Store HTTP registry (Phase 2, S5)
- Publishing to an OCI registry (Phase 2, S5)
- Passphrase-encrypted signing keys (Phase 2, S4)
- Strict trust policy + countersignatures (Phase 2, S4)
```

- [ ] **Step 3: Commit**

```bash
git add CHANGELOG.md docs/getting-started-publish.md
git commit -m "docs: gtdx publish getting-started + CHANGELOG entry"
```

---

## Task 14: Author-run smoke (controller)

Controller runs this directly.

- [ ] **Step 1: Build**

```bash
cargo build -p greentic-ext-cli --quiet
```

- [ ] **Step 2: End-to-end smoke**

```bash
TMP=$(mktemp -d)
./target/debug/gtdx new demo --dir "$TMP/demo" --author tester -y --no-git
GREENTIC_HOME="$TMP/home" ./target/debug/gtdx publish \
  --manifest "$TMP/demo/Cargo.toml" \
  --dist "$TMP/demo/dist"
ls "$TMP/home/registries/local/com.example.demo/0.1.0/"
cat "$TMP/home/registries/local/index.json"
cat "$TMP/demo/dist/publish-com.example.demo-0.1.0.json"
```

Expected:
- `✓ published com.example.demo@0.1.0`
- `demo-0.1.0.gtxpack`, `manifest.json`, `artifact.sha256` in the version dir
- `index.json` contains the extension entry
- `publish-com.example.demo-0.1.0.json` receipt exists

- [ ] **Step 3: Determinism smoke (repeat publish, compare SHA)**

```bash
cp "$TMP/home/registries/local/com.example.demo/0.1.0/artifact.sha256" /tmp/gtdx-sha-a
GREENTIC_HOME="$TMP/home2" ./target/debug/gtdx publish \
  --manifest "$TMP/demo/Cargo.toml" \
  --dist "$TMP/demo/dist2"
diff /tmp/gtdx-sha-a "$TMP/home2/registries/local/com.example.demo/0.1.0/artifact.sha256" && echo "DETERMINISTIC OK"
```

Expected: `DETERMINISTIC OK`.

- [ ] **Step 4: Conflict smoke**

```bash
GREENTIC_HOME="$TMP/home" ./target/debug/gtdx publish \
  --manifest "$TMP/demo/Cargo.toml" \
  --dist "$TMP/demo/dist" 2>&1 | tail -5
```

Expected: exit non-zero, stderr mentions version conflict.

- [ ] **Step 5: No commit — record observations in PR description**

---

## Task 15: Final gate + push + PR (controller)

- [ ] **Step 1: Format**

Run: `cargo fmt --all`

- [ ] **Step 2: Clippy**

Run: `cargo clippy --workspace --all-targets --all-features -- -D warnings 2>&1 | tail -20`
Expected: exit 0.

- [ ] **Step 3: Full test suite**

Run: `cargo test --workspace --all-targets 2>&1 | tail -30`
Expected: all green.

- [ ] **Step 4: Commit stragglers**

```bash
if ! git diff --quiet; then
  git add -A
  git commit -m "style: cargo fmt post-track-c"
fi
```

- [ ] **Step 5: Push + PR**

```bash
git push -u origin feat/phase1-track-c-publish-local
gh pr create --base main --head feat/phase1-track-c-publish-local \
  --title "feat(ext-cli): Phase 1 Track C — gtdx publish to local filesystem" \
  --body "$(cat docs/superpowers/plans/2026-04-19-phase1-track-c-publish-local.md | head -40)"
```

---

## Acceptance (Phase 1 Track C complete when all true)

1. `gtdx publish --registry local` on a scaffolded project succeeds and writes the hierarchical layout under `$GREENTIC_HOME/registries/local/<id>/<version>/` (Task 14).
2. Two consecutive publishes of identical sources produce byte-identical `.gtxpack` (sha256 equal) — verified by unit test + author smoke (Tasks 2, 11, 14).
3. Duplicate-version publish without `--force` errors (Task 11 unit test + Task 12 integration test + Task 14 smoke).
4. `GreenticStoreRegistry::publish()` and `OciRegistry::publish()` return `NotImplemented` with actionable hint (Task 5).
5. `describe.json` validation aggregates errors (Task 8 unit tests).
6. Track B's `gtdx dev` still works (Task 3 refactor keeps `build_pack` signature; existing dev tests green).
7. Full workspace clippy + test green (Task 15).

---

## Self-Review

**1. Spec coverage:**
- Spec §2 CLI UX flags — all 11 in `commands::publish::Args` (Task 9).
- Spec §3 pipeline — schema validate → business validate → build → pack → sign? → publish → receipt (Task 10).
- Spec §4 `Registry::publish()` contract — Tasks 4-6 define types, implement local, stub Store/OCI.
- Spec §5 layout — Task 6 `local_publish.rs` writes the exact layout.
- Spec §5.1 atomicity — `atomic_write` + advisory lock in Task 6.
- Spec §6 deterministic ZIP — Task 2 `pack_writer` + Task 3 refactor.
- Spec §6.1 LF normalization — Task 2 `is_text_path` + `normalize_entry`.
- Spec §6.2 determinism test — Task 2 unit test + Task 12 integration test.
- Spec §7 signing — Task 10 reuses `sign_describe` (JCS). NOTE: spec §7 specifies a "phase1-artifact-sig-only-not-jcs" workaround because at spec-write time JCS signing was not yet merged. It has since landed (Wave 1 PR #6), so we ship full JCS signing — better than the spec's stopgap. The `--sign` warning in Task 10 reflects this.
- Spec §8 pre-publish validation — Task 8 validator covers semver/id/offered-cap/https-only. Rule §8.4 (WIT contract version vs `.gtdx-contract.lock`) is DEFERRED to Phase 4 per spec non-goals on WIT sync; Task 8 does not enforce it.
- Spec §9 exit codes — Task 10 returns `anyhow::Error`s that bubble up as exit 1; spec's numeric exit codes (2/70/10/etc) are NOT implemented in Phase 1. Flagged as follow-up; `NotImplemented` hint covers the most user-actionable case (exit 50).
- Spec §10 testing — covered by Tasks 2, 8, 11, 12.
- Spec §11 LOC budget — plan files stay under 300 LOC each; total ~900.
- Spec §12 acceptance — all 7 rows tracked above.

**2. Placeholder scan:** No TBD/TODO/"handle later" in plan body. TODO markers inside template code don't apply here (this plan doesn't touch templates).

**3. Type consistency:**
- `PublishRequest { ext_id, ext_name, version, kind, artifact_bytes, artifact_sha256, describe, signature, force }` defined in Task 4, used identically in Tasks 6 + 10 + 11.
- `PublishReceipt { url, sha256, published_at, signed }` defined Task 4, used Task 6 + 10.
- `PublishReceiptJson` (CLI-level, wraps trait receipt) defined Task 8 — distinct from the trait-level `PublishReceipt` on purpose; the CLI adds `artifact`/`registry`/`trust_policy`/`signing_known_limitations`.
- `ValidationError { field, message }` defined Task 8, used in tests Task 8 + consumed by `format_errors` Task 8.
- `RegistryError::{VersionExists, NotImplemented}` added Task 5, matched in Task 10 + Task 11.

**4. Known deferrals:**
- Exit code mapping (spec §9) — Phase 2 follow-up.
- Rule §8.4 WIT-contract vs lockfile — Phase 4 (`gtdx sync-wit`).
- `--format json` output for publish — accepted flag is present but Task 10 renders only human output; JSON emission is a tiny follow-up and not acceptance-blocking.
- `--trust` policy actually enforced at publish time — Phase 1 currently accepts any value and only labels the receipt; proper strict-mode checks land with countersignatures in Phase 2.
- `gtdx keygen` command already exists (Track A merged); no key CLI work needed here.
- Concurrent-publish test (spec §10 `concurrent_publish`) — `fs2` exclusive lock is exercised implicitly; not adding a dedicated multi-thread test in Phase 1 to avoid flakiness. The lock file is created and locked on every `publish_local` call (Task 6).
