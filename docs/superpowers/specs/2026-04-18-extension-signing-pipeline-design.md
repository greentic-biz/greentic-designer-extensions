# Extension Signing Pipeline — Design

- **Date:** 2026-04-18
- **Status:** Draft, pending user review
- **Branch:** `spec/extension-signing-pipeline` (this repo)
- **Scope:** Cross-repo — `greentic-biz/greentic-designer-extensions` (core signing infra) + `greentic-biz/greentic-deployer-extensions` (signed ref exts) + `greenticai/greentic-deployer` (runtime dep bump + new `SingleVm` built-in variant).
- **Related:** Phase B #1 of the deploy-extension migration. Parent: `/home/bimbim/.claude/projects/-home-bimbim-works-greentic/memory/deploy-extension-migration.md`.

## 1. Context & motivation

### Current state

`greentic-biz/greentic-designer-extensions` already ships most of an ed25519 signing stack:

- `greentic-ext-contract::signature::verify_ed25519(pubkey_b64, sig_b64, payload)` — production-ready.
- `greentic-ext-contract::describe::Signature { algorithm, publicKey, value }` — schema field exists, optional.
- `greentic-ext-registry::lifecycle::verify_signature` — trust policies (`Strict` / `Normal` / `Loose`) defined.
- Test roundtrip in `tests/signature_rt.rs` — primitive sign/verify confirmed.

What's missing:

1. **Canonicalization bug.** `verify_signature` at `greentic-ext-registry/src/lifecycle.rs:101` passes `serde_json::to_vec(&artifact.describe)` — which includes the `signature` field itself — as the signing payload. Chicken-and-egg; no describe.json can ever be validly signed.
2. **No signing CLI.** `greentic-ext-cli` (gtdx) ships `validate / list / install / search / info / login / registries / doctor / version` — no `sign / verify / keygen`.
3. **No private-key storage convention.** `credentials.toml` is for tokens; no ed25519 key file / env var pattern.
4. **No runtime verify hook.** `ExtensionRuntime::register_loaded_from_dir()` reads describe.json but never calls `verify_ed25519`. `GREENTIC_EXT_ALLOW_UNSIGNED` env escape hatch referenced in deployer PR#121 spec but not implemented anywhere.
5. **No CI signing step.** Reference extensions (`deploy-desktop@0.1.0` merged unsigned; `bundle-standard@0.1.0` same) ship without signatures. No workflow to apply signatures at release.
6. **No second unblocked deploy ref ext.** Only `deploy-desktop` ships. `deploy-single-vm` — also native-backend-backed, no pack dependency — has never been built.

### Goal

End-to-end signed extensions with:

1. Fixed canonicalization via RFC 8785 JCS (JSON Canonicalization Scheme).
2. `gtdx sign / verify / keygen` subcommands.
3. Single-org keypair in 1Password, distributed as GH Actions secret `EXT_SIGNING_KEY_PEM`.
4. Strict-by-default runtime verification with `GREENTIC_EXT_ALLOW_UNSIGNED=1` dev escape hatch.
5. Signed `deploy-desktop@0.2.0` + new `deploy-single-vm@0.1.0` ref exts.
6. Deployer v0.4.54 with `BuiltinBackendId::SingleVm` variant, consuming new `greentic-ext-runtime` rev.

### Non-goals (explicit)

