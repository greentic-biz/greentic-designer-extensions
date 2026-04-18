# Extension Signing Pipeline — Wave 1 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship end-to-end sign + verify infrastructure for Greentic extensions in `greentic-designer-extensions`: JCS canonicalization fix, `sign_describe`/`verify_describe` helpers, runtime verify hook with `GREENTIC_EXT_ALLOW_UNSIGNED` escape hatch, and `gtdx sign/verify/keygen` CLI subcommands.

**Architecture:** Additive-only in three existing crates. `greentic-ext-contract` gets `serde_jcs` dep and two new helper functions that wrap the existing `verify_ed25519` primitive. `greentic-ext-registry::lifecycle::verify_signature` switches from the broken `serde_json::to_vec` path to the new `verify_describe` helper — single-line body swap. `greentic-ext-runtime::ExtensionRuntime::register_loaded_from_dir` gains a verify-or-bypass gate before the existing `LoadedExtension::load_from_dir` call. `greentic-ext-cli` gains three new subcommands following the established `commands/<name>::{Args, run}` pattern.

**Tech Stack:** Rust 1.94, edition 2024. `ed25519-dalek` 2.1 (existing), `serde_jcs` 0.1 (NEW), `pkcs8` 0.10 (NEW, for CLI key IO), `tempfile` + `wat` + `zip` (existing test infrastructure).

**Spec:** `docs/superpowers/specs/2026-04-18-extension-signing-pipeline-design.md` — Wave 1 scope only. Wave 2 (deployer-extensions) and Wave 3 (deployer) get their own plans after Wave 1 merges and produces a pinnable rev SHA.

---

## File Structure

### New files

| Path | Responsibility |
| --- | --- |
| `crates/greentic-ext-cli/src/commands/sign.rs` | Sign command impl: read describe, call `sign_describe`, write in-place |
| `crates/greentic-ext-cli/src/commands/verify.rs` | Verify command impl: accept file/dir/.gtxpack, call `verify_describe` |
| `crates/greentic-ext-cli/src/commands/keygen.rs` | Keygen command impl: PKCS8 PEM to stdout or file (mode 0600) |
| `crates/greentic-ext-cli/tests/sign_verify_cmd.rs` | 8 integration tests exercising the three new CLI commands |
| `crates/greentic-ext-runtime/tests/signature_gate.rs` | 5 integration tests for the verify-or-bypass gate |
| `crates/greentic-ext-runtime/tests/support/mod.rs` | Shared test helpers: scoped env guard, signed fixture builder |

### Modified files

| Path | Change |
| --- | --- |
| `Cargo.toml` (root) | Add `serde_jcs` + `pkcs8` + `zip` to `[workspace.dependencies]` |
| `crates/greentic-ext-contract/Cargo.toml` | Add `serde_jcs = { workspace = true }` |
| `crates/greentic-ext-contract/src/error.rs` | Add `ContractError::Canonicalize(String)` variant |
| `crates/greentic-ext-contract/src/signature.rs` | Add `canonical_signing_payload`, `sign_describe`, `verify_describe` |
| `crates/greentic-ext-contract/src/lib.rs` | Re-export the three new fns |
| `crates/greentic-ext-contract/tests/signature_rt.rs` | 3 new describe.json sign/verify tests |
| `crates/greentic-ext-registry/src/lifecycle.rs` | Replace `verify_signature` body with `verify_describe` call |
| `crates/greentic-ext-runtime/src/error.rs` | Add `RuntimeError::SignatureInvalid { extension_id, reason }` |
| `crates/greentic-ext-runtime/src/runtime.rs` | Add verify gate in `register_loaded_from_dir` |
| `crates/greentic-ext-runtime/Cargo.toml` | Add `serde_json` if not already workspace (it is; check) |
| `crates/greentic-ext-runtime/tests/runtime_load.rs` | Set `GREENTIC_EXT_ALLOW_UNSIGNED=1` via scoped guard (fixture is unsigned) |
| `crates/greentic-ext-cli/src/main.rs` | Add `Sign`/`Verify`/`Keygen` variants to `Command` enum + dispatch arms |
| `crates/greentic-ext-cli/src/commands/mod.rs` | `pub mod sign; pub mod verify; pub mod keygen;` |
| `crates/greentic-ext-cli/Cargo.toml` | Add `ed25519-dalek` feature `pkcs8`, add `pkcs8`, `zip`, `rand` deps |

### Untouched files (for clarity — plan does not modify these)

- `crates/greentic-ext-contract/src/describe.rs` — `DescribeJson`, `Signature`, etc. stay as-is
- All WIT files under `wit/`
- All reference-extension crates under `reference-extensions/`
- `.github/workflows/ci.yml` — existing `ci/local_check.sh` covers new tests
- `ci/local_check.sh` — existing `cargo test --workspace --all-features` picks up new tests

---

## Prerequisite: branch setup

- [ ] **Step 0: Create and check out implementation branch**

```bash
cd /home/bimbim/works/greentic/greentic-designer-extensions
git checkout main
git pull --ff-only origin main
git checkout -b feat/extension-signing-pipeline
```

Expected: clean working tree on new branch off latest main.

---

## Task 1: Add `serde_jcs` + `pkcs8` + `zip` to workspace deps

**Files:**
- Modify: `Cargo.toml` (root)

- [ ] **Step 1: Inspect current `[workspace.dependencies]`**

```bash
grep -n "serde_jcs\|pkcs8\|^zip" Cargo.toml
```

Expected: three greps find nothing (or only the `oci-client`'s indirect `pkcs8`; new direct `pkcs8` dep missing).

- [ ] **Step 2: Add deps**

Insert into `[workspace.dependencies]` block of root `Cargo.toml`, alphabetically sorted:

```toml
# (existing dep lines stay)
pkcs8 = "0.10"
serde_jcs = "0.1"
zip = { version = "2", default-features = false, features = ["deflate"] }
```

(`zip` may already exist — check before adding.)

- [ ] **Step 3: Verify workspace resolves**

```bash
cargo metadata --format-version 1 --no-deps 2>&1 | tail -3
```

Expected: no error, JSON output ends with closing brace.

- [ ] **Step 4: Commit**

```bash
git add Cargo.toml
git commit -m "chore: add serde_jcs + pkcs8 + zip to workspace dependencies"
```

---

## Task 2: Add `ContractError::Canonicalize` variant

**Files:**
- Modify: `crates/greentic-ext-contract/src/error.rs`

- [ ] **Step 1: Read current `error.rs`**

```bash
cat crates/greentic-ext-contract/src/error.rs
```

Expected content (reference):

```rust
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ContractError {
    #[error("describe.json schema validation failed: {0}")]
    SchemaInvalid(String),

    #[error("capability id is malformed: {0}")]
    MalformedCapabilityId(String),

    #[error("version is not semver: {0}")]
    MalformedVersion(String),

    #[error("signature verification failed: {0}")]
    SignatureInvalid(String),

    #[error("unsupported apiVersion: {0}")]
    UnsupportedApiVersion(String),

    #[error("io: {0}")]
    Io(#[from] std::io::Error),

    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
}
```

- [ ] **Step 2: Add the `Canonicalize` variant**

Append inside the enum (after `UnsupportedApiVersion`, before `Io`):

```rust
    #[error("canonicalization failed: {0}")]
    Canonicalize(String),
```

- [ ] **Step 3: Verify compilation**

```bash
cargo check -p greentic-ext-contract 2>&1 | tail -3
```

Expected: `Finished …` with no errors.

- [ ] **Step 4: Commit**

```bash
git add crates/greentic-ext-contract/src/error.rs
git commit -m "feat(contract): add ContractError::Canonicalize variant"
```

---

## Task 3: TDD `canonical_signing_payload` helper

**Files:**
- Modify: `crates/greentic-ext-contract/src/signature.rs`
- Modify: `crates/greentic-ext-contract/Cargo.toml`
- Test: `crates/greentic-ext-contract/tests/signature_rt.rs`

- [ ] **Step 1: Add `serde_jcs` dep to contract crate**

Edit `crates/greentic-ext-contract/Cargo.toml` — append under `[dependencies]`:

```toml
serde_jcs = { workspace = true }
```

- [ ] **Step 2: Write the failing test**

Append to `crates/greentic-ext-contract/tests/signature_rt.rs`:

```rust
use greentic_ext_contract::{canonical_signing_payload, DescribeJson};

fn sample_describe_with_sig(sig_value: Option<&str>) -> DescribeJson {
    let json = serde_json::json!({
        "apiVersion": "greentic.ai/v1",
        "kind": "DesignExtension",
        "metadata": {
            "id": "greentic.canonicalize-test",
            "name": "Canonicalize Test",
            "version": "0.1.0",
            "summary": "test fixture",
            "author": { "name": "test" },
            "license": "MIT"
        },
        "engine": { "greenticDesigner": "*", "extRuntime": "*" },
        "capabilities": { "offered": [], "required": [] },
        "runtime": { "component": "x.wasm", "memoryLimitMB": 64, "permissions": {} },
        "contributions": {},
        "signature": sig_value.map(|v| serde_json::json!({
            "algorithm": "ed25519",
            "publicKey": "AAAA",
            "value": v
        }))
    });
    serde_json::from_value(json).expect("sample describe parses")
}

#[test]
fn canonical_payload_omits_signature_field() {
    let with_sig = sample_describe_with_sig(Some("SIG_A"));
    let bytes_with = canonical_signing_payload(&with_sig).expect("canonicalize with sig");
    let without_sig = sample_describe_with_sig(None);
    let bytes_without = canonical_signing_payload(&without_sig).expect("canonicalize without sig");
    assert_eq!(bytes_with, bytes_without, "canonical bytes must ignore .signature");
}

#[test]
fn canonical_payload_is_deterministic_across_serde_round_trip() {
    let d1 = sample_describe_with_sig(None);
    let json = serde_json::to_string(&d1).unwrap();
    let d2: DescribeJson = serde_json::from_str(&json).unwrap();
    let b1 = canonical_signing_payload(&d1).unwrap();
    let b2 = canonical_signing_payload(&d2).unwrap();
    assert_eq!(b1, b2, "canonical form must survive serde round trip");
}
```

- [ ] **Step 3: Run the tests to verify failure**

```bash
cargo test -p greentic-ext-contract --test signature_rt 2>&1 | tail -15
```

Expected: compile errors citing `cannot find function canonical_signing_payload`.

- [ ] **Step 4: Implement `canonical_signing_payload`**

Edit `crates/greentic-ext-contract/src/signature.rs`. Append at the bottom (before any existing `fn strip_prefix`):

```rust
use crate::describe::DescribeJson;

/// Canonicalize describe.json for signing — strip the `.signature` field
/// and emit RFC 8785 JCS bytes. Output is deterministic across languages
/// and serde versions.
pub fn canonical_signing_payload(describe: &DescribeJson) -> Result<Vec<u8>, ContractError> {
    let mut clone = describe.clone();
    clone.signature = None;
    serde_jcs::to_vec(&clone).map_err(|e| ContractError::Canonicalize(e.to_string()))
}
```

- [ ] **Step 5: Re-export from `lib.rs`**

Edit `crates/greentic-ext-contract/src/lib.rs`. Extend the `pub use` for the `signature` module:

```rust
pub use self::signature::{artifact_sha256, canonical_signing_payload, sign_ed25519, verify_ed25519};
```

- [ ] **Step 6: Run the tests to verify pass**

```bash
cargo test -p greentic-ext-contract --test signature_rt 2>&1 | tail -15
```

Expected: all tests pass (existing 3 + 2 new = 5 passing).

- [ ] **Step 7: Commit**

```bash
git add crates/greentic-ext-contract/
git commit -m "feat(contract): add canonical_signing_payload (RFC 8785 JCS, strips signature)"
```

---

## Task 4: TDD `sign_describe` helper

**Files:**
- Modify: `crates/greentic-ext-contract/src/signature.rs`
- Test: `crates/greentic-ext-contract/tests/signature_rt.rs`

- [ ] **Step 1: Write the failing test**

Append to `crates/greentic-ext-contract/tests/signature_rt.rs`:

```rust
use ed25519_dalek::SigningKey;
use greentic_ext_contract::sign_describe;
use rand::rngs::OsRng;

#[test]
fn sign_describe_populates_signature_field() {
    let sk = SigningKey::generate(&mut OsRng);
    let mut d = sample_describe_with_sig(None);
    assert!(d.signature.is_none());
    sign_describe(&mut d, &sk).expect("sign");
    let sig = d.signature.as_ref().expect("signature populated");
    assert_eq!(sig.algorithm, "ed25519");
    assert_eq!(sig.public_key.len(), 44, "base64 of 32 bytes is 44 chars");
    assert_eq!(sig.value.len(), 88, "base64 of 64 bytes is 88 chars");
}

#[test]
fn sign_describe_strips_preexisting_signature_before_signing() {
    // If caller passes a describe that already has a stale signature,
    // sign_describe should canonicalize as-if signature was None so the
    // new sig is not computed over a signed payload.
    let sk = SigningKey::generate(&mut OsRng);
    let mut d_preexisting = sample_describe_with_sig(Some("STALE"));
    let mut d_fresh = sample_describe_with_sig(None);
    sign_describe(&mut d_preexisting, &sk).expect("sign");
    sign_describe(&mut d_fresh, &sk).expect("sign");
    assert_eq!(
        d_preexisting.signature.as_ref().unwrap().value,
        d_fresh.signature.as_ref().unwrap().value,
        "signing a stale-signed describe must produce same signature as signing clean",
    );
}
```

- [ ] **Step 2: Run tests, confirm compile failure**

```bash
cargo test -p greentic-ext-contract --test signature_rt 2>&1 | tail -10
```

Expected: `cannot find function sign_describe`.

- [ ] **Step 3: Implement `sign_describe`**

Append to `crates/greentic-ext-contract/src/signature.rs`:

```rust
/// Sign describe.json in-place. Strips any existing `.signature` field,
/// canonicalizes via JCS, signs the canonical bytes, and injects a fresh
/// `.signature` object. Safe to call on already-signed describe (produces
/// identical bytes regardless of prior sig).
pub fn sign_describe(
    describe: &mut DescribeJson,
    signing_key: &ed25519_dalek::SigningKey,
) -> Result<(), ContractError> {
    use ed25519_dalek::Signer;
    // Defensive: strip before canonicalize so the sig is computed on clean payload.
    describe.signature = None;
    let payload = canonical_signing_payload(describe)?;
    let sig = signing_key.sign(&payload);
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

- [ ] **Step 4: Re-export from `lib.rs`**

Extend the re-export line:

```rust
pub use self::signature::{artifact_sha256, canonical_signing_payload, sign_describe, sign_ed25519, verify_ed25519};
```

- [ ] **Step 5: Run tests, confirm all pass**

```bash
cargo test -p greentic-ext-contract --test signature_rt 2>&1 | tail -10
```

Expected: 7 tests pass (3 existing + 2 Task 3 + 2 new).

- [ ] **Step 6: Commit**

```bash
git add crates/greentic-ext-contract/
git commit -m "feat(contract): add sign_describe helper (mutates describe in-place)"
```

---

## Task 5: TDD `verify_describe` helper

**Files:**
- Modify: `crates/greentic-ext-contract/src/signature.rs`
- Test: `crates/greentic-ext-contract/tests/signature_rt.rs`

- [ ] **Step 1: Write the failing tests**

Append to `crates/greentic-ext-contract/tests/signature_rt.rs`:

```rust
use greentic_ext_contract::verify_describe;

#[test]
fn sign_describe_then_verify_describe_roundtrip() {
    let sk = SigningKey::generate(&mut OsRng);
    let mut d = sample_describe_with_sig(None);
    sign_describe(&mut d, &sk).expect("sign");
    verify_describe(&d).expect("verify");
}

#[test]
fn verify_describe_missing_signature_fails() {
    let d = sample_describe_with_sig(None);
    let err = verify_describe(&d).unwrap_err();
    assert!(matches!(err, greentic_ext_contract::ContractError::SignatureInvalid(_)));
    assert!(format!("{err}").contains("missing signature"));
}

#[test]
fn verify_describe_rejects_tampered_metadata() {
    let sk = SigningKey::generate(&mut OsRng);
    let mut d = sample_describe_with_sig(None);
    sign_describe(&mut d, &sk).expect("sign");
    d.metadata.version = "99.99.99".into();
    let err = verify_describe(&d).unwrap_err();
    assert!(matches!(err, greentic_ext_contract::ContractError::SignatureInvalid(_)));
}

#[test]
fn verify_describe_rejects_non_ed25519_algorithm() {
    let sk = SigningKey::generate(&mut OsRng);
    let mut d = sample_describe_with_sig(None);
    sign_describe(&mut d, &sk).expect("sign");
    d.signature.as_mut().unwrap().algorithm = "sha256-hmac".into();
    let err = verify_describe(&d).unwrap_err();
    assert!(format!("{err}").contains("unsupported algorithm"));
}

#[test]
fn verify_describe_survives_serde_round_trip() {
    // Field-order-independence test: sign, re-serialize through serde_json,
    // re-parse, verify still passes. Proves JCS canonicalization is stable.
    let sk = SigningKey::generate(&mut OsRng);
    let mut d1 = sample_describe_with_sig(None);
    sign_describe(&mut d1, &sk).expect("sign");
    let json = serde_json::to_string(&d1).unwrap();
    let d2: DescribeJson = serde_json::from_str(&json).unwrap();
    verify_describe(&d2).expect("verify after serde roundtrip");
}
```

- [ ] **Step 2: Run tests, confirm compile failure**

```bash
cargo test -p greentic-ext-contract --test signature_rt 2>&1 | tail -10
```

Expected: `cannot find function verify_describe`.

- [ ] **Step 3: Implement `verify_describe`**

Append to `crates/greentic-ext-contract/src/signature.rs`:

```rust
/// Verify the inline `.signature` field of a describe.json. Returns
/// `Ok(())` iff signature is present, algorithm is `ed25519`, and the
/// signature matches the canonical payload (describe with `.signature`
/// stripped, serialized via JCS).
pub fn verify_describe(describe: &DescribeJson) -> Result<(), ContractError> {
    let sig = describe
        .signature
        .as_ref()
        .ok_or_else(|| ContractError::SignatureInvalid("missing signature field".into()))?;
    if sig.algorithm != "ed25519" {
        return Err(ContractError::SignatureInvalid(format!(
            "unsupported algorithm: {}",
            sig.algorithm
        )));
    }
    let payload = canonical_signing_payload(describe)?;
    verify_ed25519(&sig.public_key, &sig.value, &payload)
}
```

- [ ] **Step 4: Re-export from `lib.rs`**

Final re-export line:

```rust
pub use self::signature::{
    artifact_sha256, canonical_signing_payload, sign_describe, sign_ed25519,
    verify_describe, verify_ed25519,
};
```

- [ ] **Step 5: Run tests, confirm all pass**

```bash
cargo test -p greentic-ext-contract 2>&1 | tail -10
```

Expected: 12 tests pass (3 existing + 2 Task 3 + 2 Task 4 + 5 new).

- [ ] **Step 6: Commit**

```bash
git add crates/greentic-ext-contract/
git commit -m "feat(contract): add verify_describe helper (verifies inline describe signature)"
```

---

## Task 6: Fix registry `verify_signature` body

**Files:**
- Modify: `crates/greentic-ext-registry/src/lifecycle.rs`

- [ ] **Step 1: Locate the buggy function**

```bash
grep -n "fn verify_signature" crates/greentic-ext-registry/src/lifecycle.rs
```

Expected: one match near line 101.

- [ ] **Step 2: Replace body**

Edit `crates/greentic-ext-registry/src/lifecycle.rs`. Find:

```rust
fn verify_signature(
    artifact: &ExtensionArtifact,
    policy: TrustPolicy,
) -> Result<(), RegistryError> {
    match policy {
        TrustPolicy::Loose => Ok(()),
        TrustPolicy::Strict | TrustPolicy::Normal => {
            let Some(sig) = &artifact.describe.signature else {
                return Err(RegistryError::SignatureInvalid("missing signature".into()));
            };
            let payload = serde_json::to_vec(&artifact.describe)?;
            greentic_ext_contract::verify_ed25519(&sig.public_key, &sig.value, &payload)
                .map_err(|e| RegistryError::SignatureInvalid(e.to_string()))
        }
    }
}
```

Replace with:

```rust
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

- [ ] **Step 3: Verify registry tests still pass**

```bash
cargo test -p greentic-ext-registry 2>&1 | tail -15
```

Expected: all existing tests pass. The fixture-builder in `ExtensionFixtureBuilder` produces unsigned describes, and the `lifecycle.rs` tests install with `TrustPolicy::Loose` — so no test currently exercises the `verify_describe` call. Tests should still be green. If any test breaks, investigate — that's unexpected and worth flagging.

- [ ] **Step 4: Commit**

```bash
git add crates/greentic-ext-registry/src/lifecycle.rs
git commit -m "fix(registry): use verify_describe for JCS-correct signature check

Previous implementation serialized describe.json with .signature included,
creating a chicken-and-egg problem. verify_describe strips the field and
uses RFC 8785 JCS canonicalization."
```

---

## Task 7: Add `RuntimeError::SignatureInvalid` variant

**Files:**
- Modify: `crates/greentic-ext-runtime/src/error.rs`

- [ ] **Step 1: Read current error.rs**

```bash
cat crates/greentic-ext-runtime/src/error.rs
```

- [ ] **Step 2: Add the new variant**

Insert after the `NotFound` variant, before `Contract`:

```rust
    #[error(
        "signature verification failed for extension '{extension_id}': {reason}\n\
         hint: reinstall a signed extension, or set GREENTIC_EXT_ALLOW_UNSIGNED=1 for dev"
    )]
    SignatureInvalid { extension_id: String, reason: String },
```

- [ ] **Step 3: Verify compilation**

```bash
cargo check -p greentic-ext-runtime 2>&1 | tail -3
```

Expected: clean compile.

- [ ] **Step 4: Commit**

```bash
git add crates/greentic-ext-runtime/src/error.rs
git commit -m "feat(runtime): add RuntimeError::SignatureInvalid variant"
```

---

## Task 8: Create test support module with scoped env guard and signed fixture

**Files:**
- Create: `crates/greentic-ext-runtime/tests/support/mod.rs`

Rust 2024 edition makes `std::env::set_var` `unsafe`. We need a scoped RAII guard. The deployer repo established this pattern in PR#121; we mirror it here.

- [ ] **Step 1: Create directory and file**

```bash
mkdir -p crates/greentic-ext-runtime/tests/support
touch crates/greentic-ext-runtime/tests/support/mod.rs
```

- [ ] **Step 2: Write the helper module**

Write to `crates/greentic-ext-runtime/tests/support/mod.rs`:

```rust
//! Shared test helpers for runtime integration tests.
//!
//! Tests must not run in parallel when mutating process environment.
//! The `env_set` guard serializes via a global Mutex.

use std::sync::{Mutex, MutexGuard, OnceLock};

fn env_mutex() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

pub struct EnvGuard {
    key: String,
    prev: Option<String>,
    _lock: MutexGuard<'static, ()>,
}

impl EnvGuard {
    pub fn set(key: &str, value: &str) -> Self {
        let lock = env_mutex()
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let prev = std::env::var(key).ok();
        // SAFETY: serialized via global mutex; we hold the lock for guard lifetime.
        #[allow(unsafe_code)]
        unsafe {
            std::env::set_var(key, value);
        }
        EnvGuard {
            key: key.to_string(),
            prev,
            _lock: lock,
        }
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        #[allow(unsafe_code)]
        unsafe {
            match &self.prev {
                Some(v) => std::env::set_var(&self.key, v),
                None => std::env::remove_var(&self.key),
            }
        }
    }
}

/// Build a signed extension fixture using the `ExtensionFixtureBuilder`
/// from `greentic-ext-testing`, then sign its describe.json with a fresh
/// ed25519 key. Returns the fixture (TempDir-backed) and the signing key
/// used (caller may ignore).
pub fn signed_fixture(
    kind: greentic_ext_contract::ExtensionKind,
    id: &str,
    version: &str,
) -> (greentic_ext_testing::ExtensionFixture, ed25519_dalek::SigningKey) {
    use ed25519_dalek::SigningKey;
    use rand::rngs::OsRng;

    let minimal_wasm =
        wat::parse_str(r"(component)").expect("wat component must compile");
    let fixture = greentic_ext_testing::ExtensionFixtureBuilder::new(kind, id, version)
        .offer("greentic:test/ping", "1.0.0")
        .with_wasm(minimal_wasm)
        .build()
        .expect("fixture build");

    // Read, sign, write back.
    let describe_path = fixture.root().join("describe.json");
    let raw = std::fs::read_to_string(&describe_path).unwrap();
    let mut describe: greentic_ext_contract::DescribeJson =
        serde_json::from_str(&raw).unwrap();
    let sk = SigningKey::generate(&mut OsRng);
    greentic_ext_contract::sign_describe(&mut describe, &sk).expect("sign");
    let out = serde_json::to_string_pretty(&describe).unwrap();
    std::fs::write(&describe_path, out).unwrap();

    (fixture, sk)
}

/// Mutate an installed fixture's describe.json to invalidate its signature.
pub fn tamper_fixture(fixture: &greentic_ext_testing::ExtensionFixture) {
    let path = fixture.root().join("describe.json");
    let raw = std::fs::read_to_string(&path).unwrap();
    let mut describe: greentic_ext_contract::DescribeJson =
        serde_json::from_str(&raw).unwrap();
    describe.metadata.version = "99.99.99".into();
    std::fs::write(&path, serde_json::to_string_pretty(&describe).unwrap()).unwrap();
}

/// Build an **unsigned** fixture (no .signature field). Mirrors existing
/// `ExtensionFixtureBuilder` default output.
pub fn unsigned_fixture(
    kind: greentic_ext_contract::ExtensionKind,
    id: &str,
    version: &str,
) -> greentic_ext_testing::ExtensionFixture {
    let minimal_wasm =
        wat::parse_str(r"(component)").expect("wat component must compile");
    greentic_ext_testing::ExtensionFixtureBuilder::new(kind, id, version)
        .offer("greentic:test/ping", "1.0.0")
        .with_wasm(minimal_wasm)
        .build()
        .expect("fixture build")
}
```

- [ ] **Step 3: Add `#![forbid(unsafe_code)]` exception**

This module uses `unsafe { set_var }`. If `crates/greentic-ext-runtime/src/lib.rs` has `#![forbid(unsafe_code)]`, it applies to lib — not `tests/`. Confirm via:

```bash
grep forbid crates/greentic-ext-runtime/src/lib.rs
```

If lib has `forbid(unsafe_code)`, no action needed (tests are a different crate). If not, proceed — the `#[allow(unsafe_code)]` inside the fn body suffices.

- [ ] **Step 4: Verify `rand`, `wat`, `ed25519-dalek` are dev-dep or workspace-accessible**

Existing `crates/greentic-ext-runtime/Cargo.toml` `[dev-dependencies]` should already include `greentic-ext-testing` and `wat`. Add if missing:

```toml
[dev-dependencies]
ed25519-dalek = { workspace = true }
rand = { workspace = true }
```

Check current state:

```bash
grep -A 10 "^\[dev-dependencies\]" crates/greentic-ext-runtime/Cargo.toml
```

- [ ] **Step 5: Commit**

```bash
git add crates/greentic-ext-runtime/tests/support/ crates/greentic-ext-runtime/Cargo.toml
git commit -m "test(runtime): add scoped env guard + signed/unsigned fixture helpers"
```

---

## Task 9: TDD runtime verify gate in `register_loaded_from_dir`

**Files:**
- Modify: `crates/greentic-ext-runtime/src/runtime.rs`
- Create: `crates/greentic-ext-runtime/tests/signature_gate.rs`

- [ ] **Step 1: Write the failing integration tests**

Create `crates/greentic-ext-runtime/tests/signature_gate.rs`:

```rust
#[path = "support/mod.rs"]
mod support;

use std::path::PathBuf;

use greentic_ext_contract::ExtensionKind;
use greentic_ext_runtime::{DiscoveryPaths, ExtensionRuntime, RuntimeConfig, RuntimeError};

use support::{signed_fixture, tamper_fixture, unsigned_fixture, EnvGuard};

fn new_runtime() -> ExtensionRuntime {
    let config = RuntimeConfig::from_paths(DiscoveryPaths::new(PathBuf::from("/dev/null")));
    ExtensionRuntime::new(config).unwrap()
}

#[test]
fn rejects_unsigned_by_default() {
    // Clear env var explicitly — another test may have set it.
    let _guard = EnvGuard::set("GREENTIC_EXT_ALLOW_UNSIGNED", "");
    // Immediately remove to simulate unset:
    drop(_guard);

    let fx = unsigned_fixture(ExtensionKind::Design, "greentic.unsigned", "0.1.0");
    let mut rt = new_runtime();
    let err = rt.register_loaded_from_dir(fx.root()).unwrap_err();
    match err {
        RuntimeError::SignatureInvalid { extension_id, reason } => {
            assert_eq!(extension_id, "greentic.unsigned");
            assert!(reason.contains("missing signature"), "got: {reason}");
        }
        other => panic!("expected SignatureInvalid, got {other:?}"),
    }
}

#[test]
fn rejects_tampered_signature() {
    let _guard = EnvGuard::set("GREENTIC_EXT_ALLOW_UNSIGNED", "");
    drop(_guard);

    let (fx, _sk) = signed_fixture(ExtensionKind::Design, "greentic.tampered", "0.1.0");
    tamper_fixture(&fx);
    let mut rt = new_runtime();
    let err = rt.register_loaded_from_dir(fx.root()).unwrap_err();
    assert!(matches!(err, RuntimeError::SignatureInvalid { .. }));
}

#[test]
fn accepts_signed_by_default() {
    let _guard = EnvGuard::set("GREENTIC_EXT_ALLOW_UNSIGNED", "");
    drop(_guard);

    let (fx, _sk) = signed_fixture(ExtensionKind::Design, "greentic.signed", "0.1.0");
    let mut rt = new_runtime();
    rt.register_loaded_from_dir(fx.root()).expect("load signed");
}

#[test]
fn allow_unsigned_env_bypasses() {
    let _guard = EnvGuard::set("GREENTIC_EXT_ALLOW_UNSIGNED", "1");
    let fx = unsigned_fixture(ExtensionKind::Design, "greentic.bypass", "0.1.0");
    let mut rt = new_runtime();
    rt.register_loaded_from_dir(fx.root()).expect("load unsigned with env");
}

#[test]
fn allow_unsigned_env_bypasses_even_if_tampered() {
    let _guard = EnvGuard::set("GREENTIC_EXT_ALLOW_UNSIGNED", "1");
    let (fx, _sk) = signed_fixture(ExtensionKind::Design, "greentic.bypass-tampered", "0.1.0");
    tamper_fixture(&fx);
    let mut rt = new_runtime();
    // Skip-entirely semantics per design §4: env set = don't even verify.
    rt.register_loaded_from_dir(fx.root()).expect("load tampered with env");
}
```

- [ ] **Step 2: Run tests, confirm failure**

```bash
cargo test -p greentic-ext-runtime --test signature_gate 2>&1 | tail -15
```

Expected: all 5 tests fail — verify gate doesn't exist yet. Current behavior loads unsigned successfully.

- [ ] **Step 3: Implement the verify gate**

Edit `crates/greentic-ext-runtime/src/runtime.rs`. Locate `register_loaded_from_dir`:

```rust
pub fn register_loaded_from_dir(&mut self, dir: &std::path::Path) -> Result<(), RuntimeError> {
    let loaded = LoadedExtension::load_from_dir(&self.engine, dir)?;
    // ... existing body ...
}
```

Insert a call to the new verify helper at the top of the function body (before `let loaded = ...`):

```rust
pub fn register_loaded_from_dir(&mut self, dir: &std::path::Path) -> Result<(), RuntimeError> {
    self.verify_dir_signature(dir)?;
    let loaded = LoadedExtension::load_from_dir(&self.engine, dir)?;
    // ... existing body unchanged ...
}
```

Add the helper method (inside the same `impl ExtensionRuntime` block):

```rust
fn verify_dir_signature(&self, dir: &std::path::Path) -> Result<(), RuntimeError> {
    if std::env::var("GREENTIC_EXT_ALLOW_UNSIGNED").is_ok() {
        tracing::warn!(
            extension_dir = %dir.display(),
            "GREENTIC_EXT_ALLOW_UNSIGNED is set — signature verification skipped"
        );
        return Ok(());
    }
    let path = dir.join("describe.json");
    let raw = std::fs::read_to_string(&path)?;
    let describe: greentic_ext_contract::DescribeJson = serde_json::from_str(&raw)?;
    greentic_ext_contract::verify_describe(&describe).map_err(|e| {
        RuntimeError::SignatureInvalid {
            extension_id: describe.metadata.id.clone(),
            reason: e.to_string(),
        }
    })?;
    let pub_prefix = describe
        .signature
        .as_ref()
        .map(|s| s.public_key.chars().take(16).collect::<String>())
        .unwrap_or_else(|| "?".to_string());
    tracing::info!(
        extension_id = %describe.metadata.id,
        key_prefix = %pub_prefix,
        "extension signature verified"
    );
    Ok(())
}
```

Rust `is_ok()` on `std::env::var` returns true iff the var is set to **any** value (including empty string). This matches the design §4 semantics: "any set = bypass".

Wait — empty string check: `std::env::var("X")` returns `Ok("")` if set to empty string, which means `is_ok() == true`. In our `rejects_unsigned_by_default` test, we set then drop the guard (which calls `remove_var`), so `var()` returns `Err(NotPresent)` → `is_ok() == false` → proceed to verify. Correct.

- [ ] **Step 4: Run tests, confirm pass**

```bash
cargo test -p greentic-ext-runtime --test signature_gate 2>&1 | tail -15
```

Expected: 5 tests pass.

- [ ] **Step 5: Commit**

```bash
git add crates/greentic-ext-runtime/
git commit -m "feat(runtime): verify signature on extension load + GREENTIC_EXT_ALLOW_UNSIGNED escape hatch"
```

---

## Task 10: Update `runtime_load.rs` existing test to use env guard

**Files:**
- Modify: `crates/greentic-ext-runtime/tests/runtime_load.rs`

The existing `loads_extension_and_registers_caps` test uses `ExtensionFixtureBuilder` which produces unsigned describes. After Task 9, it will fail. Two options: sign the fixture, OR set env guard. We choose env guard (matches spec §4: in-repo fixture stays intentionally unsigned).

- [ ] **Step 1: Read current test file**

```bash
cat crates/greentic-ext-runtime/tests/runtime_load.rs
```

Expected: one `#[tokio::test]` function `loads_extension_and_registers_caps`.

- [ ] **Step 2: Add support module reference + env guard**

Modify `crates/greentic-ext-runtime/tests/runtime_load.rs`:

```rust
#[path = "support/mod.rs"]
mod support;

use std::path::PathBuf;

use greentic_ext_contract::ExtensionKind;
use greentic_ext_runtime::{DiscoveryPaths, ExtensionRuntime, RuntimeConfig};
use greentic_ext_testing::ExtensionFixtureBuilder;

use support::EnvGuard;

#[tokio::test]
async fn loads_extension_and_registers_caps() {
    // In-repo fixture is intentionally unsigned. Wave 1 runtime rejects
    // unsigned by default; this test opts into the escape hatch explicitly.
    let _guard = EnvGuard::set("GREENTIC_EXT_ALLOW_UNSIGNED", "1");

    let minimal_wasm = wat::parse_str(r"(component)").expect("component must compile");
    let fixture = ExtensionFixtureBuilder::new(ExtensionKind::Design, "greentic.test-ext", "0.1.0")
        .offer("greentic:test/ping", "1.0.0")
        .with_wasm(minimal_wasm)
        .build()
        .unwrap();

    let config = RuntimeConfig::from_paths(DiscoveryPaths::new(PathBuf::from("/dev/null")));
    let mut rt = ExtensionRuntime::new(config).unwrap();
    rt.register_loaded_from_dir(fixture.root()).unwrap();

    let registry = rt.capability_registry();
    assert!(
        registry
            .offerings()
            .any(|o| o.extension_id == "greentic.test-ext")
    );
}
```

- [ ] **Step 3: Run all runtime tests**

```bash
cargo test -p greentic-ext-runtime 2>&1 | tail -15
```

Expected: all tests pass including the 5 new signature_gate tests and the updated runtime_load test.

- [ ] **Step 4: Commit**

```bash
git add crates/greentic-ext-runtime/tests/runtime_load.rs
git commit -m "test(runtime): opt into GREENTIC_EXT_ALLOW_UNSIGNED for unsigned fixture load test"
```

---

## Task 11: CLI — `keygen` command

**Files:**
- Create: `crates/greentic-ext-cli/src/commands/keygen.rs`
- Modify: `crates/greentic-ext-cli/src/commands/mod.rs`
- Modify: `crates/greentic-ext-cli/Cargo.toml`

- [ ] **Step 1: Add CLI deps**

Edit `crates/greentic-ext-cli/Cargo.toml` `[dependencies]`:

```toml
base64 = { workspace = true }
ed25519-dalek = { workspace = true, features = ["pkcs8", "rand_core"] }
pkcs8 = { workspace = true }
rand = { workspace = true }
zip = { workspace = true }
```

(Some may already be present — check before adding. `base64` likely missing — add it.)

- [ ] **Step 2: Create the command module**

Write to `crates/greentic-ext-cli/src/commands/keygen.rs`:

```rust
use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use base64::Engine as _;
use clap::Args as ClapArgs;
use ed25519_dalek::pkcs8::EncodePrivateKey;
use ed25519_dalek::SigningKey;
use rand::rngs::OsRng;

#[derive(ClapArgs, Debug)]
pub struct Args {
    /// Write private key to this file instead of stdout (mode 0600).
    /// File must not already exist.
    #[arg(long)]
    pub out: Option<PathBuf>,
}

pub fn run(args: &Args, _home: &Path) -> Result<()> {
    let signing_key = SigningKey::generate(&mut OsRng);
    let pem = signing_key
        .to_pkcs8_pem(pkcs8::LineEnding::LF)
        .map_err(|e| anyhow::anyhow!("encode PKCS8 PEM: {e}"))?;

    let pubkey_b64 = base64::engine::general_purpose::STANDARD
        .encode(signing_key.verifying_key().to_bytes());

    match &args.out {
        Some(path) => {
            write_mode_0600(path, pem.as_bytes())
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
    f.write_all(bytes)
}

#[cfg(not(unix))]
fn write_mode_0600(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    // Windows: no POSIX perms. Use create_new for refuse-to-overwrite.
    let mut f = std::fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(path)?;
    f.write_all(bytes)
}
```

- [ ] **Step 3: Register module**

Edit `crates/greentic-ext-cli/src/commands/mod.rs` — add to the `pub mod …;` list (alphabetical):

```rust
pub mod keygen;
```

- [ ] **Step 4: Verify compilation**

```bash
cargo check -p greentic-ext-cli 2>&1 | tail -5
```

Expected: clean compile.

- [ ] **Step 5: Commit**

```bash
git add crates/greentic-ext-cli/
git commit -m "feat(cli): add keygen command (ed25519 PKCS8 PEM to stdout or file)"
```

---

## Task 12: CLI — `sign` command

**Files:**
- Create: `crates/greentic-ext-cli/src/commands/sign.rs`
- Modify: `crates/greentic-ext-cli/src/commands/mod.rs`

- [ ] **Step 1: Create the command module**

Write to `crates/greentic-ext-cli/src/commands/sign.rs`:

```rust
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use clap::Args as ClapArgs;
use ed25519_dalek::pkcs8::DecodePrivateKey;
use ed25519_dalek::SigningKey;
use greentic_ext_contract::DescribeJson;

#[derive(ClapArgs, Debug)]
pub struct Args {
    /// Path to describe.json to sign in-place.
    pub describe_path: PathBuf,

    /// Read PKCS8 PEM private key from this file.
    /// Mutually exclusive with --key-env.
    #[arg(long, conflicts_with = "key_env")]
    pub key: Option<PathBuf>,

    /// Read PKCS8 PEM private key from this env var.
    /// Default: GREENTIC_EXT_SIGNING_KEY_PEM
    #[arg(long, default_value = "GREENTIC_EXT_SIGNING_KEY_PEM")]
    pub key_env: String,
}

pub fn run(args: &Args, _home: &Path) -> Result<()> {
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
    let mut describe: DescribeJson =
        serde_json::from_str(&raw).context("parse describe.json")?;

    greentic_ext_contract::sign_describe(&mut describe, &signing_key)
        .context("sign describe")?;

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

- [ ] **Step 2: Register module**

Edit `crates/greentic-ext-cli/src/commands/mod.rs` — add (alphabetical):

```rust
pub mod sign;
```

- [ ] **Step 3: Verify compilation**

```bash
cargo check -p greentic-ext-cli 2>&1 | tail -5
```

Expected: clean compile.

- [ ] **Step 4: Commit**

```bash
git add crates/greentic-ext-cli/
git commit -m "feat(cli): add sign command (mutates describe.json in-place with ed25519 sig)"
```

---

## Task 13: CLI — `verify` command

**Files:**
- Create: `crates/greentic-ext-cli/src/commands/verify.rs`
- Modify: `crates/greentic-ext-cli/src/commands/mod.rs`

- [ ] **Step 1: Create the command module**

Write to `crates/greentic-ext-cli/src/commands/verify.rs`:

```rust
use std::io::Read;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use clap::Args as ClapArgs;
use greentic_ext_contract::DescribeJson;

#[derive(ClapArgs, Debug)]
pub struct Args {
    /// Path to verify. Accepts:
    ///   - describe.json file (verifies inline signature)
    ///   - extension directory (reads describe.json inside)
    ///   - .gtxpack archive (unzips describe.json to temp, verifies)
    pub path: PathBuf,
}

pub fn run(args: &Args, _home: &Path) -> Result<()> {
    let describe = load_describe(&args.path)?;
    greentic_ext_contract::verify_describe(&describe)
        .map_err(|e| anyhow::anyhow!("signature invalid: {e}"))?;
    let sig = describe.signature.as_ref().expect("verify passed → signature present");
    println!(
        "OK  {} v{} signed by {}",
        describe.metadata.id,
        describe.metadata.version,
        &sig.public_key[..16.min(sig.public_key.len())],
    );
    Ok(())
}

fn load_describe(path: &Path) -> Result<DescribeJson> {
    if path.is_file() {
        let ext = path.extension().and_then(|s| s.to_str());
        match ext {
            Some("json") => load_describe_file(path),
            Some("gtxpack") | Some("zip") => load_describe_from_archive(path),
            other => anyhow::bail!(
                "unsupported file extension: {other:?} (expected .json or .gtxpack)"
            ),
        }
    } else if path.is_dir() {
        load_describe_file(&path.join("describe.json"))
    } else {
        anyhow::bail!("not a file or directory: {}", path.display())
    }
}

fn load_describe_file(path: &Path) -> Result<DescribeJson> {
    let raw = std::fs::read_to_string(path)
        .with_context(|| format!("read {}", path.display()))?;
    serde_json::from_str(&raw)
        .with_context(|| format!("parse {}", path.display()))
}

fn load_describe_from_archive(pack_path: &Path) -> Result<DescribeJson> {
    let file = std::fs::File::open(pack_path)
        .with_context(|| format!("open {}", pack_path.display()))?;
    let mut zip = zip::ZipArchive::new(file).context("open zip")?;
    let mut entry = zip
        .by_name("describe.json")
        .context("describe.json missing from archive")?;
    let mut buf = String::new();
    entry.read_to_string(&mut buf).context("read describe.json")?;
    serde_json::from_str(&buf).context("parse describe.json")
}
```

- [ ] **Step 2: Register module**

Edit `crates/greentic-ext-cli/src/commands/mod.rs`:

```rust
pub mod verify;
```

- [ ] **Step 3: Verify compilation**

```bash
cargo check -p greentic-ext-cli 2>&1 | tail -5
```

Expected: clean compile.

- [ ] **Step 4: Commit**

```bash
git add crates/greentic-ext-cli/
git commit -m "feat(cli): add verify command (file/dir/.gtxpack signature check)"
```

---

## Task 14: Wire `Sign`/`Verify`/`Keygen` into `main.rs` dispatch

**Files:**
- Modify: `crates/greentic-ext-cli/src/main.rs`

- [ ] **Step 1: Read the current Command enum**

```bash
grep -A 25 "^enum Command" crates/greentic-ext-cli/src/main.rs
```

- [ ] **Step 2: Add variants**

Insert into the `Command` enum (after `Version`, or alphabetical — keep consistency with existing ordering):

```rust
    /// Generate an ed25519 keypair for signing extension artifacts
    Keygen(commands::keygen::Args),
    /// Sign a describe.json in-place with ed25519
    Sign(commands::sign::Args),
    /// Verify an extension's signature (file, directory, or .gtxpack)
    Verify(commands::verify::Args),
```

- [ ] **Step 3: Add dispatch arms**

In the `main()` match block, add (before the fallthrough/version, same spot):

```rust
        Command::Keygen(args) => commands::keygen::run(&args, &home),
        Command::Sign(args) => commands::sign::run(&args, &home),
        Command::Verify(args) => commands::verify::run(&args, &home),
```

- [ ] **Step 4: Build the binary**

```bash
cargo build -p greentic-ext-cli 2>&1 | tail -5
```

Expected: clean build.

- [ ] **Step 5: Smoke-test each subcommand**

```bash
./target/debug/gtdx --help 2>&1 | grep -E "keygen|sign|verify"
```

Expected: three lines listing the new subcommands.

- [ ] **Step 6: Commit**

```bash
git add crates/greentic-ext-cli/src/main.rs
git commit -m "feat(cli): wire keygen/sign/verify into top-level command dispatch"
```

---

## Task 15: CLI integration tests

**Files:**
- Create: `crates/greentic-ext-cli/tests/sign_verify_cmd.rs`

- [ ] **Step 1: Write the integration test file**

Create `crates/greentic-ext-cli/tests/sign_verify_cmd.rs`:

```rust
use std::path::PathBuf;
use std::process::Command;

use greentic_ext_contract::ExtensionKind;
use greentic_ext_testing::ExtensionFixtureBuilder;
use tempfile::TempDir;

fn gtdx_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_gtdx"))
}