- **No `TrustPolicy::Strict` countersignature flow.** Phase C work. `Strict` variant stays declared-but-unimplemented.
- **No key rotation automation.** Documented procedure only.
- **No revocation list.** Mitigation is key rotation + resign everything.
- **No hardware keys (YubiKey / HSM).** Phase C.
- **No per-extension keypairs.** Single org key.
- **No deployer-side `ext apply` dispatch wiring** — that's Phase B #4. This spec ships metadata + load-time verification only.
- **No pack-authoring for aws/gcp/azure/helm/etc.** 11 of 13 built-in backends still blocked on pack infrastructure (Phase B #4). Only `Desktop` + `SingleVm` ref exts in scope here.
- **No `greentic-bundle-extensions` signing adoption in this spec** — follow-up PR, same pattern.

## 2. Architecture

### Repo scope

| Repo | Changes in this spec |
| --- | --- |
| `greentic-biz/greentic-designer-extensions` | **Wave 1.** Fix canonicalization, add `sign_describe` / `verify_describe` helpers, add `sign / verify / keygen` subcommands to `gtdx`, add runtime verify hook, update registry to new canonical path, bump `greentic-ext-runtime` patch. |
| `greentic-biz/greentic-deployer-extensions` | **Wave 2.** `build.sh` env-aware signing, `deploy-desktop@0.2.0` signed rebuild, new `deploy-single-vm@0.1.0` crate, CI `EXT_SIGNING_KEY_PEM` secret wired, `CI_REQUIRE_SIGNED` guardrail, gtdx installed in CI. |
| `greenticai/greentic-deployer` | **Wave 3.** Bump pinned rev of `greentic-ext-runtime` + `greentic-ext-contract`. Add `BuiltinBackendId::SingleVm` + `BuiltinBackendHandlerId::SingleVm` variants with `FromStr` / `as_str` / `handler_matches`. Adjust `tests/ext_{loader,dispatch}.rs` to set `GREENTIC_EXT_ALLOW_UNSIGNED=1` for in-repo fixture. Bump `0.4.53 → 0.4.54`. |

### Ship order

Wave 1 merges first. Wave 2 + Wave 3 ship in parallel (independent — zero shared code). Release-tag order: deployer v0.4.54 cuts first (< 1 hour), then ref ext artifacts published. CHANGELOGs cross-link.

### Dataflow

```
Build time (CI, main branch push):
  GH Secret: EXT_SIGNING_KEY_PEM (PKCS#8 PEM ed25519)
       │
       ▼
  CI workflow env: GREENTIC_EXT_SIGNING_KEY_PEM=<secret value>
       │
       ▼
  build.sh:
    1. cargo component build --release --target wasm32-wasip1 → extension.wasm
    2. gtdx sign describe.json   ← reads $GREENTIC_EXT_SIGNING_KEY_PEM
       • clone describe, set .signature = None
       • serde_jcs::to_vec(&cloned) → canonical bytes (RFC 8785)
       • ed25519 sign(PKCS8 key, canonical bytes) → sig_bytes
       • inject {"signature":{"algorithm":"ed25519","publicKey":<b64>,"value":<b64>}}
       • write describe.json in-place (pretty-printed for diff-friendly commits)
    3. wasm-tools validate extension.wasm
    4. zip -X -r greentic.deploy-<id>-<version>.gtxpack describe.json extension.wasm schemas assets

Install time:
  User: unzip .gtxpack → ~/.greentic/extensions/deploy/<id>/
  (no verification at install — loader's job)

Load time (deployer / runner):
  greentic-ext-runtime::ExtensionRuntime::register_loaded_from_dir(path)
       │
       ├─ read describe.json
       ├─ if env GREENTIC_EXT_ALLOW_UNSIGNED is set:
       │     tracing::warn! "signature verification skipped"
       │     return Ok(())
       ├─ else:
       │     greentic_ext_contract::verify_describe(&describe)
       │       • describe.signature.ok_or(missing)?
       │       • canonical = jcs::to_vec(&strip_signature(&describe))
       │       • verify_ed25519(sig.publicKey, sig.value, canonical)
       │     on Err → RuntimeError::SignatureInvalid (reject instantiation)
       │     on Ok  → tracing::info! "signature verified", key_prefix
       ▼
  wasmtime::Component::from_file(extension.wasm) → instantiate
```

### Trust boundaries

- **Fork PRs:** GH secrets unavailable. `build.sh` detects empty `GREENTIC_EXT_SIGNING_KEY_PEM` → dev-mode → unsigned build. CI-PR workflow passes; malicious forks cannot sign artifacts with org key.
- **Main branch push / release:** Secret available → signed build. Release workflow with `CI_REQUIRE_SIGNED=1` guardrail fails if signature missing.
- **Developer machines:** No signing key. `build.sh` dev-mode → unsigned artifact. Local testing works via `GREENTIC_EXT_ALLOW_UNSIGNED=1`.

### Key provisioning (one-time)

```bash
# Generate locally
gtdx keygen --out /tmp/ext-signing.pem
# stderr: "public key (base64): gN8eF3jT...Qp+w="

# Store in 1Password
op document create /tmp/ext-signing.pem \
    --title "greentic-ext-signing-key" \
    --vault "Engineering" \
    --tags "greentic,signing,ed25519"

# Distribute to 3 repo secrets (same value)
for repo in \
    greentic-biz/greentic-deployer-extensions \
    greentic-biz/greentic-bundle-extensions \
    greentic-biz/greentic-designer-extensions; do
    gh secret set EXT_SIGNING_KEY_PEM --repo "$repo" < /tmp/ext-signing.pem
done

# Destroy local copy
shred -u /tmp/ext-signing.pem
```

Public key prefix (first 16 chars) documented in each consumer README for audit. Rotation procedure: generate new keypair, update 1Password + GH secrets, resign all ref exts, bump ref ext versions, publish. Old signatures still valid as historical artifacts (no revocation mechanism in Phase B).

## 3. Wave 1 — canonicalization fix + contract helpers

### File changes

| File | Change |
| --- | --- |
| `crates/greentic-ext-contract/Cargo.toml` | Add dep `serde_jcs = "0.1"` |
| `crates/greentic-ext-contract/src/signature.rs` | Add `canonical_signing_payload`, `sign_describe`, `verify_describe` |
| `crates/greentic-ext-contract/src/lib.rs` | Re-export new helpers |
| `crates/greentic-ext-contract/tests/signature_rt.rs` | 3 new tests |
| `crates/greentic-ext-registry/src/lifecycle.rs` | Replace buggy `verify_signature` body with `verify_describe` call |

### `canonical_signing_payload`

```rust
// crates/greentic-ext-contract/src/signature.rs
use crate::describe::DescribeJson;
use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;
use ed25519_dalek::{Signer, SigningKey, VerifyingKey, Signature as EdSignature};
use sha2::{Digest, Sha256};

/// Canonicalize describe.json for signing — strip `.signature` field,
/// emit RFC 8785 JCS bytes. Output is deterministic across languages and
/// serde versions.
pub fn canonical_signing_payload(describe: &DescribeJson) -> Result<Vec<u8>, ContractError> {
    let mut clone = describe.clone();
    clone.signature = None;
    serde_jcs::to_vec(&clone).map_err(|e| ContractError::Canonicalize(e.to_string()))
}
```

### `sign_describe` — mutate in place

```rust
pub fn sign_describe(
    describe: &mut DescribeJson,
    signing_key: &SigningKey,
) -> Result<(), ContractError> {
    // Defensive: ensure .signature is None before canonicalize, otherwise the
    // caller could pass a pre-signed describe and we'd canonicalize the wrong bytes.
    describe.signature = None;
    let payload = canonical_signing_payload(describe)?;
    let sig: EdSignature = signing_key.sign(&payload);
    let pubkey_b64 = B64.encode(signing_key.verifying_key().to_bytes());
    let sig_b64 = B64.encode(sig.to_bytes());
    describe.signature = Some(crate::describe::Signature {
        algorithm: "ed25519".into(),
        public_key: pubkey_b64,
        value: sig_b64,
    });
    Ok(())
}
```

### `verify_describe`

```rust
pub fn verify_describe(describe: &DescribeJson) -> Result<(), ContractError> {
    let sig = describe.signature.as_ref()
        .ok_or_else(|| ContractError::SignatureInvalid("missing signature field".into()))?;
    if sig.algorithm != "ed25519" {
        return Err(ContractError::SignatureInvalid(
            format!("unsupported algorithm: {}", sig.algorithm),
        ));
    }
    let payload = canonical_signing_payload(describe)?;
    verify_ed25519(&sig.public_key, &sig.value, &payload)
}
```

### New `ContractError` variant

```rust
#[derive(Debug, thiserror::Error)]
pub enum ContractError {
    // existing variants preserved ...
    #[error("signature invalid: {0}")]
    SignatureInvalid(String),

    #[error("canonicalization failed: {0}")]
    Canonicalize(String),
}
```

### Registry lifecycle update

```rust
// crates/greentic-ext-registry/src/lifecycle.rs
fn verify_signature(
    artifact: &ExtensionArtifact,
    policy: TrustPolicy,
) -> Result<(), RegistryError> {
    match policy {
        TrustPolicy::Loose => Ok(()),
        TrustPolicy::Strict | TrustPolicy::Normal => {
            greentic_ext_contract::verify_describe(&artifact.describe)
                .map_err(|e| RegistryError::SignatureInvalid(e.to_string()))
        }
    }
}
```

### Contract tests

```rust
// crates/greentic-ext-contract/tests/signature_rt.rs
use greentic_ext_contract::*;
use ed25519_dalek::SigningKey;
use rand::rngs::OsRng;

#[test]
fn sign_describe_roundtrip() {
    let sk = SigningKey::generate(&mut OsRng);
    let mut d = sample_describe();
    assert!(d.signature.is_none());
    sign_describe(&mut d, &sk).expect("sign");
    assert!(d.signature.is_some());
    verify_describe(&d).expect("verify");
}

#[test]
fn sign_describe_tamper_detected() {
    let sk = SigningKey::generate(&mut OsRng);
    let mut d = sample_describe();
    sign_describe(&mut d, &sk).expect("sign");
    d.metadata.version = "99.99.99".into();
    let err = verify_describe(&d).unwrap_err();
    assert!(matches!(err, ContractError::SignatureInvalid(_)));
}

#[test]
fn sign_describe_reencoding_stable() {
    let sk = SigningKey::generate(&mut OsRng);
    let mut d1 = sample_describe();
    sign_describe(&mut d1, &sk).expect("sign");
    let json = serde_json::to_string(&d1).unwrap();
    let d2: DescribeJson = serde_json::from_str(&json).unwrap();
    verify_describe(&d2).expect("verify after round-trip");
}

fn sample_describe() -> DescribeJson {
    // build a valid DescribeJson fixture — helper shared with other tests.
    // (Exact body depends on existing DescribeJson shape — see describe.rs.)
    todo!("use existing sample fixture from tests/common.rs or define here")
}
```

(The `sample_describe` helper is filled in at impl time from existing test fixture patterns in the repo.)

### Wave 1 commit order

1. `crates/greentic-ext-contract/src/signature.rs` — add `canonical_signing_payload`, `sign_describe`, `verify_describe` + `ContractError::Canonicalize` variant.
2. Add `serde_jcs` dep to `greentic-ext-contract/Cargo.toml`.
3. `crates/greentic-ext-contract/tests/signature_rt.rs` — 3 new tests.
4. `crates/greentic-ext-registry/src/lifecycle.rs` — replace `verify_signature` body with `verify_describe` call.
5. Runtime hook (see §4).
6. gtdx CLI subcommands (see §5).
7. Bump patch version of `greentic-ext-runtime`.

## 4. Wave 1 — runtime verification hook

### File changes

| File | Change |
| --- | --- |
| `crates/greentic-ext-runtime/src/runtime.rs` | Add `verify_signature_or_bypass` helper, call it from `register_loaded_from_dir` |
| `crates/greentic-ext-runtime/src/error.rs` | Add `RuntimeError::SignatureInvalid` variant |
| `crates/greentic-ext-runtime/tests/signature_gate.rs` | New integration test file, 5 tests |

### Hook implementation

```rust
// crates/greentic-ext-runtime/src/runtime.rs
use greentic_ext_contract::{verify_describe, DescribeJson};

impl ExtensionRuntime {
    pub fn register_loaded_from_dir(&mut self, dir: &Path) -> Result<(), RuntimeError> {
        let describe_path = dir.join("describe.json");
        let raw = std::fs::read_to_string(&describe_path)
            .map_err(|e| RuntimeError::LoadFailed(format!("read {}: {e}", describe_path.display())))?;
        let describe: DescribeJson = serde_json::from_str(&raw)
            .map_err(|e| RuntimeError::LoadFailed(format!("parse {}: {e}", describe_path.display())))?;

        self.verify_signature_or_bypass(&describe)?;

        // existing instantiation flow unchanged:
        let component_path = dir.join(&describe.runtime.component);
        let component = wasmtime::Component::from_file(&self.engine, &component_path)
            .map_err(|e| RuntimeError::LoadFailed(format!("wasmtime: {e}")))?;
        // ... register component, emit loaded event, etc.
        Ok(())
    }

    fn verify_signature_or_bypass(&self, describe: &DescribeJson) -> Result<(), RuntimeError> {
        if std::env::var("GREENTIC_EXT_ALLOW_UNSIGNED").is_ok() {
            tracing::warn!(
                extension_id = %describe.metadata.id,
                "GREENTIC_EXT_ALLOW_UNSIGNED is set — signature verification skipped"
            );
            return Ok(());
        }
        verify_describe(describe).map_err(|e| RuntimeError::SignatureInvalid {
            extension_id: describe.metadata.id.clone(),
            reason: e.to_string(),
        })?;
        let pub_prefix = describe.signature.as_ref()
            .map(|s| &s.public_key[..16.min(s.public_key.len())])
            .unwrap_or("?");
        tracing::info!(
            extension_id = %describe.metadata.id,
            key_prefix = %pub_prefix,
            "extension signature verified"
        );
        Ok(())
    }
}
```

### `RuntimeError` new variant

```rust
#[derive(thiserror::Error, Debug)]
pub enum RuntimeError {
    #[error("load failed: {0}")]
    LoadFailed(String),

    #[error("signature verification failed for extension '{extension_id}': {reason}\n\
             hint: reinstall a signed extension, or set GREENTIC_EXT_ALLOW_UNSIGNED=1 for dev")]
    SignatureInvalid { extension_id: String, reason: String },

    // ... other existing variants preserved
}
```

### Integration tests

```rust
// crates/greentic-ext-runtime/tests/signature_gate.rs
use greentic_ext_runtime::{ExtensionRuntime, RuntimeConfig, RuntimeError};
use ed25519_dalek::SigningKey;
use rand::rngs::OsRng;
use std::path::Path;

#[test]
fn rejects_unsigned_by_default() {
    let tmp = tempfile::tempdir().unwrap();
    write_fixture_ext(tmp.path(), /*signed=*/ false);
    let mut rt = ExtensionRuntime::new(RuntimeConfig::default()).unwrap();
    let err = rt.register_loaded_from_dir(tmp.path()).unwrap_err();
    assert!(matches!(err, RuntimeError::SignatureInvalid { .. }));
}

#[test]
fn rejects_tampered_signature() {
    let tmp = tempfile::tempdir().unwrap();
    write_fixture_ext_signed_then_tamper(tmp.path());
    let mut rt = ExtensionRuntime::new(RuntimeConfig::default()).unwrap();
    let err = rt.register_loaded_from_dir(tmp.path()).unwrap_err();
    assert!(matches!(err, RuntimeError::SignatureInvalid { .. }));
}

#[test]
fn accepts_signed_by_default() {
    let tmp = tempfile::tempdir().unwrap();
    write_fixture_ext(tmp.path(), /*signed=*/ true);
    let mut rt = ExtensionRuntime::new(RuntimeConfig::default()).unwrap();
    rt.register_loaded_from_dir(tmp.path()).expect("load signed");
}

#[test]
fn allow_unsigned_env_bypasses() {
    let _guard = env_set("GREENTIC_EXT_ALLOW_UNSIGNED", "1");
    let tmp = tempfile::tempdir().unwrap();
    write_fixture_ext(tmp.path(), /*signed=*/ false);
    let mut rt = ExtensionRuntime::new(RuntimeConfig::default()).unwrap();
    rt.register_loaded_from_dir(tmp.path()).expect("load unsigned with env");
}

#[test]
fn allow_unsigned_env_bypasses_even_if_tampered() {
    // By design: env set = skip verification entirely, don't even try.
    let _guard = env_set("GREENTIC_EXT_ALLOW_UNSIGNED", "1");
    let tmp = tempfile::tempdir().unwrap();
    write_fixture_ext_signed_then_tamper(tmp.path());
    let mut rt = ExtensionRuntime::new(RuntimeConfig::default()).unwrap();
    rt.register_loaded_from_dir(tmp.path()).expect("load tampered with env");
}

// Helper fns `write_fixture_ext`, `write_fixture_ext_signed_then_tamper`, `env_set`
// defined in tests/common.rs — use RAII scope guard for env safety.
```

## 5. Wave 1 — gtdx CLI subcommands

### File changes

| File | Change |
| --- | --- |
| `crates/greentic-ext-cli/src/main.rs` | Add `Sign`, `Verify`, `Keygen` to `Command` enum + dispatch arms |
| `crates/greentic-ext-cli/src/commands/sign.rs` | New |
| `crates/greentic-ext-cli/src/commands/verify.rs` | New |
| `crates/greentic-ext-cli/src/commands/keygen.rs` | New |
| `crates/greentic-ext-cli/Cargo.toml` | Add ed25519-dalek pkcs8 feature, `rand`, `pkcs8` |
| `crates/greentic-ext-cli/tests/sign_verify_cmd.rs` | New integration tests |

### `gtdx keygen`

```rust
// crates/greentic-ext-cli/src/commands/keygen.rs
use anyhow::{Context, Result};
use clap::Parser;
use ed25519_dalek::pkcs8::EncodePrivateKey;
use ed25519_dalek::SigningKey;
use rand::rngs::OsRng;
use std::io::Write;
use std::path::{Path, PathBuf};

#[derive(Parser)]
pub struct Args {
    /// Write private key to this file instead of stdout (mode 0600).
    #[arg(long)]
    pub out: Option<PathBuf>,
}

pub fn run(args: Args) -> Result<()> {
    let signing_key = SigningKey::generate(&mut OsRng);
    let pem = signing_key
        .to_pkcs8_pem(pkcs8::LineEnding::LF)
        .context("encode PKCS8 PEM")?;

    let pubkey_b64 = base64::engine::general_purpose::STANDARD
        .encode(signing_key.verifying_key().to_bytes());

    match args.out {
        Some(path) => {
            write_mode_0600(&path, pem.as_bytes())
                .with_context(|| format!("write {}", path.display()))?;
            eprintln!("private key written: {}", path.display());
        }
        None => {
            print!("{}", pem.as_str());
        }
    }

    eprintln!("public key (base64): {pubkey_b64}");
    eprintln!();
    eprintln!("Next steps:");
    eprintln!("  1. Store the private key in your org vault (e.g., 1Password).");
    eprintln!("  2. Add as GH Actions secret: gh secret set EXT_SIGNING_KEY_PEM");
    eprintln!("  3. Distribute the public key via describe.json.signature.publicKey.");

    Ok(())
}

#[cfg(unix)]
fn write_mode_0600(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    use std::os::unix::fs::OpenOptionsExt;
    let mut f = std::fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .mode(0o600)
        .open(path)?;
    f.write_all(bytes)?;
    Ok(())
}

#[cfg(not(unix))]
fn write_mode_0600(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    std::fs::write(path, bytes)
}
```

### `gtdx sign`

```rust
// crates/greentic-ext-cli/src/commands/sign.rs
use anyhow::{Context, Result};
use clap::Parser;
use ed25519_dalek::pkcs8::DecodePrivateKey;
use ed25519_dalek::SigningKey;
use greentic_ext_contract::DescribeJson;
use std::path::PathBuf;

#[derive(Parser)]
pub struct Args {
    /// Path to describe.json to sign in-place.
    pub describe_path: PathBuf,

    /// Read PKCS8 PEM private key from this file. Mutually exclusive with --key-env.
    #[arg(long, conflicts_with = "key_env")]
    pub key: Option<PathBuf>,

    /// Read PKCS8 PEM private key from this env var.
    #[arg(long, default_value = "GREENTIC_EXT_SIGNING_KEY_PEM")]
    pub key_env: String,
}

pub fn run(args: Args) -> Result<()> {
    let pem = match &args.key {
        Some(path) => std::fs::read_to_string(path)
            .with_context(|| format!("read key {}", path.display()))?,
        None => std::env::var(&args.key_env).with_context(|| {
            format!(
                "env var ${} not set (use --key <path> or export the env var)",
                args.key_env
            )
        })?,
    };

    let signing_key = SigningKey::from_pkcs8_pem(&pem)
        .map_err(|e| anyhow::anyhow!("parse PKCS8 PEM private key: {e}"))?;

    let raw = std::fs::read_to_string(&args.describe_path)
        .with_context(|| format!("read {}", args.describe_path.display()))?;
    let mut describe: DescribeJson = serde_json::from_str(&raw).context("parse describe.json")?;

    greentic_ext_contract::sign_describe(&mut describe, &signing_key).context("sign describe")?;

    // Pretty-print for diff-friendly commits. JCS canonicalization happens inside
    // sign_describe regardless of on-disk formatting, so readability is free.
    let out = serde_json::to_string_pretty(&describe)? + "\n";
    std::fs::write(&args.describe_path, out)
        .with_context(|| format!("write {}", args.describe_path.display()))?;

    let pub_b64 = &describe.signature.as_ref().unwrap().public_key;
    eprintln!(
        "signed {} with key {}",
        args.describe_path.display(),
        &pub_b64[..16.min(pub_b64.len())],
    );
    Ok(())
}
```

### `gtdx verify`

```rust
// crates/greentic-ext-cli/src/commands/verify.rs
use anyhow::{Context, Result};
use clap::Parser;
use greentic_ext_contract::DescribeJson;
use std::path::{Path, PathBuf};

#[derive(Parser)]
pub struct Args {
    /// Path to verify. Accepts:
    ///   - describe.json file (verifies inline signature)
    ///   - extension directory (reads describe.json inside)
    ///   - .gtxpack archive (unzips to temp, verifies inside)
    pub path: PathBuf,
}

pub fn run(args: Args) -> Result<()> {
    let describe = load_describe_from_any_source(&args.path)?;
    greentic_ext_contract::verify_describe(&describe)
        .map_err(|e| anyhow::anyhow!("signature invalid: {e}"))?;
    let pub_b64 = &describe.signature.as_ref().unwrap().public_key;
    println!(
        "OK  {} v{} signed by {}",
        describe.metadata.id,
        describe.metadata.version,
        &pub_b64[..16.min(pub_b64.len())],
    );
    Ok(())
}

fn load_describe_from_any_source(path: &Path) -> Result<DescribeJson> {
    if path.is_file() {
        let ext = path.extension().and_then(|s| s.to_str());
        match ext {
            Some("json") => load_describe_file(path),
            Some("gtxpack") | Some("zip") => load_describe_from_gtxpack(path),
            other => anyhow::bail!(
                "unsupported file extension: {:?} (expected .json or .gtxpack)",
                other
            ),
        }
    } else if path.is_dir() {
        load_describe_file(&path.join("describe.json"))
    } else {
        anyhow::bail!("not a file or directory: {}", path.display())
    }
}

fn load_describe_file(path: &Path) -> Result<DescribeJson> {
    let raw = std::fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    serde_json::from_str(&raw).with_context(|| format!("parse {}", path.display()))
}

fn load_describe_from_gtxpack(pack_path: &Path) -> Result<DescribeJson> {
    let file = std::fs::File::open(pack_path)
        .with_context(|| format!("open {}", pack_path.display()))?;
    let mut zip = zip::ZipArchive::new(file).context("open zip")?;
    let mut entry = zip
        .by_name("describe.json")
        .context("describe.json missing from .gtxpack")?;
    let mut buf = String::new();
    use std::io::Read;
    entry.read_to_string(&mut buf).context("read describe.json")?;
    serde_json::from_str(&buf).context("parse describe.json from .gtxpack")
}
```

### Integration tests

```rust
// crates/greentic-ext-cli/tests/sign_verify_cmd.rs

#[test] fn keygen_writes_valid_pkcs8_to_stdout() { /* capture stdout, parse as PKCS8, verify ed25519 curve */ }
#[test] fn keygen_refuses_overwrite() { /* create file, rerun keygen --out <same>, expect exit 1 */ }
#[test] fn sign_then_verify_roundtrip_via_cli() { /* keygen → sign test describe → verify succeeds */ }
#[test] fn sign_uses_env_var_when_no_key_flag() { /* env set, sign without --key, works */ }
#[test] fn sign_flag_overrides_env_var() { /* env set + --key passed, flag wins */ }
#[test] fn verify_rejects_tampered() { /* sign, mutate metadata, verify exit 1 */ }
#[test] fn verify_accepts_gtxpack_archive() { /* sign, zip, verify via .gtxpack path */ }
#[test] fn verify_accepts_directory() { /* sign, leave in dir, verify via dir path */ }
```

Full test bodies sketched at impl time — shown scaffolding only.

### Cargo deps added

```toml
# crates/greentic-ext-cli/Cargo.toml additions
[dependencies]
ed25519-dalek = { workspace = true, features = ["pkcs8", "std", "rand_core"] }
pkcs8 = "0.10"
rand = { workspace = true }
zip = { workspace = true }
```

## 6. Wave 2 — `greentic-deployer-extensions`

### 6.1 `build.sh` env-aware signing

Modify `reference-extensions/deploy-desktop/build.sh` (and create analogous in `deploy-single-vm`):

```bash
# ... existing cargo component build + wasm-tools validate ...

if [ -n "${GREENTIC_EXT_SIGNING_KEY_PEM:-}" ]; then
    echo "==> sign describe.json (GREENTIC_EXT_SIGNING_KEY_PEM set)"
    gtdx sign describe.json
else
    echo "==> unsigned build (GREENTIC_EXT_SIGNING_KEY_PEM not set — dev mode)"
    # Strip any pre-existing signature so CI and local builds produce identical artifacts.
    if jq -e '.signature' describe.json >/dev/null; then
        tmp=$(mktemp)
        jq 'del(.signature)' describe.json > "$tmp" && mv "$tmp" describe.json
    fi
fi

# ... continue with ZIP assembly ...
```

### 6.2 `deploy-desktop@0.2.0` — bump + sign

- Bump `Cargo.toml` version `0.1.0 → 0.2.0`.
- Bump `describe.json` `metadata.version` to `"0.2.0"`.
- Rebuild to produce signed artifact.
- No source/schema changes beyond version.

### 6.3 `deploy-single-vm@0.1.0` — new reference extension

Directory: `reference-extensions/deploy-single-vm/`. Mirror `deploy-desktop` layout:

```
deploy-single-vm/
├── Cargo.toml                     cdylib+rlib, cargo-component metadata
├── rust-toolchain.toml            1.94.0 + wasm32-wasip1
├── wit/world.wit                  deploy-extension world
├── src/lib.rs                     Component with 4 Guest impls
├── describe.json                  kind: DeployExtension, 1 target
├── schemas/
│   ├── single-vm.credentials.schema.json
│   └── single-vm.config.schema.json
└── build.sh                       cargo component build + sign + zip
```

#### `describe.json`

```jsonc
{
  "apiVersion": "greentic.ai/v1",
  "kind": "DeployExtension",
  "metadata": {
    "id": "greentic.deploy-single-vm",
    "name": "Single VM Deploy",
    "version": "0.1.0",
    "summary": "Deploy to a single Linux VM via systemd units and a container runtime",
    "author": { "name": "Greentic", "email": "team@greentic.ai" },
    "license": "MIT"
  },
  "engine": { "extRuntime": "^0.1.0" },
  "capabilities": {
    "offered": [
      { "id": "greentic:deploy/single-vm-systemd", "version": "0.1.0" }
    ],
    "required": []
  },
  "runtime": {
    "component": "extension.wasm",
    "memoryLimitMB": 32,
    "permissions": { "network": [], "secrets": [], "callExtensionKinds": [] }
  },
  "contributions": {
    "targets": [
      {
        "id": "single-vm-systemd",
        "displayName": "Single VM (systemd + container runtime)",
        "description": "SSH into a Linux VM, install systemd units, run container(s) under docker or podman",
        "supportsRollback": true,
        "execution": { "kind": "builtin", "backend": "single_vm", "handler": null }
      }
    ]
  }
}
```

#### Credentials schema

```jsonc
// schemas/single-vm.credentials.schema.json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "title": "SingleVmCredentials",
  "type": "object",
  "required": ["host", "user"],
  "properties": {
    "host":           { "type": "string", "description": "Hostname or IP of the target VM" },
    "port":           { "type": "integer", "default": 22, "minimum": 1, "maximum": 65535 },
    "user":           { "type": "string", "description": "SSH username" },
    "privateKeyPath": { "type": "string", "description": "Path to SSH private key file" },
    "useSshAgent":    { "type": "boolean", "default": true, "description": "Prefer ssh-agent over privateKeyPath when available" }
  },
  "additionalProperties": false
}
```

#### Config schema

```jsonc
// schemas/single-vm.config.schema.json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "title": "SingleVmConfig",
  "type": "object",
  "required": ["deploymentName", "image"],
  "properties": {
    "deploymentName":   { "type": "string", "minLength": 1, "description": "Systemd unit base name" },
    "image":            { "type": "string", "description": "Container image reference (OCI)" },
    "containerRuntime": { "type": "string", "enum": ["docker", "podman"], "default": "docker" },
    "ports":            { "type": "array", "items": { "type": "string" }, "description": "host:container port maps" },
    "env":              { "type": "array", "items": { "type": "string" }, "description": "KEY=VALUE env vars" },
    "installPath":      { "type": "string", "default": "/opt/greentic", "description": "Where to install systemd units on the VM" }
  },
  "additionalProperties": false
}
```

#### `src/lib.rs`

Same shape as `deploy-desktop` — 4 Guest impls. Differences:
- `get_identity()` returns `"greentic.deploy-single-vm"`, kind `Deploy`.
- `get_offered()` returns 1 capability.
- `list_targets()` returns 1 target.
- `credential_schema` / `config_schema` return the 2 new schemas via `include_str!`.
- `validate_credentials` returns empty vec for `single-vm-systemd`, error diagnostic otherwise.
- `deployment::Guest::{deploy, poll, rollback}` return `Internal("deploy-single-vm uses Mode A builtin execution; dispatcher should route via backend=single_vm, not WASM")`.

### 6.4 CI workflow — inject secret, install gtdx, validate

`.github/workflows/ci.yml`:

```yaml
jobs:
  check:
    runs-on: ubuntu-latest
    timeout-minutes: 25
    env:
      GREENTIC_EXT_SIGNING_KEY_PEM: ${{ secrets.EXT_SIGNING_KEY_PEM }}
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@1.94.0
        with: { components: "rustfmt, clippy", targets: "wasm32-wasip1" }
      - uses: Swatinem/rust-cache@v2
      - name: Install jq + zip + unzip
        run: sudo apt-get update && sudo apt-get install -y jq zip unzip
      - name: Install cargo-component + wasm-tools + gtdx
        run: |
          cargo install cargo-component --locked
          cargo install wasm-tools --locked
          cargo install --git https://github.com/greentic-biz/greentic-designer-extensions \
              --rev <new-wave1-rev> greentic-ext-cli
      - name: Require signed artifacts on main-branch push
        if: github.ref == 'refs/heads/main'
        run: echo "CI_REQUIRE_SIGNED=1" >> "$GITHUB_ENV"
      - run: bash ci/local_check.sh
```

### 6.5 `ci/validate-gtxpack.sh` — add signed guardrail

Append at end:

```bash
# Fail if signature required but missing, via CI_REQUIRE_SIGNED.
if [ -n "${CI_REQUIRE_SIGNED:-}" ]; then
    echo "==> CI_REQUIRE_SIGNED — verify inline signature"
    jq -e '.signature' "$STAGE/describe.json" >/dev/null \
        || { echo "FAIL: unsigned describe.json in CI_REQUIRE_SIGNED mode"; exit 1; }
    gtdx verify "$STAGE/describe.json"
fi
```

### 6.6 `ci/local_check.sh` — build both ref exts

```bash
echo "==> build deploy-desktop"
(cd reference-extensions/deploy-desktop && bash build.sh)
bash ci/validate-gtxpack.sh \
    reference-extensions/deploy-desktop/greentic.deploy-desktop-0.2.0.gtxpack

echo "==> build deploy-single-vm"
(cd reference-extensions/deploy-single-vm && bash build.sh)
bash ci/validate-gtxpack.sh \
    reference-extensions/deploy-single-vm/greentic.deploy-single-vm-0.1.0.gtxpack
```

### 6.7 README updates

Add to the "Shipped extensions" table:

| Extension | Targets | Execution mode | Status |
| --- | --- | --- | --- |
| `greentic.deploy-desktop@0.2.0` | `docker-compose-local`, `podman-local` | Mode A → `desktop` built-in backend | **Signed** |
| `greentic.deploy-single-vm@0.1.0` | `single-vm-systemd` | Mode A → `single_vm` built-in backend | **Signed** |

Add a "Signing" section documenting:
- Public key prefix (first 16 chars) + vault location.
- How to verify locally: `gtdx verify reference-extensions/deploy-desktop/*.gtxpack`.
- How to install signed artifacts.
- How to bypass in dev: `GREENTIC_EXT_ALLOW_UNSIGNED=1`.

## 7. Wave 3 — `greentic-deployer`

### 7.1 Rev bump

`Cargo.toml`:

```toml
greentic-ext-runtime  = { git = "https://github.com/greentic-biz/greentic-designer-extensions", rev = "<wave1-rev>", optional = true, package = "greentic-ext-runtime" }
greentic-ext-contract = { git = "https://github.com/greentic-biz/greentic-designer-extensions", rev = "<wave1-rev>", optional = true, package = "greentic-ext-contract" }
```

The exact rev is the merge commit SHA of Wave 1 into `greentic-designer-extensions` `main`.

### 7.2 `BuiltinBackendId::SingleVm` variant

`src/extension.rs` — mirror `Desktop` pattern from PR#121.

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BuiltinBackendId {
    Terraform, K8sRaw, Helm, Aws, Azure, Gcp,
    JujuK8s, JujuMachine, Operator, Serverless, Snap,
    Desktop,
    SingleVm, // NEW
}

impl BuiltinBackendId {
    pub fn as_str(self) -> &'static str {
        match self {
            // ... existing variants ...
            Self::Desktop  => "desktop",
            Self::SingleVm => "single_vm",
        }
    }

    pub fn handler_matches(self, handler: Option<&str>) -> bool {
        match self {
            Self::Desktop => matches!(handler, None | Some("docker-compose") | Some("podman")),
            Self::SingleVm => handler.is_none(),
            _ => handler.is_none(),
        }
    }
}