fn new_describe_fixture() -> (TempDir, PathBuf) {
    let fx = ExtensionFixtureBuilder::new(ExtensionKind::Design, "greentic.cli-sign", "0.1.0")
        .offer("greentic:test/y", "1.0.0")
        .with_wasm(b"wasm".to_vec())
        .build()
        .unwrap();
    let describe = fx.root().join("describe.json");
    // Move TempDir ownership out of ExtensionFixture: we return the path but keep fx alive.
    // Since ExtensionFixture owns the TempDir, return it wrapped.
    let dir = fx.into_temp_dir();
    (dir, describe)
}

#[test]
fn keygen_writes_valid_pkcs8_to_stdout() {
    let output = Command::new(gtdx_bin())
        .arg("keygen")
        .output()
        .unwrap();
    assert!(output.status.success(), "stderr: {}",
        String::from_utf8_lossy(&output.stderr));
    let pem = String::from_utf8(output.stdout).unwrap();
    assert!(pem.starts_with("-----BEGIN PRIVATE KEY-----"));
    assert!(pem.trim_end().ends_with("-----END PRIVATE KEY-----"));
}

#[test]
fn keygen_refuses_overwrite() {
    let tmp = TempDir::new().unwrap();
    let key_path = tmp.path().join("k.pem");
    std::fs::write(&key_path, b"existing").unwrap();
    let output = Command::new(gtdx_bin())
        .arg("keygen")
        .arg("--out")
        .arg(&key_path)
        .output()
        .unwrap();
    assert!(!output.status.success(), "keygen should refuse overwrite");
}

#[test]
fn sign_then_verify_roundtrip() {
    // keygen → sign → verify, all via CLI
    let tmp = TempDir::new().unwrap();
    let key_path = tmp.path().join("k.pem");
    let out = Command::new(gtdx_bin())
        .arg("keygen")
        .arg("--out")
        .arg(&key_path)
        .output()
        .unwrap();
    assert!(out.status.success());

    let (_fx_dir, describe_path) = new_describe_fixture();
    let out = Command::new(gtdx_bin())
        .arg("sign")
        .arg(&describe_path)
        .arg("--key")
        .arg(&key_path)
        .output()
        .unwrap();
    assert!(out.status.success(), "sign stderr: {}",
        String::from_utf8_lossy(&out.stderr));

    let out = Command::new(gtdx_bin())
        .arg("verify")
        .arg(&describe_path)
        .output()
        .unwrap();
    assert!(out.status.success(), "verify stderr: {}",
        String::from_utf8_lossy(&out.stderr));
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert!(stdout.starts_with("OK  greentic.cli-sign v0.1.0"));
}

#[test]
fn sign_uses_env_var_when_no_key_flag() {
    let tmp = TempDir::new().unwrap();
    let key_path = tmp.path().join("k.pem");
    Command::new(gtdx_bin())
        .arg("keygen")
        .arg("--out")
        .arg(&key_path)
        .output()
        .unwrap();
    let pem = std::fs::read_to_string(&key_path).unwrap();

    let (_fx_dir, describe_path) = new_describe_fixture();
    let out = Command::new(gtdx_bin())
        .arg("sign")
        .arg(&describe_path)
        .env("GREENTIC_EXT_SIGNING_KEY_PEM", &pem)
        .output()
        .unwrap();
    assert!(out.status.success(), "sign stderr: {}",
        String::from_utf8_lossy(&out.stderr));

    // Verify via file
    let out = Command::new(gtdx_bin())
        .arg("verify")
        .arg(&describe_path)
        .output()
        .unwrap();
    assert!(out.status.success());
}