impl std::str::FromStr for BuiltinBackendId {
    type Err = UnknownBuiltinBackendStr;
    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        Ok(match s {
            // ... existing ...
            "desktop"   => Self::Desktop,
            "single_vm" => Self::SingleVm,
            other => return Err(UnknownBuiltinBackendStr(other.to_string())),
        })
    }
}
```

Also extend `BuiltinBackendHandlerId` mirror enum with `SingleVm` variant + `as_str` mapping `=> "single_vm"`.

### 7.3 Tests in `ext_roundtrip_tests`

Add:

```rust
#[test]
fn single_vm_variant_roundtrip() {
    use std::str::FromStr;
    assert_eq!(BuiltinBackendId::from_str("single_vm").unwrap(), BuiltinBackendId::SingleVm);
    assert_eq!(BuiltinBackendId::SingleVm.as_str(), "single_vm");
}

#[test]
fn single_vm_handler_matches_rejects_any_handler() {
    assert!(BuiltinBackendId::SingleVm.handler_matches(None));
    assert!(!BuiltinBackendId::SingleVm.handler_matches(Some("docker")));
}
```

### 7.4 Integration test env guards

`tests/ext_loader.rs` and `tests/ext_dispatch.rs`:

```rust
#[test]
fn ...test_body... {
    // Set at start of each test that loads the in-repo unsigned fixture:
    let _guard = env_set("GREENTIC_EXT_ALLOW_UNSIGNED", "1");
    // ... existing test body ...
}
```

The `env_set` helper mirrors the scoped guard pattern already used in deployer loader tests. If already imported from `tests/support/`, reuse; otherwise add.

### 7.5 Version bump + CHANGELOG

- `Cargo.toml` version `0.4.53 → 0.4.54`.
- Cargo.lock updated via `cargo update -p greentic-deployer`.
- CHANGELOG entry:

```
## 0.4.54 (2026-04-NN)