#[test]
fn sign_missing_key_emits_hint() {
    let (_fx_dir, describe_path) = new_describe_fixture();
    // Explicitly clear env var so it's unset.
    let out = Command::new(gtdx_bin())
        .arg("sign")
        .arg(&describe_path)
        .env_remove("GREENTIC_EXT_SIGNING_KEY_PEM")
        .output()
        .unwrap();
    assert!(!out.status.success());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("GREENTIC_EXT_SIGNING_KEY_PEM"));
    assert!(stderr.contains("--key"));
}

#[test]
fn verify_rejects_tampered() {
    let tmp = TempDir::new().unwrap();
    let key_path = tmp.path().join("k.pem");
    Command::new(gtdx_bin())
        .arg("keygen")
        .arg("--out")
        .arg(&key_path)
        .output()
        .unwrap();

    let (_fx_dir, describe_path) = new_describe_fixture();
    Command::new(gtdx_bin())
        .arg("sign")
        .arg(&describe_path)
        .arg("--key")
        .arg(&key_path)
        .output()
        .unwrap();

    // Mutate version
    let raw = std::fs::read_to_string(&describe_path).unwrap();
    let mut v: serde_json::Value = serde_json::from_str(&raw).unwrap();
    v["metadata"]["version"] = serde_json::json!("99.99.99");
    std::fs::write(&describe_path, serde_json::to_string_pretty(&v).unwrap()).unwrap();

    let out = Command::new(gtdx_bin())
        .arg("verify")
        .arg(&describe_path)
        .output()
        .unwrap();
    assert!(!out.status.success());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("signature invalid"));
}

#[test]
fn verify_accepts_directory() {
    let tmp = TempDir::new().unwrap();
    let key_path = tmp.path().join("k.pem");
    Command::new(gtdx_bin())
        .arg("keygen")
        .arg("--out")
        .arg(&key_path)
        .output()
        .unwrap();

    let (fx_dir, describe_path) = new_describe_fixture();
    Command::new(gtdx_bin())
        .arg("sign")
        .arg(&describe_path)
        .arg("--key")
        .arg(&key_path)
        .output()
        .unwrap();

    let out = Command::new(gtdx_bin())
        .arg("verify")
        .arg(fx_dir.path())
        .output()
        .unwrap();
    assert!(out.status.success(), "stderr: {}",
        String::from_utf8_lossy(&out.stderr));
}

#[test]
fn verify_accepts_gtxpack_archive() {
    use std::io::Write;
    let tmp = TempDir::new().unwrap();
    let key_path = tmp.path().join("k.pem");
    Command::new(gtdx_bin())
        .arg("keygen")
        .arg("--out")
        .arg(&key_path)
        .output()
        .unwrap();

    let (fx_dir, describe_path) = new_describe_fixture();
    Command::new(gtdx_bin())
        .arg("sign")
        .arg(&describe_path)
        .arg("--key")
        .arg(&key_path)
        .output()
        .unwrap();

    // Zip describe.json + extension.wasm into a .gtxpack
    let pack_path = tmp.path().join("ext.gtxpack");
    {
        let f = std::fs::File::create(&pack_path).unwrap();
        let mut zip = zip::ZipWriter::new(f);
        let options: zip::write::SimpleFileOptions =
            zip::write::SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Deflated);
        zip.start_file("describe.json", options).unwrap();
        zip.write_all(&std::fs::read(&describe_path).unwrap()).unwrap();
        zip.start_file("extension.wasm", options).unwrap();
        zip.write_all(&std::fs::read(fx_dir.path().join("extension.wasm")).unwrap()).unwrap();
        zip.finish().unwrap();
    }

    let out = Command::new(gtdx_bin())
        .arg("verify")
        .arg(&pack_path)
        .output()
        .unwrap();
    assert!(out.status.success(), "stderr: {}",
        String::from_utf8_lossy(&out.stderr));
}
```

- [ ] **Step 2: Check if `ExtensionFixture::into_temp_dir` exists**

```bash
grep -n "into_temp_dir\|fn root" crates/greentic-ext-testing/src/fixture.rs
```

If `into_temp_dir` doesn't exist on `ExtensionFixture`, the test helper `new_describe_fixture` above won't compile. Options:

(a) **Add the method** in `crates/greentic-ext-testing/src/fixture.rs`:

```rust
impl ExtensionFixture {
    pub fn into_temp_dir(self) -> tempfile::TempDir {
        self.dir
    }
}
```

(Assuming `ExtensionFixture` has a private `dir: TempDir` field. Verify by reading the struct definition.)

(b) **Alternative** — return the fixture itself and borrow `.root()`:

```rust
fn new_describe_fixture() -> (greentic_ext_testing::ExtensionFixture, PathBuf) {
    let fx = ExtensionFixtureBuilder::new(...).build().unwrap();
    let describe = fx.root().join("describe.json");
    (fx, describe)
}
```

Then tests use `fx.root()` for the directory. Prefer (b) if `ExtensionFixture` impls `Drop` correctly.

Apply whichever option keeps the tests simple. Commit the fixture helper change separately if (a).

- [ ] **Step 3: Add dev-dependencies for the tests**

Edit `crates/greentic-ext-cli/Cargo.toml` `[dev-dependencies]`:

```toml
[dev-dependencies]
greentic-ext-contract = { path = "../greentic-ext-contract" }
greentic-ext-testing = { path = "../greentic-ext-testing" }
tempfile = { workspace = true }
zip = { workspace = true }
```

- [ ] **Step 4: Run the CLI integration tests**

```bash
cargo test -p greentic-ext-cli --test sign_verify_cmd 2>&1 | tail -20
```

Expected: all 8 tests pass. First run will compile everything (may take > 1 minute).

- [ ] **Step 5: Commit**

```bash
git add crates/greentic-ext-cli/
git commit -m "test(cli): 8 integration tests for keygen/sign/verify commands"
```

---

## Task 16: Full workspace checks + README note

**Files:**
- Optionally modify: `README.md` (brief pointer only)

- [ ] **Step 1: Run full `ci/local_check.sh`**

```bash
bash ci/local_check.sh 2>&1 | tail -15
```

Expected: all four stages pass (fmt, clippy, test, release build). Catches anything missed.

- [ ] **Step 2: Optional — add a README line**

Edit repo root `README.md` if it exists and mentions the ecosystem. Add one line pointing at the new signing commands:

```md
- `gtdx keygen / sign / verify` — manage ed25519 signatures for extension bundles.
```

Skip if README is sparse or doesn't cover subcommands.

- [ ] **Step 3: Final commit (if README touched)**

```bash
git add README.md
git commit -m "docs(readme): mention new gtdx signing commands"
```

---

## Task 17: Push branch + open PR (no self-merge)

**Files:** none

- [ ] **Step 1: Verify branch commit count**

```bash
git log --oneline main..HEAD
```

Expected: 14–16 commits on `feat/extension-signing-pipeline` branch (one per Task above).

- [ ] **Step 2: Push**

```bash
git push -u origin feat/extension-signing-pipeline
```

Expected: branch uploaded, no conflicts.

- [ ] **Step 3: Open the PR via gh CLI**

```bash
gh pr create --base main --head feat/extension-signing-pipeline \
  --title "feat(ext): extension signing pipeline (Wave 1)" \
  --body "$(cat <<'EOF'