- feat(ext): add BuiltinBackendId::SingleVm variant
- chore: bump greentic-ext-runtime + greentic-ext-contract rev to pull in
  extension signature verification. Installed extensions without a valid
  signature are rejected at runtime load; set GREENTIC_EXT_ALLOW_UNSIGNED=1
  to bypass for dev.
- fix: tests/ext_{loader,dispatch}.rs set GREENTIC_EXT_ALLOW_UNSIGNED=1 to
  continue loading the in-repo unsigned fixture
```

## 8. Error handling

### `greentic-ext-contract::ContractError`

```rust
SignatureInvalid(String)   // existing
Canonicalize(String)       // NEW
```

### `greentic-ext-runtime::RuntimeError`

```rust
LoadFailed(String)         // existing
SignatureInvalid {         // NEW
    extension_id: String,
    reason: String,
}
```

### CLI errors

Surfaced via `anyhow::Context`. No domain enum.

### User-facing messages

| Situation | Message (stderr, exit code) |
| --- | --- |
| gtdx sign, no key | `env var $GREENTIC_EXT_SIGNING_KEY_PEM not set (use --key <path> or export)` (exit 1) |
| gtdx sign, malformed PEM | `parse PKCS8 PEM private key: <parse error>` (exit 1) |
| gtdx verify, no sig field | `signature invalid: missing signature field` (exit 1) |
| gtdx verify, tampered | `signature invalid: signature verification failed` (exit 1) |
| gtdx keygen, file exists | `write <path>: file exists` (exit 1 — refuse to overwrite) |
| runtime load, unsigned | `signature verification failed for extension 'X': missing signature field\nhint: reinstall a signed extension, or set GREENTIC_EXT_ALLOW_UNSIGNED=1 for dev` (reject) |
| runtime load, tampered | `signature verification failed for extension 'X': signature verification failed\nhint: …` (reject) |
| runtime load, env set | WARN log `GREENTIC_EXT_ALLOW_UNSIGNED is set — signature verification skipped` (load) |
| runtime load, env unset + valid sig | INFO log `extension signature verified`, key prefix (load) |

## 9. Testing strategy

### Unit / integration tests

| Crate | Test file | New cases |
| --- | --- | --- |
| `greentic-ext-contract` | `tests/signature_rt.rs` | `sign_describe_roundtrip`, `sign_describe_tamper_detected`, `sign_describe_reencoding_stable` |
| `greentic-ext-cli` | `tests/sign_verify_cmd.rs` | 8 CLI integration tests (keygen / sign / verify paths) |
| `greentic-ext-runtime` | `tests/signature_gate.rs` | 5 gate tests (unsigned reject, tampered reject, signed accept, env bypass, env bypass even when tampered) |
| `greentic-deployer-extensions` | CI `ci/local_check.sh` | Builds + validates both signed ref exts |
| `greentic-deployer` | `src/extension.rs` roundtrip tests | 2 new (SingleVm variant roundtrip + handler_matches) |
| `greentic-deployer` | `tests/ext_{loader,dispatch}.rs` | env guard updates (behavior preserved, env unlocked) |

### CI acceptance per repo

- `greentic-designer-extensions/ci/local_check.sh`: all existing + new Wave 1 tests pass.
- `greentic-deployer-extensions/ci/local_check.sh`: builds both ref exts, validates with `CI_REQUIRE_SIGNED=1` on main-branch push.
- `greentic-deployer/ci/local_check.sh`: all 250+ existing tests pass, plus 2 new roundtrip tests.

### Manual end-to-end

After all 3 waves land:

1. `gtdx keygen --out /tmp/k.pem` — key generated, public key printed.
2. `GREENTIC_EXT_SIGNING_KEY_PEM="$(cat /tmp/k.pem)" gtdx sign greentic-deployer-extensions/reference-extensions/deploy-desktop/describe.json` — signed in place.
3. `gtdx verify greentic-deployer-extensions/reference-extensions/deploy-desktop/greentic.deploy-desktop-0.2.0.gtxpack` — exit 0 with OK line.
4. Install signed `.gtxpack` to `~/.greentic/extensions/deploy/`; `greentic-deployer ext list` works (deployer v0.4.54+).
5. Mutate installed describe.json metadata.version manually; `greentic-deployer ext list` fails with `SignatureInvalid`.
6. `GREENTIC_EXT_ALLOW_UNSIGNED=1 greentic-deployer ext list` — bypasses, warns, loads.

## 10. Acceptance criteria

1. `cd greentic-designer-extensions && cargo test --workspace` — all tests pass.
2. `gtdx keygen` produces valid PKCS8 PEM to stdout; `--out /path` writes file 0600; refuses to overwrite.
3. `gtdx sign <describe.json>` mutates in place with valid `signature` field; succeeds via env var or `--key`.
4. `gtdx verify <describe.json|dir|.gtxpack>` exits 0 on valid, 1 on invalid/missing.
5. `greentic-ext-contract::verify_describe` roundtrips through serde re-encoding (canonicalization independent of field order).
6. `greentic-ext-runtime::ExtensionRuntime::register_loaded_from_dir` rejects unsigned; accepts signed; bypasses when `GREENTIC_EXT_ALLOW_UNSIGNED=1`.
7. `cd greentic-deployer-extensions && bash ci/local_check.sh` — builds both ref exts, validates.
8. CI on main-branch push with `CI_REQUIRE_SIGNED=1`: signature required; fails if missing.
9. CI on fork PR: builds unsigned, CI passes (no signing), documented behavior.
10. `cd greentic-deployer && bash ci/local_check.sh` with bumped rev + env guards: all tests pass.
11. `greentic-deployer` v0.4.54 released to crates.io; installed with `--features extensions` rejects unsigned extensions by default.
12. Manual E2E (§9) passes.

## 11. Development strategy

Wave 1 is one PR in `greentic-designer-extensions`. Wave 2 + Wave 3 are one PR each, independent.

Within Wave 1, commit order (for subagent-driven TDD):

1. `serde_jcs` dep + empty shim of `canonical_signing_payload`.
2. `canonical_signing_payload` + JCS strip-signature impl.
3. `sign_describe` impl.
4. `verify_describe` impl.
5. `ContractError::Canonicalize` variant.
6. `tests/signature_rt.rs` — 3 new tests.
7. Registry `verify_signature` body replaced with `verify_describe` call.
8. `RuntimeError::SignatureInvalid` variant.
9. `verify_signature_or_bypass` helper in `ExtensionRuntime`.
10. Hook into `register_loaded_from_dir`.
11. `tests/signature_gate.rs` — 5 tests.
12. `gtdx keygen` subcommand.
13. `gtdx sign` subcommand.
14. `gtdx verify` subcommand.
15. `tests/sign_verify_cmd.rs` — 8 CLI tests.
16. Version bump + CHANGELOG + README.

Wave 2 commit order:

1. Extend `build.sh` with env-aware signing.
2. Bump `deploy-desktop` to 0.2.0 + rebuild.
3. Create `reference-extensions/deploy-single-vm/` skeleton (Cargo.toml, wit/world.wit, rust-toolchain.toml).
4. Add 2 schemas.
5. `src/lib.rs` with 4 Guest impls.
6. `describe.json` + optional assets.
7. `build.sh` for single-vm.
8. Update `ci/local_check.sh` to build + validate both.
9. Update `ci/validate-gtxpack.sh` with `CI_REQUIRE_SIGNED` guardrail.
10. Workflow updates — secret injection, gtdx install.
11. README updates.

Wave 3 commit order:

1. Bump rev pins in `Cargo.toml` + `cargo update`.
2. Add `SingleVm` variant to `BuiltinBackendId` + mirror enum + `FromStr` / `as_str` / `handler_matches`.
3. Add 2 roundtrip tests.
4. Update `tests/ext_{loader,dispatch}.rs` with env guards.
5. Bump version 0.4.53 → 0.4.54 + CHANGELOG.

## 12. Open questions

- **Q1.** gtdx install in CI adds ~30s per PR (cached after first run via rust-cache). If that becomes a hot path, consider shipping a prebuilt gtdx binary via GH Releases of `greentic-designer-extensions`. Deferred until pain observed.
- **Q2.** `sample_describe` helper in contract tests — reuse existing test fixture if one exists in `tests/common.rs`; otherwise create minimal fixture inline. Decided at impl time.
- **Q3.** `GREENTIC_EXT_ALLOW_UNSIGNED` env value: any string set = bypass. Alternative: `=1` / `=true` only. Rekomendasi "any set" to match conventional Rust env patterns (`RUST_LOG`, `RUST_BACKTRACE`).

## 13. References

- Parent migration memory: `/home/bimbim/.claude/projects/-home-bimbim-works-greentic/memory/deploy-extension-migration.md`
- Deployer PR #121 — `src/ext/` host integration, introduces `GREENTIC_EXT_ALLOW_UNSIGNED` env var concept.
- Deployer PR #122 — parent migration spec + plan.
- `greentic-deployer-extensions` PR #1 — first ref ext shipped unsigned.
- `greentic-pack` `crates/packc/src/signing/signer.rs` — precedent for PKCS8 PEM + ed25519 signing.
- RFC 8785 — JSON Canonicalization Scheme (JCS).