## Summary

Ship end-to-end signing infrastructure for Greentic extensions — Wave 1 of
the Phase B deploy-extension migration's signing sub-project.

- **JCS canonicalization** fixes the long-standing bug in
  \`greentic-ext-registry::lifecycle::verify_signature\` where describe.json
  was serialized with \`.signature\` field included in the signed payload,
  making valid signatures impossible.
- **\`sign_describe\` / \`verify_describe\`** helpers in \`greentic-ext-contract\`
  strip the signature field, canonicalize via RFC 8785 JCS, and leverage the
  existing \`verify_ed25519\` primitive.
- **Runtime verify gate** in \`ExtensionRuntime::register_loaded_from_dir\`:
  rejects unsigned or tampered extensions by default, with
  \`GREENTIC_EXT_ALLOW_UNSIGNED=1\` escape hatch for dev.
- **\`gtdx keygen / sign / verify\`** CLI subcommands mirroring the
  \`greentic-pack\` precedent.

## Test plan

- [x] \`cargo test -p greentic-ext-contract --test signature_rt\` — 12 tests pass (3 existing + 9 new)
- [x] \`cargo test -p greentic-ext-runtime --test signature_gate\` — 5 new tests pass
- [x] \`cargo test -p greentic-ext-runtime --test runtime_load\` — updated to use env guard
- [x] \`cargo test -p greentic-ext-cli --test sign_verify_cmd\` — 8 new tests pass
- [x] \`bash ci/local_check.sh\` — fmt, clippy, test, release build all green

## Follow-ups (Wave 2 and Wave 3)

- **Wave 2** (\`greentic-biz/greentic-deployer-extensions\`): adopt env-aware
  sign step in \`build.sh\`, bump \`deploy-desktop\` to \`0.2.0\` (signed), add
  new \`deploy-single-vm@0.1.0\` (signed from day 1), CI \`EXT_SIGNING_KEY_PEM\`
  secret, \`CI_REQUIRE_SIGNED\` guardrail.
- **Wave 3** (\`greenticai/greentic-deployer\`): bump pinned rev of
  \`greentic-ext-runtime\` + \`greentic-ext-contract\` to this PR's merge SHA,
  add \`BuiltinBackendId::SingleVm\` variant, adjust tests to set
  \`GREENTIC_EXT_ALLOW_UNSIGNED=1\`, bump to 0.4.54.

## Spec

\`docs/superpowers/specs/2026-04-18-extension-signing-pipeline-design.md\`

## Plan

\`docs/superpowers/plans/2026-04-18-extension-signing-pipeline-wave1.md\`

---

🤖 Implemented via subagent-driven development.
EOF
)"
```

Expected: PR URL returned.

**Do not self-merge.** The controller / user reviews and merges.

---

## Self-Review

### 1. Spec coverage

| Spec section | Tasks | Status |
| --- | --- | --- |
| §3 Wave 1 contract layer — `canonical_signing_payload` | Task 3 | ✓ |
| §3 Wave 1 contract layer — `sign_describe` | Task 4 | ✓ |
| §3 Wave 1 contract layer — `verify_describe` | Task 5 | ✓ |
| §3 Wave 1 `ContractError::Canonicalize` variant | Task 2 | ✓ |
| §3 Wave 1 contract tests (3 new) | Tasks 3/4/5 | ✓ (exceeds — 9 new) |
| §3 Wave 1 registry lifecycle fix | Task 6 | ✓ |
| §4 Wave 1 runtime `RuntimeError::SignatureInvalid` | Task 7 | ✓ |
| §4 Wave 1 `verify_signature_or_bypass` helper | Task 9 | ✓ |
| §4 Wave 1 hook in `register_loaded_from_dir` | Task 9 | ✓ |
| §4 Wave 1 `signature_gate.rs` — 5 tests | Task 9 | ✓ |
| §4 Wave 1 scoped env guard helper | Task 8 | ✓ |
| §5 Wave 1 `gtdx keygen` | Task 11 | ✓ |
| §5 Wave 1 `gtdx sign` | Task 12 | ✓ |
| §5 Wave 1 `gtdx verify` | Task 13 | ✓ |
| §5 Wave 1 CLI main.rs dispatch | Task 14 | ✓ |
| §5 Wave 1 CLI integration tests — 8 cases | Task 15 | ✓ |
| Deps: `serde_jcs`, `pkcs8`, `zip` | Task 1 | ✓ |

### 2. Placeholder scan

- No "TBD" / "TODO" / "fill in" in task bodies.
- Every code step has complete code.
- Every command has expected output.
- Task 15 Step 2 presents an (a)/(b) decision about `into_temp_dir` — not a placeholder, a real decision with both paths specified.

### 3. Type consistency

- `DescribeJson.signature` field type `Option<Signature>` — used consistently across Tasks 3/4/5/13.
- `ContractError::SignatureInvalid(String)` + new `ContractError::Canonicalize(String)` — referenced in Tasks 2/3/4/5 consistently.
- `RuntimeError::SignatureInvalid { extension_id: String, reason: String }` — matches between Task 7 (declaration) and Task 9 (construction + Task 9 tests matching).
- `canonical_signing_payload(&DescribeJson) -> Result<Vec<u8>, ContractError>` — same signature in Tasks 3 (def) and Tasks 4/5 (usage).
- `sign_describe(&mut DescribeJson, &SigningKey) -> Result<(), ContractError>` — same in Task 4 and Task 12 (CLI sign).
- `verify_describe(&DescribeJson) -> Result<(), ContractError>` — same in Task 5 and Tasks 6 / 9 / 13.
- `GREENTIC_EXT_SIGNING_KEY_PEM` env var — same name in Tasks 12 (default) and 15 (tests).
- `GREENTIC_EXT_ALLOW_UNSIGNED` env var — same name in Tasks 9 (impl) and 9/10 (tests).

Plan is internally consistent. No drift detected.
