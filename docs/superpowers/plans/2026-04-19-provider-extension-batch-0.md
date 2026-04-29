# Provider Extension — Pre-flight + Batch 0 (Pilot Telegram) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship the fourth extension kind (`ExtensionKind::Provider`) infrastructure in `greentic-designer-extensions` plus a Telegram pilot `.gtxpack` in `greentic-messaging-providers`, proving end-to-end installation, registration, and runner hot-reload.

**Architecture:** `.gtxpack` wraps `.gtpack` runtime artifact inline. `gtdx install` extracts embedded gtpack to `~/.greentic/runtime/packs/providers/gtdx/`, runner picks up via existing 30s polling. Metadata queries go through `ExtensionRegistry::list_by_kind` + `invoke_tool`. Runtime path stays in `greentic:component@0.6.0` — extension is purely additive.

**Tech Stack:** Rust 1.94.0 edition 2024, `wasm32-wasip1` cdylib (ext component) + `wasm32-wasip2` (runtime pack), `cargo-component` 0.21.1, `wit-bindgen` 0.41, `wasmtime` 43, `ed25519-dalek` with JCS canonicalization, `serde_jcs`, `serde_yaml_bw` (aliased as `serde_yaml`).

**Spec reference:** `docs/superpowers/specs/2026-04-19-provider-extension-design.md`

---

## File Structure

### Wave A — Pre-flight infrastructure (`greentic-designer-extensions`)

**Create:**
- `wit/extension-provider.wit` — new WIT package `greentic:extension-provider@0.1.0`
- `crates/greentic-extension-sdk-contract/src/describe/provider.rs` — `RuntimeGtpack` struct + sha256 validator (nested into existing Runtime struct per F1)
- `docs/how-to-write-a-provider-extension.md` — authoring guide with Telegram walkthrough
- `crates/greentic-extension-sdk-contract/tests/provider_describe.rs` — describe.json fixtures + validation
- `crates/greentic-ext-registry/tests/provider_lifecycle.rs` — install/uninstall + sha256 + conflict cases
- `crates/greentic-ext-cli/tests/provider_commands.rs` — `gtdx list --kind provider` + `info`
- `tests/fixtures/provider-fixture/` (in contract crate) — minimal fake `.gtxpack` for tests

**Modify:**
- `crates/greentic-extension-sdk-contract/src/kind.rs` — add `Provider` variant + `dir_name`
- `crates/greentic-extension-sdk-contract/src/describe.rs` — extend existing `Runtime` with optional `gtpack: Option<RuntimeGtpack>`; add `TryFrom<DescribeJsonRaw>` invariant check on `DescribeJson` (F1)
- `crates/greentic-extension-sdk-contract/src/lib.rs` — re-export provider types
- `crates/greentic-extension-sdk-contract/tests/kind.rs` — `Provider` round-trip
- `crates/greentic-ext-registry/src/registry.rs` — add `list_by_kind` + `get_describe` trait methods
- `crates/greentic-ext-registry/src/local.rs` — impl new trait methods
- `crates/greentic-ext-registry/src/store.rs` — impl new trait methods
- `crates/greentic-ext-registry/src/oci.rs` — impl new trait methods
- `crates/greentic-ext-registry/src/lifecycle.rs` — `install_provider` path with runtime.gtpack extraction + sha256 check + manual-conflict refuse
- `crates/greentic-ext-registry/src/types.rs` — `ExtensionInfo` (if missing) + `ProviderInfo` summary variant
- `crates/greentic-ext-cli/src/commands/list.rs` — `--kind provider` filter
- `crates/greentic-ext-cli/src/commands/info.rs` — provider-specific output (channels/triggers/events)
- `crates/greentic-ext-cli/src/commands/install.rs` — route to lifecycle provider path when `kind=provider`
- `Cargo.toml` (workspace) — bump `greentic-extension-sdk-contract` to 0.2.0, `greentic-ext-registry` to 0.2.0, `greentic-ext-cli` to 0.2.0 (minor bumps — new field added to describe.json is backwards-compatible for existing kinds but adds new kind)

### Wave B — Telegram pilot (`greentic-messaging-providers`)

**Create:**
- `wit/extension-base.wit` — vendored from designer-extensions
- `wit/extension-host.wit` — vendored
- `wit/extension-provider.wit` — vendored
- `wit/deps/` — transitive WIT deps if any (wasi, etc.)
- `crates/provider-telegram-extension/Cargo.toml` — cdylib crate, `cargo component` ready
- `crates/provider-telegram-extension/src/lib.rs` — WIT guest impl (messaging interface only)
- `crates/provider-telegram-extension/describe.json` — provider metadata + runtime.gtpack ref
- `crates/provider-telegram-extension/schemas/direct-message-secret.schema.json` — bot token + webhook secret JSON Schema
- `crates/provider-telegram-extension/schemas/direct-message-config.schema.json` — non-sensitive config
- `crates/provider-telegram-extension/i18n/en.json` — English wizard strings
- `crates/provider-telegram-extension/i18n/id.json` — Indonesian
- `crates/provider-telegram-extension/i18n/ja.json` — Japanese
- `crates/provider-telegram-extension/i18n/zh.json` — Chinese
- `crates/provider-telegram-extension/i18n/es.json` — Spanish
- `crates/provider-telegram-extension/i18n/de.json` — German
- `crates/provider-telegram-extension/build.sh` — build + pack + sign script
- `crates/provider-telegram-extension/tests/describe_validate.rs` — rlib sibling crate (cdylib+WIT linker workaround)
- `ci/steps/30_provider_extensions.sh` — CI step for extension build
- `.github/workflows/release.yml` additions — sign + publish on tag

**Modify:**
- `Cargo.toml` (workspace root) — add `exclude = ["crates/provider-*-extension"]` per existing reference-extensions pattern
- `ci/local_check.sh` — invoke new `30_provider_extensions.sh` step
- `README.md` — add Extension Authoring section linking to how-to doc

### Wave C — E2E validation (`greentic-e2e`)

**Create:**
- `scripts/run_provider_extension_e2e.sh` — orchestrates gtdx install + runner start + Telegram send
- `fixtures/provider-telegram-test/.env.example` — sandbox bot token template

**Modify:**
- `.github/workflows/nightly.yml` — add provider extension E2E job (optional in this plan; can defer to post-merge)

---

## Wave A — Pre-flight Infrastructure

### Task A1: Add `Provider` variant to `ExtensionKind`

**Files:**
- Modify: `crates/greentic-extension-sdk-contract/src/kind.rs`
- Modify: `crates/greentic-extension-sdk-contract/tests/kind.rs`

- [ ] **Step 1: Write the failing test**

Append to `crates/greentic-extension-sdk-contract/tests/kind.rs`:

```rust
#[test]
fn provider_kind_serde_roundtrip() {
    let original = ExtensionKind::Provider;
    let json = serde_json::to_string(&original).unwrap();
    assert_eq!(json, "\"ProviderExtension\"");
    let parsed: ExtensionKind = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed, original);
}

#[test]
fn provider_kind_dir_name() {
    assert_eq!(ExtensionKind::Provider.dir_name(), "provider");
}
```

- [ ] **Step 2: Run test to verify failure**

```bash
cargo test -p greentic-extension-sdk-contract --test kind provider_kind_serde_roundtrip provider_kind_dir_name
```

Expected: FAIL with "no variant Provider" / "no associated item `Provider`".

- [ ] **Step 3: Add the variant**

Modify `crates/greentic-extension-sdk-contract/src/kind.rs`:

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ExtensionKind {
    #[serde(rename = "DesignExtension")]
    Design,
    #[serde(rename = "BundleExtension")]
    Bundle,
    #[serde(rename = "DeployExtension")]
    Deploy,
    #[serde(rename = "ProviderExtension")]
    Provider,
}

impl ExtensionKind {
    #[must_use]
    pub const fn dir_name(self) -> &'static str {
        match self {
            Self::Design => "design",
            Self::Bundle => "bundle",
            Self::Deploy => "deploy",
            Self::Provider => "provider",
        }
    }
}
```

- [ ] **Step 4: Run test to verify pass**

```bash
cargo test -p greentic-extension-sdk-contract --test kind
```

Expected: PASS (all kind tests including new provider tests).

- [ ] **Step 5: Commit**

```bash
git add crates/greentic-extension-sdk-contract/src/kind.rs crates/greentic-extension-sdk-contract/tests/kind.rs
git commit -m "feat(contract): add Provider variant to ExtensionKind"
```

---

### Task A2: Define `RuntimeGtpack` struct (nested into existing Runtime)

**Design note (F1):** The existing `DescribeJson` struct in `describe/mod.rs` already has a `Runtime` field with `component`/`memoryLimitMB`/`permissions`. We don't create a parallel describe struct — we add `RuntimeGtpack` as an OPTIONAL nested field on the existing `Runtime` struct. See spec §8.2 "Schema integration notes".

**Files:**
- Create: `crates/greentic-extension-sdk-contract/src/describe/provider.rs` — just `RuntimeGtpack` struct + sha256 validator
- Modify: `crates/greentic-extension-sdk-contract/src/describe.rs` (restructure to module dir: `describe.rs` → `describe/mod.rs`)
- Modify: `crates/greentic-extension-sdk-contract/src/lib.rs` — re-export `RuntimeGtpack`
- Create: `crates/greentic-extension-sdk-contract/tests/runtime_gtpack.rs` — unit tests for struct + sha256 validation

- [ ] **Step 1: Read current describe.rs**

```bash
cat crates/greentic-extension-sdk-contract/src/describe.rs | head -120
```

Note the `Runtime` struct shape (line ~79-90 equivalent): `component`, `memory_limit_mb`, `permissions`. We'll extend this in Task A3 — not in this task.

- [ ] **Step 2: Write failing tests first (TDD)**

Create `crates/greentic-extension-sdk-contract/tests/runtime_gtpack.rs`:

```rust
use greentic_extension_sdk_contract::describe::RuntimeGtpack;

#[test]
fn runtime_gtpack_parses_from_json() {
    let json = serde_json::json!({
        "file": "runtime/provider.gtpack",
        "sha256": "a".repeat(64),
        "pack_id": "greentic.provider.telegram",
        "component_version": "0.6.0"
    });
    let rg: RuntimeGtpack = serde_json::from_value(json).unwrap();
    assert_eq!(rg.file, "runtime/provider.gtpack");
    assert_eq!(rg.pack_id, "greentic.provider.telegram");
    assert_eq!(rg.component_version, "0.6.0");
}

#[test]
fn runtime_gtpack_rejects_short_sha256() {
    let json = serde_json::json!({
        "file": "runtime/provider.gtpack",
        "sha256": "abc",
        "pack_id": "greentic.provider.x",
        "component_version": "0.6.0"
    });
    let err = serde_json::from_value::<RuntimeGtpack>(json).unwrap_err();
    assert!(err.to_string().to_lowercase().contains("sha256"));
}

#[test]
fn runtime_gtpack_rejects_non_hex_sha256() {
    let mut json = serde_json::json!({
        "file": "runtime/provider.gtpack",
        "sha256": "z".repeat(64),
        "pack_id": "greentic.provider.x",
        "component_version": "0.6.0"
    });
    // mutation for clarity — actually z.repeat(64) is non-hex
    let _ = &mut json;
    let err = serde_json::from_value::<RuntimeGtpack>(serde_json::json!({
        "file": "runtime/provider.gtpack",
        "sha256": "z".repeat(64),
        "pack_id": "greentic.provider.x",
        "component_version": "0.6.0"
    })).unwrap_err();
    assert!(err.to_string().to_lowercase().contains("sha256"));
}
```

- [ ] **Step 3: Run to verify failure**

```bash
cargo test -p greentic-extension-sdk-contract --test runtime_gtpack
```

Expected: FAIL (unresolved import `RuntimeGtpack`).

- [ ] **Step 4: Create the struct**

Create `crates/greentic-extension-sdk-contract/src/describe/provider.rs`:

```rust
//! Provider-specific extensions to the describe schema.
//!
//! `RuntimeGtpack` is an optional nested field on `Runtime` — populated when
//! `kind == ProviderExtension`. Enforces SHA-256 format at parse time.

use serde::{Deserialize, Deserializer, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RuntimeGtpack {
    pub file: String,
    #[serde(deserialize_with = "deserialize_sha256")]
    pub sha256: String,
    pub pack_id: String,
    pub component_version: String,
}

fn deserialize_sha256<'de, D>(d: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    let s = String::deserialize(d)?;
    if s.len() != 64 || !s.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(serde::de::Error::custom(format!(
            "invalid sha256: expected 64 lowercase hex chars, got len={} value={s:?}",
            s.len()
        )));
    }
    Ok(s)
}
```

- [ ] **Step 5: Restructure describe.rs → describe/mod.rs**

```bash
mkdir -p crates/greentic-extension-sdk-contract/src/describe
git mv crates/greentic-extension-sdk-contract/src/describe.rs crates/greentic-extension-sdk-contract/src/describe/mod.rs
```

At the top of the new `describe/mod.rs`, add:
```rust
pub mod provider;
pub use provider::RuntimeGtpack;
```

- [ ] **Step 6: Update lib.rs re-export**

Read `crates/greentic-extension-sdk-contract/src/lib.rs`. Find the existing `pub use self::describe::{...}` line. Add `RuntimeGtpack` to it:
```rust
pub use self::describe::{DescribeJson, RuntimeGtpack, /* existing items */};
```

Do NOT add `ProviderRuntime` — it doesn't exist in F1.

- [ ] **Step 7: Run tests**

```bash
cargo test -p greentic-extension-sdk-contract --test runtime_gtpack
cargo test -p greentic-extension-sdk-contract
cargo clippy -p greentic-extension-sdk-contract --all-targets -- -D warnings
```

Expected: new tests PASS, full suite PASS, clippy clean.

- [ ] **Step 8: Commit**

```bash
git add crates/greentic-extension-sdk-contract/
git commit -m "feat(contract): add RuntimeGtpack struct for provider extension artifacts"
```

---

### Task A3: Add optional `gtpack` field to existing `Runtime` struct + enforce kind↔gtpack invariant

**Design note (F1):** We extend the EXISTING `Runtime` struct in `describe/mod.rs` with a new optional `gtpack: Option<RuntimeGtpack>` field. We also enforce on the existing `DescribeJson` struct: `kind == Provider ↔ runtime.gtpack.is_some()`. No new `Describe` struct — `DescribeJson` remains the single source of truth.

**Files:**
- Modify: `crates/greentic-extension-sdk-contract/src/describe/mod.rs` — add `gtpack` to `Runtime`, add `TryFrom<DescribeJsonRaw>` validation
- Modify: `crates/greentic-extension-sdk-contract/tests/describe_roundtrip.rs` — add 3 invariant tests
- Modify any existing test fixtures in `greentic-extension-sdk-contract/tests/` that construct `DescribeJson` / `Runtime` to include `gtpack: None` (should be default via `#[serde(default)]` — no change needed if fixtures deserialize from JSON)

- [ ] **Step 1: Read current describe/mod.rs**

```bash
cat crates/greentic-extension-sdk-contract/src/describe/mod.rs
```

Note the `Runtime` struct at approx line 79-90 and the `DescribeJson` struct at line 11-26. These are what we modify.

- [ ] **Step 2: Write failing tests (TDD)**

Append to `crates/greentic-extension-sdk-contract/tests/describe_roundtrip.rs`:

```rust
fn hex64(c: char) -> String {
    std::iter::repeat(c).take(64).collect()
}

fn base_metadata() -> serde_json::Value {
    serde_json::json!({
        "id": "greentic.provider.telegram",
        "name": "Telegram",
        "version": "0.1.0",
        "summary": "Telegram messaging provider",
        "author": { "name": "Greentic" },
        "license": "Apache-2.0"
    })
}

fn base_engine() -> serde_json::Value {
    serde_json::json!({ "greenticDesigner": "*", "extRuntime": "^0.1.0" })
}

fn base_capabilities() -> serde_json::Value {
    serde_json::json!({ "offered": [], "required": [] })
}

#[test]
fn describe_with_kind_provider_and_gtpack_roundtrips() {
    let json = serde_json::json!({
        "apiVersion": "greentic.ai/v1",
        "kind": "ProviderExtension",
        "metadata": base_metadata(),
        "engine": base_engine(),
        "capabilities": base_capabilities(),
        "runtime": {
            "component": "wasm/provider_telegram_ext.wasm",
            "memoryLimitMB": 64,
            "permissions": { "network": [], "secrets": [], "callExtensionKinds": [] },
            "gtpack": {
                "file": "runtime/provider.gtpack",
                "sha256": hex64('b'),
                "pack_id": "greentic.provider.telegram",
                "component_version": "0.6.0"
            }
        },
        "contributions": {}
    });
    let describe: greentic_extension_sdk_contract::DescribeJson =
        serde_json::from_value(json).unwrap();
    assert_eq!(describe.kind, greentic_extension_sdk_contract::ExtensionKind::Provider);
    assert!(describe.runtime.gtpack.is_some());
    let gt = describe.runtime.gtpack.as_ref().unwrap();
    assert_eq!(gt.pack_id, "greentic.provider.telegram");
    // Re-serialize and re-parse
    let v = serde_json::to_value(&describe).unwrap();
    let _: greentic_extension_sdk_contract::DescribeJson = serde_json::from_value(v).unwrap();
}

#[test]
fn describe_with_kind_provider_requires_gtpack() {
    let json = serde_json::json!({
        "apiVersion": "greentic.ai/v1",
        "kind": "ProviderExtension",
        "metadata": base_metadata(),
        "engine": base_engine(),
        "capabilities": base_capabilities(),
        "runtime": {
            "component": "wasm/provider_telegram_ext.wasm",
            "memoryLimitMB": 64,
            "permissions": { "network": [], "secrets": [], "callExtensionKinds": [] }
        },
        "contributions": {}
    });
    let err = serde_json::from_value::<greentic_extension_sdk_contract::DescribeJson>(json)
        .unwrap_err()
        .to_string()
        .to_lowercase();
    assert!(
        err.contains("gtpack") || err.contains("provider"),
        "error should explain missing gtpack field; got: {err}"
    );
}

#[test]
fn describe_non_provider_rejects_gtpack() {
    let json = serde_json::json!({
        "apiVersion": "greentic.ai/v1",
        "kind": "DesignExtension",
        "metadata": base_metadata(),
        "engine": base_engine(),
        "capabilities": base_capabilities(),
        "runtime": {
            "component": "wasm/something.wasm",
            "memoryLimitMB": 64,
            "permissions": { "network": [], "secrets": [], "callExtensionKinds": [] },
            "gtpack": {
                "file": "runtime/provider.gtpack",
                "sha256": hex64('c'),
                "pack_id": "x",
                "component_version": "0.6.0"
            }
        },
        "contributions": {}
    });
    let err = serde_json::from_value::<greentic_extension_sdk_contract::DescribeJson>(json);
    assert!(err.is_err(), "non-provider kinds must reject gtpack field");
}
```

- [ ] **Step 3: Run to verify failure**

```bash
cargo test -p greentic-extension-sdk-contract --test describe_roundtrip
```

Expected: FAIL (compile error: no `gtpack` field; or validation not present).

- [ ] **Step 4: Extend `Runtime` struct with optional `gtpack`**

In `crates/greentic-extension-sdk-contract/src/describe/mod.rs`, modify the existing `Runtime` struct:

```rust
use crate::describe::provider::RuntimeGtpack;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Runtime {
    pub component: String,
    #[serde(rename = "memoryLimitMB", default = "default_memory")]
    pub memory_limit_mb: u32,
    pub permissions: Permissions,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gtpack: Option<RuntimeGtpack>,   // NEW
}
```

- [ ] **Step 5: Add `TryFrom` validation on `DescribeJson`**

Apply the `TryFrom<DescribeJsonRaw>` pattern to enforce the invariant. In the same file:

```rust
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct DescribeJsonRaw {
    #[serde(rename = "$schema", skip_serializing_if = "Option::is_none", default)]
    schema_ref: Option<String>,
    #[serde(rename = "apiVersion")]
    api_version: String,
    kind: crate::kind::ExtensionKind,
    metadata: Metadata,
    engine: Engine,
    capabilities: Capabilities,
    runtime: Runtime,
    contributions: serde_json::Value,
    #[serde(default)]
    signature: Option<Signature>,
}

impl TryFrom<DescribeJsonRaw> for DescribeJson {
    type Error = String;

    fn try_from(raw: DescribeJsonRaw) -> Result<Self, String> {
        use crate::kind::ExtensionKind;
        let has_gtpack = raw.runtime.gtpack.is_some();
        match (raw.kind, has_gtpack) {
            (ExtensionKind::Provider, false) => Err(
                "kind=ProviderExtension requires `runtime.gtpack`".into()
            ),
            (k, true) if k != ExtensionKind::Provider => Err(format!(
                "runtime.gtpack only allowed when kind=ProviderExtension (got kind={k:?})"
            )),
            _ => Ok(DescribeJson {
                schema_ref: raw.schema_ref,
                api_version: raw.api_version,
                kind: raw.kind,
                metadata: raw.metadata,
                engine: raw.engine,
                capabilities: raw.capabilities,
                runtime: raw.runtime,
                contributions: raw.contributions,
                signature: raw.signature,
            }),
        }
    }
}
```

Change the existing `#[derive(Debug, Clone, Serialize, Deserialize)]` on `DescribeJson` to `#[derive(Debug, Clone, Serialize)]` and add `#[serde(try_from = "DescribeJsonRaw")]` for the Deserialize path — or implement `Deserialize` manually that delegates to `DescribeJsonRaw::deserialize(...).and_then(Self::try_from)`.

**Derive approach (cleaner if compiler accepts):**

```rust
#[derive(Debug, Clone, Serialize)]
pub struct DescribeJson {
    /* existing fields unchanged */
}

impl<'de> Deserialize<'de> for DescribeJson {
    fn deserialize<D>(d: D) -> Result<Self, D::Error>
    where D: serde::Deserializer<'de>,
    {
        let raw = DescribeJsonRaw::deserialize(d)?;
        Self::try_from(raw).map_err(serde::de::Error::custom)
    }
}
```

`ExtensionKind` needs `Debug` for the error format — verify this is already derived (A1 confirmed it is).

- [ ] **Step 6: Update existing tests / fixtures that instantiate `Runtime` programmatically**

Search for `Runtime {` in tests:

```bash
grep -rn "Runtime {" crates/greentic-extension-sdk-contract/tests/ crates/greentic-extension-sdk-contract/src/
```

For any construction site, add `gtpack: None`. (If fixtures only parse JSON, skip — defaults handle it.)

- [ ] **Step 7: Run tests + clippy**

```bash
cargo test -p greentic-extension-sdk-contract
cargo clippy -p greentic-extension-sdk-contract --all-targets -- -D warnings
```

Expected: all green, no warnings. Check `cargo test --workspace` later passes too (downstream crates might use Runtime constructor).

```bash
cargo build --workspace --all-features
```

If downstream crates fail (`greentic-ext-registry`, `greentic-ext-cli`), add `gtpack: None` to their Runtime constructors.

- [ ] **Step 8: Commit**

```bash
git add crates/
git commit -m "feat(contract): add optional runtime.gtpack field + kind=Provider invariant"
```

---

### Task A4: Create `extension-provider.wit` WIT package

**Files:**
- Create: `wit/extension-provider.wit`

- [ ] **Step 1: Write the WIT package**

Create `wit/extension-provider.wit`:

```wit
package greentic:extension-provider@0.1.0;

interface types {
  type channel-id = string;
  type trigger-id = string;
  type event-id = string;

  enum direction {
    inbound,
    outbound,
    bidirectional,
  }

  enum card-tier {
    tier-a-native,
    tier-b-attachment,
    tier-c-fallback,
    tier-d-text-only,
  }

  record channel-profile {
    id: channel-id,
    display-name: string,
    direction: direction,
    tier-support: list<card-tier>,
    metadata: list<tuple<string, string>>,
  }

  record trigger-profile {
    id: trigger-id,
    display-name: string,
    emit-shape: string,
  }

  record event-profile {
    id: event-id,
    display-name: string,
    payload-shape: string,
  }

  variant error {
    not-found(string),
    schema-invalid(string),
    internal(string),
  }
}

interface messaging {
  use types.{channel-id, channel-profile, error};

  list-channels: func() -> list<channel-profile>;
  describe-channel: func(id: channel-id) -> result<channel-profile, error>;
  secret-schema: func(id: channel-id) -> result<string, error>;
  config-schema: func(id: channel-id) -> result<string, error>;
  dry-run-encode: func(id: channel-id, sample: list<u8>) -> result<list<u8>, error>;
}

interface event-source {
  use types.{trigger-id, trigger-profile, error};

  list-trigger-types: func() -> list<trigger-profile>;
  describe-trigger: func(id: trigger-id) -> result<trigger-profile, error>;
  trigger-schema: func(id: trigger-id) -> result<string, error>;
}

interface event-sink {
  use types.{event-id, event-profile, error};

  list-event-types: func() -> list<event-profile>;
  describe-event: func(id: event-id) -> result<event-profile, error>;
  event-schema: func(id: event-id) -> result<string, error>;
}

world messaging-only-provider {
  include greentic:extension-base/base@0.1.0;
  export messaging;
}

world event-source-only-provider {
  include greentic:extension-base/base@0.1.0;
  export event-source;
}

world event-sink-only-provider {
  include greentic:extension-base/base@0.1.0;
  export event-sink;
}

world messaging-and-event-source-provider {
  include greentic:extension-base/base@0.1.0;
  export messaging;
  export event-source;
}

world messaging-and-event-sink-provider {
  include greentic:extension-base/base@0.1.0;
  export messaging;
  export event-sink;
}

world full-provider {
  include greentic:extension-base/base@0.1.0;
  export messaging;
  export event-source;
  export event-sink;
}
```

- [ ] **Step 2: Verify WIT parses**

```bash
which wit-bindgen && wit-bindgen --help 2>&1 | head -3 || echo "wit-bindgen not installed; skipping parse check"
wasm-tools component wit wit/ 2>&1 | head -20
```

Expected: output lists the package + interfaces + worlds without errors. If `wasm-tools` not installed: `cargo install wasm-tools` first.

- [ ] **Step 3: Commit**

```bash
git add wit/extension-provider.wit
git commit -m "feat(wit): add greentic:extension-provider@0.1.0 contract"
```

---

### Task A5: Add `list_by_kind` + `get_describe` to `ExtensionRegistry` trait

**Files:**
- Modify: `crates/greentic-ext-registry/src/registry.rs`
- Modify: `crates/greentic-ext-registry/src/types.rs` (if needed)

- [ ] **Step 1: Read existing registry types**

Read `crates/greentic-ext-registry/src/types.rs` to see `ExtensionSummary`, `ExtensionMetadata` shapes. Determine if `ExtensionSummary` already carries kind; if not, add it.

- [ ] **Step 2: Ensure `ExtensionSummary` has `kind` field**

If missing, modify `crates/greentic-ext-registry/src/types.rs`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtensionSummary {
    pub id: String,
    pub version: String,
    pub kind: greentic_extension_sdk_contract::ExtensionKind,
    pub description: String,
    // ... existing fields
}
```

If already present: skip this step.

- [ ] **Step 3: Add trait methods with default implementations**

Modify `crates/greentic-ext-registry/src/registry.rs`:

```rust
use greentic_extension_sdk_contract::ExtensionKind;

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

    // NEW: enumerate by kind (default impl filters search results)
    async fn list_by_kind(
        &self,
        kind: ExtensionKind,
    ) -> Result<Vec<ExtensionSummary>, RegistryError> {
        let all = self.search(SearchQuery::default()).await?;
        Ok(all.into_iter().filter(|s| s.kind == kind).collect())
    }

    // NEW: return parsed describe.json (default impl wraps metadata)
    async fn get_describe(
        &self,
        name: &str,
        version: &str,
    ) -> Result<greentic_extension_sdk_contract::Describe, RegistryError> {
        let metadata = self.metadata(name, version).await?;
        // ExtensionMetadata must already carry Describe; if not, registry-specific impl
        Ok(metadata.describe)
    }
}
```

The default implementations let concrete registries inherit for free; optimized implementations can override.

- [ ] **Step 4: Run full registry test suite**

```bash
cargo test -p greentic-ext-registry
```

Expected: all existing tests still green.

- [ ] **Step 5: Commit**

```bash
git add crates/greentic-ext-registry/src/registry.rs crates/greentic-ext-registry/src/types.rs
git commit -m "feat(registry): add list_by_kind + get_describe trait methods"
```

---

### Task A6: Extend lifecycle install for `runtime.gtpack` extraction

**Files:**
- Modify: `crates/greentic-ext-registry/src/lifecycle.rs`
- Create: `crates/greentic-ext-registry/tests/provider_lifecycle.rs`
- Create: `crates/greentic-ext-registry/tests/fixtures/provider_minimal/` (regenerated per test run)

- [ ] **Step 1: Write failing test for provider install**

Create `crates/greentic-ext-registry/tests/provider_lifecycle.rs`:

```rust
use greentic_extension_sdk_contract::ExtensionKind;
use greentic_ext_registry::lifecycle::{InstallOptions, install_from_path};
use sha2::{Digest, Sha256};
use tempfile::tempdir;

mod support;
use support::build_provider_fixture_gtxpack;

#[tokio::test]
async fn install_provider_extracts_gtpack_to_providers_gtdx_dir() {
    let tmp = tempdir().unwrap();
    let home = tmp.path().join("home");
    std::fs::create_dir_all(&home).unwrap();

    // Fake gtpack content (runner doesn't load it during lifecycle test — just bytes)
    let gtpack_bytes = b"fake-gtpack-content".to_vec();
    let sha = hex::encode(Sha256::digest(&gtpack_bytes));

    let gtxpack_path = build_provider_fixture_gtxpack(
        tmp.path(),
        "greentic.provider.fixture",
        "0.1.0",
        &gtpack_bytes,
        &sha,
    );

    let opts = InstallOptions {
        install_root: home.clone(),
        allow_unsigned: true,
        force: false,
    };
    install_from_path(&gtxpack_path, &opts).await.unwrap();

    let extracted_pack = home
        .join(".greentic/runtime/packs/providers/gtdx")
        .join("greentic.provider.fixture-0.1.0.gtpack");
    assert!(extracted_pack.exists(), "extracted gtpack should exist");
    assert_eq!(std::fs::read(&extracted_pack).unwrap(), gtpack_bytes);

    let metadata_dir = home.join(".greentic/extensions/provider/greentic.provider.fixture/0.1.0");
    assert!(metadata_dir.join("describe.json").exists());
}

#[tokio::test]
async fn install_provider_rejects_sha256_mismatch() {
    let tmp = tempdir().unwrap();
    let home = tmp.path().join("home");
    std::fs::create_dir_all(&home).unwrap();

    let real = b"real-bytes".to_vec();
    let wrong_sha = "a".repeat(64);
    let gtxpack_path = build_provider_fixture_gtxpack(
        tmp.path(),
        "greentic.provider.fake",
        "0.1.0",
        &real,
        &wrong_sha,
    );

    let opts = InstallOptions {
        install_root: home.clone(),
        allow_unsigned: true,
        force: false,
    };
    let err = install_from_path(&gtxpack_path, &opts).await.unwrap_err();
    assert!(
        err.to_string().to_lowercase().contains("sha256"),
        "expected sha256 error, got: {err}"
    );
}

#[tokio::test]
async fn install_provider_refuses_conflict_with_manual_pack() {
    let tmp = tempdir().unwrap();
    let home = tmp.path().join("home");
    let manual_dir = home.join(".greentic/runtime/packs/providers/manual");
    std::fs::create_dir_all(&manual_dir).unwrap();
    // Pre-populate a manual gtpack with matching pack_id (we use a marker file for the test;
    // the real check must read manifest.cbor from gtpacks. If that machinery isn't
    // available in a test-helper form yet, gate this test on `ignore` with a TODO).
    std::fs::write(
        manual_dir.join("telegram.gtpack"),
        support::encode_gtpack_with_pack_id("greentic.provider.telegram"),
    )
    .unwrap();

    let gtpack_bytes = b"new-bytes".to_vec();
    let sha = hex::encode(Sha256::digest(&gtpack_bytes));
    let gtxpack_path = build_provider_fixture_gtxpack(
        tmp.path(),
        "greentic.provider.telegram",
        "0.1.0",
        &gtpack_bytes,
        &sha,
    );

    let opts = InstallOptions {
        install_root: home.clone(),
        allow_unsigned: true,
        force: false,
    };
    let err = install_from_path(&gtxpack_path, &opts).await.unwrap_err();
    assert!(
        err.to_string().to_lowercase().contains("conflict")
            || err.to_string().to_lowercase().contains("manual"),
        "expected conflict error, got: {err}"
    );
}
```

Create `crates/greentic-ext-registry/tests/support/mod.rs` with `#![allow(dead_code)]`:

```rust
#![allow(dead_code)]

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use serde_json::json;
use zip::write::SimpleFileOptions;

pub fn build_provider_fixture_gtxpack(
    staging_root: &Path,
    id: &str,
    version: &str,
    gtpack_bytes: &[u8],
    sha256: &str,
) -> PathBuf {
    let out = staging_root.join(format!("{id}-{version}.gtxpack"));
    let file = fs::File::create(&out).unwrap();
    let mut zip = zip::ZipWriter::new(file);
    let opts = SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Stored);

    let describe = json!({
        "kind": "ProviderExtension",
        "id": id,
        "version": version,
        "engine": { "greenticDesigner": "*", "extRuntime": "^0.1.0" },
        "capabilities": [],
        "runtime": {
            "gtpack": {
                "file": "runtime/provider.gtpack",
                "sha256": sha256,
                "pack_id": id,
                "component_version": "0.6.0"
            }
        },
        "metadata": { "author": { "name": "Fixture" } }
    });
    zip.start_file("describe.json", opts).unwrap();
    zip.write_all(serde_json::to_string_pretty(&describe).unwrap().as_bytes())
        .unwrap();

    zip.start_file("runtime/provider.gtpack", opts).unwrap();
    zip.write_all(gtpack_bytes).unwrap();

    zip.finish().unwrap();
    out
}

pub fn encode_gtpack_with_pack_id(pack_id: &str) -> Vec<u8> {
    // Minimal gtpack ZIP with a manifest.cbor that contains pack_id.
    // Use ciborium for CBOR encoding.
    let manifest = json!({ "pack_id": pack_id, "version": "0.1.0" });
    let mut cbor_bytes = Vec::new();
    ciborium::into_writer(&manifest, &mut cbor_bytes).unwrap();

    let mut buf = Vec::new();
    {
        let cursor = std::io::Cursor::new(&mut buf);
        let mut zip = zip::ZipWriter::new(cursor);
        let opts = SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Stored);
        zip.start_file("manifest.cbor", opts).unwrap();
        zip.write_all(&cbor_bytes).unwrap();
        zip.finish().unwrap();
    }
    buf
}
```

Add to `crates/greentic-ext-registry/Cargo.toml` dev-dependencies:

```toml
[dev-dependencies]
ciborium = { workspace = true }
hex = { workspace = true }
sha2 = { workspace = true }
tempfile = { workspace = true }
tokio = { workspace = true, features = ["macros", "rt-multi-thread"] }
zip = { workspace = true }
```

- [ ] **Step 2: Run to verify failure**

```bash
cargo test -p greentic-ext-registry --test provider_lifecycle
```

Expected: FAIL — `install_from_path` either doesn't exist yet or doesn't handle `ProviderExtension` kind.

- [ ] **Step 3: Implement provider install path in lifecycle**

Read existing `crates/greentic-ext-registry/src/lifecycle.rs` to understand current install flow and `InstallOptions` struct. Extend with provider-specific branch.

Sketch of additions to `lifecycle.rs`:

```rust
use greentic_extension_sdk_contract::{DescribeJson, ExtensionKind, RuntimeGtpack};
use sha2::{Digest, Sha256};
use std::io::Read;

// Entry point — may already exist; extend match on kind
pub async fn install_from_path(
    gtxpack_path: &Path,
    opts: &InstallOptions,
) -> Result<(), LifecycleError> {
    let describe = load_describe_from_gtxpack(gtxpack_path)?;
    if !opts.allow_unsigned {
        verify_signature(&describe)?;  // existing Wave 1 function
    }
    match describe.kind {
        ExtensionKind::Design | ExtensionKind::Bundle | ExtensionKind::Deploy => {
            install_standard(gtxpack_path, &describe, opts).await
        }
        ExtensionKind::Provider => {
            install_provider(gtxpack_path, &describe, opts).await
        }
    }
}

async fn install_provider(
    gtxpack_path: &Path,
    describe: &DescribeJson,
    opts: &InstallOptions,
) -> Result<(), LifecycleError> {
    // F1: gtpack lives on describe.runtime.gtpack; invariant enforced at deserialize.
    let gtpack = describe
        .runtime
        .gtpack
        .as_ref()
        .ok_or_else(|| LifecycleError::Invalid("missing runtime.gtpack (TryFrom should have caught this)".into()))?;

    // 1. Extract embedded gtpack bytes + verify sha256
    let gtpack_bytes = read_file_from_zip(gtxpack_path, &gtpack.file)?;
    let actual_sha = hex::encode(Sha256::digest(&gtpack_bytes));
    if actual_sha != gtpack.sha256 {
        return Err(LifecycleError::Invalid(format!(
            "sha256 mismatch: describe={}, actual={}",
            gtpack.sha256, actual_sha
        )));
    }

    // 2. Conflict check vs manual/ directory
    let manual_dir = opts
        .install_root
        .join(".greentic/runtime/packs/providers/manual");
    if manual_dir.exists() && !opts.force {
        check_manual_pack_conflict(&manual_dir, &gtpack.pack_id)?;
    }

    // 3. Extract metadata (describe + schemas + i18n + wasm) to extensions dir
    let meta_dir = opts
        .install_root
        .join(".greentic/extensions/provider")
        .join(&describe.metadata.id)
        .join(&describe.metadata.version);
    std::fs::create_dir_all(&meta_dir)?;
    extract_metadata_files(gtxpack_path, &meta_dir, &gtpack.file)?;

    // 4. Drop gtpack to runner pack directory
    let gtdx_dir = opts
        .install_root
        .join(".greentic/runtime/packs/providers/gtdx");
    std::fs::create_dir_all(&gtdx_dir)?;
    let out = gtdx_dir.join(format!("{}-{}.gtpack", describe.metadata.id, describe.metadata.version));
    std::fs::write(&out, &gtpack_bytes)?;

    // 5. Register in gtdx.db (existing Storage layer)
    register_in_registry(describe, &meta_dir, &out)?;

    Ok(())
}

fn check_manual_pack_conflict(
    manual_dir: &Path,
    pack_id: &str,
) -> Result<(), LifecycleError> {
    for entry in std::fs::read_dir(manual_dir)? {
        let entry = entry?;
        if entry.path().extension().and_then(|e| e.to_str()) != Some("gtpack") {
            continue;
        }
        let manifest_pack_id = read_pack_id_from_gtpack(&entry.path())?;
        if manifest_pack_id == pack_id {
            return Err(LifecycleError::Invalid(format!(
                "conflict: manual pack at {} has same pack_id={pack_id}; \
                 remove manually or re-run with --force",
                entry.path().display()
            )));
        }
    }
    Ok(())
}

fn read_pack_id_from_gtpack(path: &Path) -> Result<String, LifecycleError> {
    let file = std::fs::File::open(path)?;
    let mut zip = zip::ZipArchive::new(file)?;
    let mut manifest_file = zip
        .by_name("manifest.cbor")
        .map_err(|_| LifecycleError::Invalid("gtpack missing manifest.cbor".into()))?;
    let mut bytes = Vec::new();
    manifest_file.read_to_end(&mut bytes)?;
    let v: serde_json::Value = ciborium::from_reader(bytes.as_slice())
        .map_err(|e| LifecycleError::Invalid(format!("cbor parse: {e}")))?;
    v.get("pack_id")
        .and_then(|x| x.as_str())
        .map(String::from)
        .ok_or_else(|| LifecycleError::Invalid("manifest.cbor missing pack_id".into()))
}

fn read_file_from_zip(gtxpack: &Path, name: &str) -> Result<Vec<u8>, LifecycleError> {
    let file = std::fs::File::open(gtxpack)?;
    let mut zip = zip::ZipArchive::new(file)?;
    let mut entry = zip.by_name(name).map_err(|_| {
        LifecycleError::Invalid(format!(".gtxpack missing {name}"))
    })?;
    let mut bytes = Vec::new();
    entry.read_to_end(&mut bytes)?;
    Ok(bytes)
}

fn extract_metadata_files(
    gtxpack: &Path,
    target_dir: &Path,
    skip_path: &str,
) -> Result<(), LifecycleError> {
    let file = std::fs::File::open(gtxpack)?;
    let mut zip = zip::ZipArchive::new(file)?;
    for i in 0..zip.len() {
        let mut entry = zip.by_index(i)?;
        let name = entry.name().to_string();
        if name == skip_path || name.ends_with('/') {
            continue;
        }
        let out = target_dir.join(&name);
        if let Some(parent) = out.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut buf = Vec::new();
        entry.read_to_end(&mut buf)?;
        std::fs::write(out, buf)?;
    }
    Ok(())
}
```

- [ ] **Step 4: Run tests**

```bash
cargo test -p greentic-ext-registry --test provider_lifecycle
```

Expected: all 3 tests PASS.

- [ ] **Step 5: Run full test suite**

```bash
cargo test -p greentic-ext-registry
```

Expected: all existing tests still pass.

- [ ] **Step 6: Commit**

```bash
git add crates/greentic-ext-registry/
git commit -m "feat(registry): install_provider handles runtime.gtpack extraction + conflict checks"
```

---

### Task A7: Implement `list_by_kind` override in local + store + OCI registries

**Files:**
- Modify: `crates/greentic-ext-registry/src/local.rs`
- Modify: `crates/greentic-ext-registry/src/store.rs`
- Modify: `crates/greentic-ext-registry/src/oci.rs`

- [ ] **Step 1: Write test for local registry filter**

Append to `crates/greentic-ext-registry/tests/local_registry.rs`:

```rust
use greentic_extension_sdk_contract::ExtensionKind;
use greentic_ext_registry::{ExtensionRegistry, LocalFilesystemRegistry};

#[tokio::test]
async fn local_list_by_kind_returns_only_matching() {
    let fixture_root = /* existing test fixture dir */;
    let reg = LocalFilesystemRegistry::new(fixture_root).unwrap();
    let providers = reg.list_by_kind(ExtensionKind::Provider).await.unwrap();
    for s in &providers {
        assert_eq!(s.kind, ExtensionKind::Provider);
    }
}
```

The default trait impl (filter all) already gives correct behavior — the purpose of overriding is optimization (stop early when kind filter narrows). For Batch 0, **default impl is sufficient**; this task becomes a sanity test only.

- [ ] **Step 2: Run test to verify default impl correctness**

```bash
cargo test -p greentic-ext-registry --test local_registry local_list_by_kind
```

Expected: PASS using default trait impl.

- [ ] **Step 3: Repeat for store + OCI**

Add similar tests to `tests/store_registry.rs` + `tests/oci_registry.rs`. If store/OCI don't support returning kind in their metadata yet, either extend `ExtensionMetadata`/`ExtensionSummary` to carry kind, or override default impl to fetch metadata per-entry and filter (acceptable for v1; optimization deferred).

- [ ] **Step 4: Commit**

```bash
git add crates/greentic-ext-registry/
git commit -m "test(registry): sanity tests for list_by_kind across all 3 registries"
```

---

### Task A8: Add `gtdx list --kind provider` filter

**Files:**
- Modify: `crates/greentic-ext-cli/src/commands/list.rs`
- Create: `crates/greentic-ext-cli/tests/provider_commands.rs`

- [ ] **Step 1: Write failing test**

Create `crates/greentic-ext-cli/tests/provider_commands.rs`:

```rust
use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::tempdir;

#[test]
fn gtdx_list_filters_by_kind_provider() {
    let tmp = tempdir().unwrap();
    // Pre-populate ~/.greentic/extensions with fixtures — fixture helper
    setup_fixture_extensions(tmp.path());

    let mut cmd = Command::cargo_bin("gtdx").unwrap();
    cmd.env("HOME", tmp.path())
        .args(["list", "--kind", "provider"])
        .assert()
        .success()
        .stdout(predicate::str::contains("greentic.provider.telegram"))
        .stdout(predicate::str::contains("greentic.design.adaptive-cards").not());
}

fn setup_fixture_extensions(home: &std::path::Path) {
    use std::fs;
    use serde_json::json;

    let provider_dir = home.join(".greentic/extensions/provider/greentic.provider.telegram/0.1.0");
    fs::create_dir_all(&provider_dir).unwrap();
    fs::write(
        provider_dir.join("describe.json"),
        json!({
            "kind": "ProviderExtension",
            "id": "greentic.provider.telegram",
            "version": "0.1.0",
            "engine": { "greenticDesigner": "*", "extRuntime": "^0.1.0" },
            "capabilities": [],
            "runtime": {
                "gtpack": {
                    "file": "runtime/provider.gtpack",
                    "sha256": "a".repeat(64),
                    "pack_id": "greentic.provider.telegram",
                    "component_version": "0.6.0"
                }
            },
            "metadata": { "author": { "name": "Greentic" } }
        }).to_string(),
    ).unwrap();

    let design_dir = home.join(".greentic/extensions/design/greentic.design.adaptive-cards/0.1.0");
    fs::create_dir_all(&design_dir).unwrap();
    fs::write(
        design_dir.join("describe.json"),
        json!({
            "kind": "DesignExtension",
            "id": "greentic.design.adaptive-cards",
            "version": "0.1.0",
            "engine": { "greenticDesigner": "*", "extRuntime": "^0.1.0" },
            "capabilities": [],
            "metadata": { "author": { "name": "Greentic" } }
        }).to_string(),
    ).unwrap();
}
```

Tests helper: if assert_cmd not yet in dev-deps, add to `crates/greentic-ext-cli/Cargo.toml`:

```toml
[dev-dependencies]
assert_cmd = { workspace = true }
predicates = { workspace = true }
tempfile = { workspace = true }
```

Implement `setup_fixture_extensions` by creating `~/.greentic/extensions/provider/greentic.provider.telegram/0.1.0/describe.json` and `~/.greentic/extensions/design/greentic.design.adaptive-cards/0.1.0/describe.json` with the right JSON blobs.

- [ ] **Step 2: Run to verify failure**

```bash
cargo test -p greentic-ext-cli --test provider_commands
```

Expected: FAIL — `--kind provider` not recognized.

- [ ] **Step 3: Read existing list.rs and extend clap enum**

In `crates/greentic-ext-cli/src/commands/list.rs`:

```rust
use greentic_extension_sdk_contract::ExtensionKind;

#[derive(clap::ValueEnum, Clone, Debug)]
pub enum KindArg {
    Design,
    Bundle,
    Deploy,
    Provider,    // NEW
    All,
}

impl KindArg {
    pub fn to_extension_kind(&self) -> Option<ExtensionKind> {
        match self {
            Self::Design => Some(ExtensionKind::Design),
            Self::Bundle => Some(ExtensionKind::Bundle),
            Self::Deploy => Some(ExtensionKind::Deploy),
            Self::Provider => Some(ExtensionKind::Provider),
            Self::All => None,
        }
    }
}

// In list handler:
let summaries = match args.kind.to_extension_kind() {
    Some(k) => registry.list_by_kind(k).await?,
    None => registry.search(SearchQuery::default()).await?,
};
```

Ensure output formatting shows kind for disambiguation.

- [ ] **Step 4: Run test to verify pass**

```bash
cargo test -p greentic-ext-cli --test provider_commands
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/greentic-ext-cli/
git commit -m "feat(cli): gtdx list --kind provider filter"
```

---

### Task A9: Extend `gtdx info` for provider capability display

**Files:**
- Modify: `crates/greentic-ext-cli/src/commands/info.rs`
- Modify: `crates/greentic-ext-cli/tests/provider_commands.rs`

- [ ] **Step 1: Write failing test**

Append to `tests/provider_commands.rs`:

```rust
#[test]
fn gtdx_info_displays_provider_channels() {
    let tmp = tempdir().unwrap();
    setup_fixture_provider_with_channels(tmp.path(), "greentic.provider.telegram");

    let mut cmd = Command::cargo_bin("gtdx").unwrap();
    cmd.env("HOME", tmp.path())
        .args(["info", "greentic.provider.telegram"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Kind: ProviderExtension"))
        .stdout(predicate::str::contains("Capabilities: messaging"))
        .stdout(predicate::str::contains("Runtime pack: greentic.provider.telegram"));
}
```

- [ ] **Step 2: Run to verify failure**

```bash
cargo test -p greentic-ext-cli --test provider_commands gtdx_info_displays_provider_channels
```

Expected: FAIL — provider-specific fields not rendered.

- [ ] **Step 3: Extend info.rs**

In `crates/greentic-ext-cli/src/commands/info.rs`, after reading `describe`, add provider-specific rendering:

```rust
println!("Kind: {}", format_kind(&describe.kind));
// ... existing prints ...

if describe.kind == ExtensionKind::Provider {
    if let Some(rt) = &describe.runtime {
        println!("Runtime pack: {}", rt.gtpack.pack_id);
        println!("Component version: {}", rt.gtpack.component_version);
    }
    if !describe.capabilities.is_empty() {
        let caps: Vec<String> = describe
            .capabilities
            .iter()
            .map(|c| c.id.to_string())
            .collect();
        println!("Capabilities: {}", caps.join(", "));
    }
    // Defer list-channels invocation to a separate subcommand (gtdx invoke ... list-channels)
    // to keep info simple and avoid wasmtime bootstrap here.
}
```

- [ ] **Step 4: Run test to verify pass**

```bash
cargo test -p greentic-ext-cli --test provider_commands
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/greentic-ext-cli/
git commit -m "feat(cli): gtdx info renders provider runtime + capabilities"
```

---

### Task A10: Install routing for `kind=provider` via `gtdx install`

**Files:**
- Modify: `crates/greentic-ext-cli/src/commands/install.rs`
- Modify: `crates/greentic-ext-cli/tests/provider_commands.rs`

- [ ] **Step 1: Write failing end-to-end install test**

Append to `tests/provider_commands.rs`:

```rust
#[test]
fn gtdx_install_provider_from_gtxpack_places_files() {
    let tmp = tempdir().unwrap();
    let home = tmp.path().join("home");
    std::fs::create_dir_all(&home).unwrap();

    // Build a fixture .gtxpack using the helper from registry support
    let gtxpack = build_fixture_gtxpack(tmp.path(), "greentic.provider.fixture", "0.1.0");

    let mut cmd = Command::cargo_bin("gtdx").unwrap();
    cmd.env("HOME", &home)
        .env("GREENTIC_EXT_ALLOW_UNSIGNED", "1")
        .args(["install", gtxpack.to_str().unwrap(), "-y"])
        .assert()
        .success();

    assert!(home
        .join(".greentic/runtime/packs/providers/gtdx/greentic.provider.fixture-0.1.0.gtpack")
        .exists());
    assert!(home
        .join(".greentic/extensions/provider/greentic.provider.fixture/0.1.0/describe.json")
        .exists());
}
```

`build_fixture_gtxpack` is the same helper as Task A6. Since tests across different crates can't share `tests/support/mod.rs`, promote the helper to `greentic-extension-sdk-testing` crate (existing workspace member). Concretely: move `build_provider_fixture_gtxpack` + `encode_gtpack_with_pack_id` from `greentic-ext-registry/tests/support/mod.rs` to a new public module `greentic-extension-sdk-testing::fixtures::gtxpack`. Add `greentic-extension-sdk-testing = { path = "../greentic-extension-sdk-testing" }` to `greentic-ext-registry/Cargo.toml` dev-dependencies and `greentic-ext-cli/Cargo.toml` dev-dependencies. Tests call `greentic_extension_sdk_testing::fixtures::gtxpack::build_provider_fixture_gtxpack(...)`.

- [ ] **Step 2: Run to verify failure**

```bash
cargo test -p greentic-ext-cli --test provider_commands gtdx_install_provider
```

Expected: FAIL — install handler doesn't branch on kind, or extraction paths are wrong.

- [ ] **Step 3: Wire install.rs to lifecycle::install_from_path**

Read current `crates/greentic-ext-cli/src/commands/install.rs`. Replace/augment install logic to call `greentic_ext_registry::lifecycle::install_from_path` — which already handles kind dispatching (added in Task A6).

If install.rs already delegates entirely to lifecycle, this task is a no-op verification.

- [ ] **Step 4: Run test to verify pass**

```bash
cargo test -p greentic-ext-cli --test provider_commands
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/greentic-ext-cli/
git commit -m "feat(cli): gtdx install routes kind=provider to lifecycle::install_provider"
```

---

### Task A11: Bump workspace crate versions + update changelog

**Files:**
- Modify: `Cargo.toml` (workspace root)
- Modify: `crates/greentic-extension-sdk-contract/Cargo.toml`
- Modify: `crates/greentic-ext-registry/Cargo.toml`
- Modify: `crates/greentic-ext-cli/Cargo.toml`
- Modify: `CHANGELOG.md`

- [ ] **Step 1: Bump versions**

In workspace `Cargo.toml` (or individual crate Cargo.toml files if not workspace-managed), bump:
- `greentic-extension-sdk-contract`: `0.1.x` → `0.2.0`
- `greentic-ext-registry`: `0.1.x` → `0.2.0`
- `greentic-ext-cli`: `0.1.x` → `0.2.0`
- `greentic-ext-runtime`: no bump (no code changes yet)
- `greentic-extension-sdk-testing`: no bump

Root package version (if any): bump to `0.7.0`.

Ensure inter-crate deps reference `= "0.2"` or `path = "..."` as currently used — keep consistent.

- [ ] **Step 2: Update CHANGELOG**

Prepend to `CHANGELOG.md`:

```markdown
## [0.7.0] - 2026-04-19

### Added
- `ExtensionKind::Provider` variant for messaging and event provider extensions
- `greentic:extension-provider@0.1.0` WIT package with 3 sub-interfaces (messaging, event-source, event-sink)
- `describe.json` `runtime.gtpack` field (required when `kind=ProviderExtension`) with sha256 integrity check
- Lifecycle `install_provider` path: sha256 verification, manual-pack conflict detection, extraction to `~/.greentic/runtime/packs/providers/gtdx/`
- `ExtensionRegistry::list_by_kind` + `get_describe` trait methods (default impls provided)
- `gtdx list --kind provider` filter
- `gtdx info <id>` renders provider-specific fields (runtime pack, capabilities)

### Changed
- Bump `greentic-extension-sdk-contract`, `-registry`, `-cli` to 0.2.0 (additive — existing kinds unaffected)

### Notes
- Runtime path unchanged: `greentic-runner` picks up extracted `.gtpack` via existing pack-index polling. Extension system is purely additive.
```

- [ ] **Step 3: Run full local check**

```bash
ci/local_check.sh
```

Expected: green (fmt + clippy + all tests).

- [ ] **Step 4: Commit**

```bash
git add Cargo.toml crates/*/Cargo.toml CHANGELOG.md
git commit -m "chore: bump designer-extensions to 0.7.0 for ExtensionKind::Provider"
```

---

### Task A12: Author `how-to-write-a-provider-extension.md`

**Files:**
- Create: `docs/how-to-write-a-provider-extension.md`
- Modify: `README.md` (add link)

- [ ] **Step 1: Write the tutorial**

Create `docs/how-to-write-a-provider-extension.md` with sections:

```markdown
# How to Write a Provider Extension

> Companion to the design spec: [`superpowers/specs/2026-04-19-provider-extension-design.md`](superpowers/specs/2026-04-19-provider-extension-design.md).

Provider extensions expose messaging providers (Slack, Telegram, Email, …) and
event providers (cron, webhook triggers, …) as installable, signed, discoverable
WASM components. This guide walks through authoring one end-to-end using
Telegram as the reference pilot.

## Prerequisites
- Rust 1.94+ with `wasm32-wasip1` target: `rustup target add wasm32-wasip1`
- `cargo-component` 0.21.1: `cargo install cargo-component --version 0.21.1`
- `gtdx` CLI on PATH (this repo's `greentic-ext-cli`)
- `wit-bindgen` 0.41 (transitive via `cargo-component`)

## Anatomy of a Provider Extension

Each provider extension consists of two artifacts shipped together inside one `.gtxpack`:

1. **Extension WASM component** (`wasm32-wasip1` cdylib) — metadata queries (channels, schemas, i18n). Runs in `gtdx` / designer / wizards.
2. **Runtime pack** (`.gtpack`, `wasm32-wasip2`) — actual message-send or event-emit runtime. Runs in `greentic-runner`.

The extension carries the runtime pack embedded inside it.

## Step 1 — Vendor the WIT contract

Copy from this repo into yours:
```bash
cp /path/to/greentic-designer-extensions/wit/extension-base.wit wit/
cp /path/to/greentic-designer-extensions/wit/extension-host.wit wit/
cp /path/to/greentic-designer-extensions/wit/extension-provider.wit wit/
```

Document contract version in your repo's `README.md`.

## Step 2 — Scaffold the extension crate

[Concrete `Cargo.toml` + `src/lib.rs` boilerplate — same as Telegram pilot produced in Batch 0]

## Step 3 — Author `describe.json`

[Sample with runtime.gtpack field — include sha256 computation step]

## Step 4 — Write schemas and i18n

[JSON Schema structure, i18n JSON layout]

## Step 5 — Build script

[build.sh from Telegram pilot]

## Step 6 — Sign and publish

[Point to gtdx sign / verify tooling]

## Step 7 — Install and test

```bash
gtdx install ./greentic.provider.my-provider-0.1.0.gtxpack
gtdx list --kind provider
gtdx info greentic.provider.my-provider
```

## Reference: Telegram pilot

Link to `greentic-messaging-providers/crates/provider-telegram-extension/`.

## Common pitfalls

- **`greenticDesigner: "*"`** must be present in describe.json `engine` field even if your provider is not designer-specific (Engine struct uses `deny_unknown_fields` and treats both fields as required).
- **Zip filename divergence** (Ubuntu InfoZip vs Python zip wrapper) — build.sh must normalize.
- **WIT vendor drift** — re-vendor WIT + rebuild when contract bumps.
- **`[workspace]` empty table** required in extension crate Cargo.toml; workspace root needs `exclude = ["crates/provider-*-extension"]`.
```

Keep doc implementation-ready — specific commands, no handwaving. Update the Telegram-specific sections after Batch 0 Task B4 builds the pilot (reference its files).

- [ ] **Step 2: Link from README**

Modify `README.md`:

```markdown
## Writing Extensions
- [Design extension guide](docs/how-to-write-a-design-extension.md)
- [Bundle extension guide](docs/how-to-write-a-bundle-extension.md)
- [Deploy extension guide](docs/how-to-write-a-deploy-extension.md)
- [Provider extension guide](docs/how-to-write-a-provider-extension.md) — NEW
```

- [ ] **Step 3: Commit**

```bash
git add docs/how-to-write-a-provider-extension.md README.md
git commit -m "docs: add provider extension authoring guide"
```

---

### Task A13: Open PR #1 — Wave A infrastructure

- [ ] **Step 1: Push branch and open PR**

```bash
git push -u origin spec/provider-extension-system
gh pr create --title "feat: provider extension infrastructure (Wave A)" --body "$(cat <<'EOF'
## Summary

Pre-flight infrastructure for the `ExtensionKind::Provider` — the fourth extension kind. Zero-runtime-change additive layer that wraps `.gtpack` runtime artifacts inside `.gtxpack` extensions for discovery, signing, and dynamic selection in designer/bundle/deploy wizards.

Spec: `docs/superpowers/specs/2026-04-19-provider-extension-design.md`
Plan: `docs/superpowers/plans/2026-04-19-provider-extension-batch-0.md`

## What landed
- `ExtensionKind::Provider` variant (contract 0.2.0)
- `greentic:extension-provider@0.1.0` WIT package with 3 sub-interfaces (messaging, event-source, event-sink)
- `describe.json` `runtime.gtpack` field with sha256 + pack_id + component_version
- Lifecycle `install_provider` path: sha256 verify, manual-pack conflict detection, extraction to `~/.greentic/runtime/packs/providers/gtdx/`
- `list_by_kind` + `get_describe` on `ExtensionRegistry` trait (default impls; concrete overrides TBD Batch 2)
- `gtdx list --kind provider` + `gtdx info` provider rendering
- Provider authoring guide

## What's next
- Batch 0 Wave B (Telegram pilot) in `greentic-messaging-providers` — separate PR after this merges
- Wave C E2E scenario in `greentic-e2e`

## Test plan
- [ ] `cargo test --workspace --all-features` green
- [ ] `cargo clippy --workspace --all-targets --all-features -- -D warnings` green
- [ ] `cargo fmt --all -- --check` green
- [ ] `ci/local_check.sh` green
- [ ] Install fixture `.gtxpack` via `gtdx install` → files land in expected directories
- [ ] Uninstall cleans up
EOF
)"
```

- [ ] **Step 2: Record PR merge SHA for Wave B rev pin**

After PR merge, capture the merge SHA (for Wave B's cargo dep `git+rev=` pin):

```bash
git checkout main && git pull
git rev-parse HEAD  # save this SHA for Wave B repo
```

Update the `spec/provider-extension-system` branch or retain deletion per repo convention.

---

## Wave B — Telegram Pilot (`greentic-messaging-providers`)

**Assumes Wave A PR merged in `greentic-designer-extensions` and merge SHA captured for git rev pin.**

**Repo switch:** `cd /home/bimbim/works/greentic/greentic-messaging-providers`

**Branch:** create `feat/provider-extension-telegram-pilot`

### Task B1: Vendor WIT contracts + workspace isolation

**Files:**
- Create: `wit/extension-base.wit`
- Create: `wit/extension-host.wit`
- Create: `wit/extension-provider.wit`
- Create: `wit/deps/` (with any transitive deps if cargo-component requires)
- Modify: `Cargo.toml` (workspace — add `exclude`)
- Modify: `README.md`

- [ ] **Step 1: Copy WIT files from designer-extensions**

```bash
DESIGNER_EXT=/home/bimbim/works/greentic/greentic-designer-extensions
mkdir -p wit
cp $DESIGNER_EXT/wit/extension-base.wit wit/
cp $DESIGNER_EXT/wit/extension-host.wit wit/
cp $DESIGNER_EXT/wit/extension-provider.wit wit/
```

- [ ] **Step 2: Check transitive WIT deps**

```bash
wasm-tools component wit wit/ 2>&1 | head -20
```

If errors about missing WASI interfaces, check how `greentic-adaptive-card-mcp/wit/` handles it — likely needs `wit/deps/wasi-*/*.wit` subdirectories.

- [ ] **Step 3: Workspace exclude**

Modify root `Cargo.toml`:

```toml
[workspace]
exclude = [
    # ... existing excludes
    "crates/provider-*-extension",
]
```

- [ ] **Step 4: README note**

Add to `README.md`:

```markdown
## Vendored WIT Contracts

`wit/extension-*.wit` files are vendored copies from
[`greentic-designer-extensions`](https://github.com/greentic-biz/greentic-designer-extensions).
When the upstream contract bumps, re-vendor:

```bash
DESIGNER_EXT=/path/to/greentic-designer-extensions
cp $DESIGNER_EXT/wit/extension-{base,host,provider}.wit wit/
```

Current vendor commit: `<record-sha-here-on-first-vendor>`.
```

- [ ] **Step 5: Commit**

```bash
git add wit/ Cargo.toml README.md
git commit -m "chore: vendor extension-{base,host,provider} WIT contracts"
```

---

### Task B2: Scaffold `provider-telegram-extension` crate

**Files:**
- Create: `crates/provider-telegram-extension/Cargo.toml`
- Create: `crates/provider-telegram-extension/src/lib.rs` (stub)

- [ ] **Step 1: Create Cargo.toml**

Create `crates/provider-telegram-extension/Cargo.toml`:

```toml
[package]
name = "provider-telegram-extension"
version = "0.1.0"
edition = "2024"
rust-version = "1.94.0"
publish = false

[lib]
crate-type = ["cdylib"]

[dependencies]
wit-bindgen = "0.41"
serde = { version = "1", features = ["derive"] }
serde_json = "1"

[package.metadata.component]
package = "greentic:provider-telegram"

[package.metadata.component.target]
path = "../../wit"
world = "messaging-only-provider"

[workspace]
# Empty table — cdylib needs isolation from workspace root
```

- [ ] **Step 2: Stub lib.rs with WIT guest bindings**

Create `crates/provider-telegram-extension/src/lib.rs`:

```rust
#![deny(clippy::all)]

wit_bindgen::generate!({
    path: "../../wit",
    world: "messaging-only-provider",
});

use exports::greentic::extension_provider::messaging::{
    CardTier, ChannelProfile, Direction, Error, Guest as MessagingGuest,
};
use exports::greentic::extension_base::base::{Describe, Guest as BaseGuest};

struct Component;

impl BaseGuest for Component {
    fn describe() -> Describe {
        Describe {
            id: "greentic.provider.telegram".into(),
            version: "0.1.0".into(),
            kind: "ProviderExtension".into(),
        }
    }
}

impl MessagingGuest for Component {
    fn list_channels() -> Vec<ChannelProfile> {
        vec![ChannelProfile {
            id: "direct-message".into(),
            display_name: "Direct Message".into(),
            direction: Direction::Bidirectional,
            tier_support: vec![CardTier::TierDTextOnly],
            metadata: vec![],
        }]
    }

    fn describe_channel(id: String) -> Result<ChannelProfile, Error> {
        if id == "direct-message" {
            Ok(Self::list_channels().into_iter().next().unwrap())
        } else {
            Err(Error::NotFound(id))
        }
    }

    fn secret_schema(id: String) -> Result<String, Error> {
        match id.as_str() {
            "direct-message" => Ok(include_str!("../schemas/direct-message-secret.schema.json").into()),
            other => Err(Error::NotFound(other.into())),
        }
    }

    fn config_schema(id: String) -> Result<String, Error> {
        match id.as_str() {
            "direct-message" => Ok(include_str!("../schemas/direct-message-config.schema.json").into()),
            other => Err(Error::NotFound(other.into())),
        }
    }

    fn dry_run_encode(_id: String, _sample: Vec<u8>) -> Result<Vec<u8>, Error> {
        Err(Error::Internal("dry-run not supported for Telegram v1".into()))
    }
}

export!(Component);
```

- [ ] **Step 3: Verify cargo component builds the stub**

```bash
cd crates/provider-telegram-extension
cargo component build --release
```

Expected: produces `target/wasm32-wasip1/release/provider_telegram_extension.wasm`. The exact WIT binding function names may differ slightly (`Guest` trait members, etc.) — adjust code to match what cargo-component generates. If names mismatch, read the generated bindings via `cargo component build -v 2>&1 | grep -A 2 'Generated'`.

- [ ] **Step 4: Commit**

```bash
cd ../..   # back to repo root
git add crates/provider-telegram-extension/Cargo.toml crates/provider-telegram-extension/src/lib.rs
git commit -m "feat(provider-telegram-ext): scaffold extension crate with WIT bindings"
```

---

### Task B3: Author `describe.json`, schemas, and i18n bundles

**Files:**
- Create: `crates/provider-telegram-extension/describe.json`
- Create: `crates/provider-telegram-extension/schemas/direct-message-secret.schema.json`
- Create: `crates/provider-telegram-extension/schemas/direct-message-config.schema.json`
- Create: `crates/provider-telegram-extension/i18n/en.json` + 5 others

- [ ] **Step 1: describe.json template**

Create `crates/provider-telegram-extension/describe.json`:

```json
{
  "kind": "ProviderExtension",
  "id": "greentic.provider.telegram",
  "version": "0.1.0",
  "engine": {
    "greenticDesigner": "*",
    "extRuntime": "^0.1.0"
  },
  "capabilities": [],
  "runtime": {
    "gtpack": {
      "file": "runtime/provider.gtpack",
      "sha256": "0000000000000000000000000000000000000000000000000000000000000000",
      "pack_id": "greentic.provider.telegram",
      "component_version": "0.6.0"
    }
  },
  "metadata": {
    "author": {
      "name": "Greentic",
      "publicKey": "S6cnfmdoj3wKx3cxPsNsP8fErvK4S12a5AtzTExPrvQ="
    },
    "description": "Telegram Bot API messaging provider",
    "tags": ["messaging", "telegram", "chat"]
  }
}
```

The `sha256` placeholder is rewritten by `build.sh` after embedding the actual `.gtpack` (Task B5).

- [ ] **Step 2: Secret schema (Bot token + webhook secret)**

Create `schemas/direct-message-secret.schema.json`:

```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "title": "Telegram Direct Message secrets",
  "type": "object",
  "properties": {
    "bot_token": {
      "type": "string",
      "pattern": "^[0-9]+:[A-Za-z0-9_-]+$",
      "description": "Telegram bot token from @BotFather",
      "writeOnly": true
    },
    "webhook_secret": {
      "type": "string",
      "minLength": 16,
      "description": "Secret for verifying webhook calls from Telegram",
      "writeOnly": true
    }
  },
  "required": ["bot_token", "webhook_secret"],
  "additionalProperties": false
}
```

- [ ] **Step 3: Config schema (non-sensitive)**

Create `schemas/direct-message-config.schema.json`:

```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "title": "Telegram Direct Message configuration",
  "type": "object",
  "properties": {
    "webhook_path": {
      "type": "string",
      "default": "/telegram/webhook",
      "description": "Path prefix for Telegram webhook endpoint"
    },
    "rate_limit_per_second": {
      "type": "integer",
      "minimum": 1,
      "maximum": 30,
      "default": 29,
      "description": "Telegram API rate limit (max 30/sec)"
    }
  },
  "additionalProperties": false
}
```

- [ ] **Step 4: i18n bundles**

Create `i18n/en.json`:

```json
{
  "provider.telegram.displayName": "Telegram",
  "provider.telegram.channel.direct-message.displayName": "Direct Message",
  "provider.telegram.secret.bot_token.label": "Bot Token",
  "provider.telegram.secret.bot_token.help": "Get from @BotFather on Telegram",
  "provider.telegram.secret.webhook_secret.label": "Webhook Secret",
  "provider.telegram.secret.webhook_secret.help": "16+ character secret Telegram will echo back for verification",
  "provider.telegram.config.webhook_path.label": "Webhook Path",
  "provider.telegram.config.rate_limit_per_second.label": "Rate Limit (per second)"
}
```

Create `i18n/id.json` (Bahasa Indonesia):

```json
{
  "provider.telegram.displayName": "Telegram",
  "provider.telegram.channel.direct-message.displayName": "Pesan Langsung",
  "provider.telegram.secret.bot_token.label": "Token Bot",
  "provider.telegram.secret.bot_token.help": "Dapatkan dari @BotFather di Telegram",
  "provider.telegram.secret.webhook_secret.label": "Rahasia Webhook",
  "provider.telegram.secret.webhook_secret.help": "Rahasia 16+ karakter untuk diverifikasi Telegram",
  "provider.telegram.config.webhook_path.label": "Path Webhook",
  "provider.telegram.config.rate_limit_per_second.label": "Batas Laju (per detik)"
}
```

Create `i18n/ja.json`, `zh.json`, `es.json`, `de.json` with matching keys translated. Use existing provider-telegram i18n coverage as source for consistency (copy keys if keys exist there; translate if new).

- [ ] **Step 5: Commit**

```bash
git add crates/provider-telegram-extension/describe.json crates/provider-telegram-extension/schemas/ crates/provider-telegram-extension/i18n/
git commit -m "feat(provider-telegram-ext): author describe.json + schemas + i18n (6 locales)"
```

---

### Task B4: Implement `build.sh` — produces signed `.gtxpack`

**Files:**
- Create: `crates/provider-telegram-extension/build.sh`

- [ ] **Step 1: Write build.sh**

Create `crates/provider-telegram-extension/build.sh`:

```bash
#!/usr/bin/env bash
set -euo pipefail

# Build Telegram provider extension .gtxpack

HERE="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$HERE/../.." && pwd)"
VERSION="$(grep -m1 '^version' "$HERE/Cargo.toml" | cut -d'"' -f2)"
OUT="$REPO_ROOT/greentic.provider.telegram-${VERSION}.gtxpack"

echo "==> Building Telegram extension .gtxpack v$VERSION"

# 1. Build the extension WASM component
echo "==> Building wasm32-wasip1 extension component"
cd "$HERE"
cargo component build --release
EXT_WASM="$HERE/target/wasm32-wasip1/release/provider_telegram_extension.wasm"
test -f "$EXT_WASM" || { echo "ERROR: ext wasm not found at $EXT_WASM"; exit 1; }

# 2. Locate runtime .gtpack (built separately by existing provider-telegram flow)
RUNTIME_GTPACK="${PROVIDER_TELEGRAM_GTPACK:-$REPO_ROOT/dist/packs/provider-telegram.gtpack}"
test -f "$RUNTIME_GTPACK" || {
  echo "ERROR: runtime .gtpack not found at $RUNTIME_GTPACK"
  echo "Set PROVIDER_TELEGRAM_GTPACK env var or build runtime pack first:"
  echo "  cd $REPO_ROOT && ./tools/build_components.sh && make pack-telegram"
  exit 1
}

# 3. Stage content
STAGING="$(mktemp -d)"
trap 'rm -rf "$STAGING"' EXIT

mkdir -p "$STAGING/wasm" "$STAGING/runtime" "$STAGING/schemas" "$STAGING/i18n"
cp "$EXT_WASM" "$STAGING/wasm/provider_telegram_ext.wasm"
cp "$RUNTIME_GTPACK" "$STAGING/runtime/provider.gtpack"
cp "$HERE/schemas/"*.json "$STAGING/schemas/"
cp "$HERE/i18n/"*.json "$STAGING/i18n/"

# 4. Compute sha256 of embedded gtpack + rewrite describe.json
GTPACK_SHA="$(sha256sum "$STAGING/runtime/provider.gtpack" | awk '{print $1}')"
python3 -c "
import json, sys
d = json.load(open('$HERE/describe.json'))
d['runtime']['gtpack']['sha256'] = '$GTPACK_SHA'
json.dump(d, open('$STAGING/describe.json', 'w'), indent=2, sort_keys=True)
"

# 5. Sign describe.json if signing key available (CI) or skip (dev)
if [ -n "${EXT_SIGNING_KEY_PEM:-}" ]; then
  echo "==> Signing describe.json"
  KEY_TMP="$(mktemp)"
  trap 'rm -f "$KEY_TMP"; rm -rf "$STAGING"' EXIT
  echo "$EXT_SIGNING_KEY_PEM" > "$KEY_TMP"
  gtdx sign "$STAGING/describe.json" --key "$KEY_TMP" --in-place
else
  echo "==> Skipping signature (EXT_SIGNING_KEY_PEM not set) — dev build"
fi

# 6. Zip into .gtxpack — robust against Ubuntu InfoZip vs Python zip wrapper
echo "==> Creating $OUT"
cd "$STAGING"
rm -f "$OUT"
zip -r "$OUT" . > /dev/null

# Handle Python zip wrapper appending .zip
if [ ! -f "$OUT" ] && [ -f "${OUT}.zip" ]; then
  mv "${OUT}.zip" "$OUT"
fi

test -f "$OUT" || { echo "ERROR: $OUT not produced"; exit 1; }
echo "==> Built: $OUT ($(du -h "$OUT" | awk '{print $1}'))"

# 7. Size guard (spec §8.1: ≤ 5 MB)
SIZE=$(stat -c %s "$OUT" 2>/dev/null || stat -f %z "$OUT")
MAX=$((5 * 1024 * 1024))
if [ "$SIZE" -gt "$MAX" ]; then
  echo "ERROR: .gtxpack exceeds 5 MB limit ($SIZE bytes)"
  exit 1
fi
```

- [ ] **Step 2: Make executable + smoke run**

```bash
chmod +x crates/provider-telegram-extension/build.sh
./crates/provider-telegram-extension/build.sh || echo "(expected failure if runtime pack missing)"
```

If runtime .gtpack missing, temporarily create a placeholder to test the zip path:

```bash
mkdir -p dist/packs
echo "placeholder-gtpack-bytes" > dist/packs/provider-telegram.gtpack
./crates/provider-telegram-extension/build.sh
```

Expected: produces `greentic.provider.telegram-0.1.0.gtxpack` at repo root, size well under 5 MB.

- [ ] **Step 3: Clean placeholder**

```bash
rm -f dist/packs/provider-telegram.gtpack greentic.provider.telegram-0.1.0.gtxpack
```

- [ ] **Step 4: Commit**

```bash
git add crates/provider-telegram-extension/build.sh
git commit -m "feat(provider-telegram-ext): build.sh produces signed .gtxpack with sha256 integrity"
```

---

### Task B5: Schema validation tests (rlib sibling crate)

**Files:**
- Create: `crates/provider-telegram-extension-tests/Cargo.toml`
- Create: `crates/provider-telegram-extension-tests/src/lib.rs`
- Create: `crates/provider-telegram-extension-tests/tests/schemas.rs`
- Create: `crates/provider-telegram-extension-tests/tests/describe.rs`

cdylib + wit-bindgen prevents tests in the extension crate itself from linking. Sibling rlib workaround (same pattern as `bundle-standard-schema-tests`).

- [ ] **Step 1: Create sibling crate**

`crates/provider-telegram-extension-tests/Cargo.toml`:

```toml
[package]
name = "provider-telegram-extension-tests"
version = "0.0.0"
edition = "2024"
publish = false

[lib]
path = "src/lib.rs"

[dependencies]
serde_json = "1"

[dev-dependencies]
jsonschema = "0.18"

[workspace]
```

`src/lib.rs`:

```rust
pub fn ext_dir() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("provider-telegram-extension")
}
```

- [ ] **Step 2: Write schema validation test**

`tests/schemas.rs`:

```rust
use jsonschema::JSONSchema;
use provider_telegram_extension_tests::ext_dir;

#[test]
fn secret_schema_validates_sample_input() {
    let schema_path = ext_dir().join("schemas/direct-message-secret.schema.json");
    let schema: serde_json::Value =
        serde_json::from_reader(std::fs::File::open(schema_path).unwrap()).unwrap();
    let compiled = JSONSchema::compile(&schema).unwrap();

    let valid = serde_json::json!({
        "bot_token": "123456789:ABCdefGHIjklMNOpqrsTUVwxyz",
        "webhook_secret": "at_least_16_chars_value"
    });
    assert!(compiled.is_valid(&valid));

    let invalid_short_secret = serde_json::json!({
        "bot_token": "123456789:ABCdefGHIjklMNOpqrsTUVwxyz",
        "webhook_secret": "short"
    });
    assert!(!compiled.is_valid(&invalid_short_secret));

    let invalid_bad_token = serde_json::json!({
        "bot_token": "not-a-valid-format",
        "webhook_secret": "at_least_16_chars_value"
    });
    assert!(!compiled.is_valid(&invalid_bad_token));
}

#[test]
fn config_schema_validates_defaults() {
    let schema_path = ext_dir().join("schemas/direct-message-config.schema.json");
    let schema: serde_json::Value =
        serde_json::from_reader(std::fs::File::open(schema_path).unwrap()).unwrap();
    let compiled = JSONSchema::compile(&schema).unwrap();

    let valid = serde_json::json!({ "webhook_path": "/tg", "rate_limit_per_second": 20 });
    assert!(compiled.is_valid(&valid));

    let out_of_range = serde_json::json!({ "rate_limit_per_second": 100 });
    assert!(!compiled.is_valid(&out_of_range));
}
```

- [ ] **Step 3: Describe.json validation test**

`tests/describe.rs`:

```rust
use provider_telegram_extension_tests::ext_dir;

#[test]
fn describe_json_has_required_provider_fields() {
    let path = ext_dir().join("describe.json");
    let d: serde_json::Value =
        serde_json::from_reader(std::fs::File::open(path).unwrap()).unwrap();
    assert_eq!(d["kind"], "ProviderExtension");
    assert_eq!(d["id"], "greentic.provider.telegram");
    assert!(d["runtime"]["gtpack"]["file"].as_str().is_some());
    assert_eq!(d["runtime"]["gtpack"]["pack_id"], "greentic.provider.telegram");
    assert_eq!(d["runtime"]["gtpack"]["component_version"], "0.6.0");
    assert_eq!(d["engine"]["greenticDesigner"], "*");
}
```

- [ ] **Step 4: Run tests**

```bash
cargo test -p provider-telegram-extension-tests
```

Expected: PASS on all 3 tests.

- [ ] **Step 5: Commit**

```bash
git add crates/provider-telegram-extension-tests/
git commit -m "test(provider-telegram-ext): schema + describe.json validation (rlib sibling)"
```

---

### Task B6: Integration test — `.gtxpack` round-trip

**Files:**
- Create: `tests/provider_telegram_gtxpack.rs` (at repo root, as workspace integration test)

- [ ] **Step 1: Write test**

Create `tests/provider_telegram_gtxpack.rs`:

```rust
//! Integration test: drive build.sh programmatically, verify .gtxpack shape.

use std::process::Command;

#[test]
#[ignore = "requires runtime .gtpack — run via ci/steps/30_provider_extensions.sh"]
fn build_sh_produces_valid_gtxpack() {
    let repo = env!("CARGO_MANIFEST_DIR");

    // Create placeholder runtime gtpack
    let dist = format!("{repo}/dist/packs");
    std::fs::create_dir_all(&dist).unwrap();
    let placeholder = format!("{dist}/provider-telegram.gtpack");
    std::fs::write(&placeholder, b"placeholder-runtime-bytes").unwrap();

    // Invoke build.sh
    let status = Command::new("bash")
        .arg("crates/provider-telegram-extension/build.sh")
        .current_dir(repo)
        .status()
        .unwrap();
    assert!(status.success(), "build.sh failed");

    // Find the output .gtxpack
    let out = format!("{repo}/greentic.provider.telegram-0.1.0.gtxpack");
    assert!(std::path::Path::new(&out).exists(), ".gtxpack not produced");

    // Verify ZIP structure
    let file = std::fs::File::open(&out).unwrap();
    let mut zip = zip::ZipArchive::new(file).unwrap();
    let names: Vec<String> = (0..zip.len())
        .map(|i| zip.by_index(i).unwrap().name().to_string())
        .collect();
    assert!(names.iter().any(|n| n == "describe.json"));
    assert!(names.iter().any(|n| n == "runtime/provider.gtpack"));
    assert!(names.iter().any(|n| n.starts_with("wasm/")));
    assert!(names.iter().any(|n| n.starts_with("schemas/")));
    assert!(names.iter().any(|n| n.starts_with("i18n/")));

    // Cleanup
    std::fs::remove_file(&out).ok();
    std::fs::remove_file(&placeholder).ok();
}
```

- [ ] **Step 2: Add to workspace Cargo.toml if needed**

Ensure root `Cargo.toml` test discovery finds this file (most likely automatic for `tests/` at root). Add `[[test]] name = "provider_telegram_gtxpack"` if needed.

- [ ] **Step 3: Run**

```bash
cargo test --test provider_telegram_gtxpack -- --include-ignored
```

Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add tests/provider_telegram_gtxpack.rs Cargo.toml
git commit -m "test: integration test for Telegram .gtxpack round-trip"
```

---

### Task B7: CI step — build + sign + size guard

**Files:**
- Create: `ci/steps/30_provider_extensions.sh`
- Modify: `ci/local_check.sh`
- Modify: `.github/workflows/release.yml` (or existing CI workflow)

- [ ] **Step 1: Create CI step script**

Create `ci/steps/30_provider_extensions.sh`:

```bash
#!/usr/bin/env bash
set -euo pipefail

HERE="$(cd "$(dirname "$0")" && pwd)"
REPO="$(cd "$HERE/../.." && pwd)"

echo "==> Provider extensions CI step"

# 1. Build runtime .gtpack prerequisite (re-uses existing tooling)
if [ ! -f "$REPO/dist/packs/provider-telegram.gtpack" ]; then
  echo "==> Building provider-telegram runtime pack"
  "$REPO/tools/build_components.sh" --only telegram
  # Assume the build pipeline emits to dist/packs/provider-telegram.gtpack.
  # If path differs, adjust. Check ls dist/packs after build.
fi

# 2. Install gtdx if needed (for signing)
if ! command -v gtdx >/dev/null 2>&1; then
  echo "==> Installing gtdx from greentic-biz/greentic-designer-extensions"
  cargo install --git https://github.com/greentic-biz/greentic-designer-extensions greentic-ext-cli --bin gtdx --locked
fi

# 3. Build .gtxpack
"$REPO/crates/provider-telegram-extension/build.sh"

# 4. Verify signature (if signed)
if [ -n "${EXT_SIGNING_KEY_PEM:-}" ]; then
  gtdx verify "$REPO/greentic.provider.telegram-0.1.0.gtxpack"
fi

# 5. Guardrail: CI on main must have signed
if [ "${CI_REQUIRE_SIGNED:-0}" = "1" ]; then
  if [ -z "${EXT_SIGNING_KEY_PEM:-}" ]; then
    echo "ERROR: main CI requires signed artifacts. EXT_SIGNING_KEY_PEM not set."
    exit 1
  fi
fi

echo "==> Provider extensions CI step complete"
```

- [ ] **Step 2: Wire into local_check.sh**

Append to `ci/local_check.sh` (before the summary block if any):

```bash
echo "==> Running provider extensions build step"
bash "$HERE/steps/30_provider_extensions.sh"
```

- [ ] **Step 3: Wire into release workflow**

Modify `.github/workflows/release.yml` (or the main workflow file) to include the step on main push:

```yaml
- name: Build provider extensions
  env:
    EXT_SIGNING_KEY_PEM: ${{ secrets.EXT_SIGNING_KEY_PEM }}
    CI_REQUIRE_SIGNED: ${{ github.ref == 'refs/heads/main' && '1' || '0' }}
  run: bash ci/steps/30_provider_extensions.sh

- name: Upload .gtxpack artifact
  if: github.ref == 'refs/heads/main'
  uses: actions/upload-artifact@v4
  with:
    name: provider-telegram-gtxpack
    path: greentic.provider.telegram-*.gtxpack
```

- [ ] **Step 4: Add EXT_SIGNING_KEY_PEM to GH secrets (manual)**

Out-of-band operation: add `EXT_SIGNING_KEY_PEM` secret to `greenticai/greentic-messaging-providers` repo using the existing org keypair (same as the 3 extension repos).

Record this as a manual checklist item in the PR description.

- [ ] **Step 5: Commit**

```bash
chmod +x ci/steps/30_provider_extensions.sh
git add ci/steps/30_provider_extensions.sh ci/local_check.sh .github/workflows/
git commit -m "ci: build + verify provider extension .gtxpack in CI"
```

---

### Task B8: End-to-end smoke test — install + runner pickup

**Files:**
- Create: `tests/provider_telegram_install.rs`

- [ ] **Step 1: Write installable-and-runner-loads test**

```rust
//! E2E: build .gtxpack → gtdx install → verify runner pack-index picks it up.

use std::path::PathBuf;
use std::process::Command;
use tempfile::tempdir;

#[test]
#[ignore = "requires gtdx + runtime .gtpack"]
fn gtdx_install_telegram_gtxpack_drops_pack_into_gtdx_dir() {
    let repo = env!("CARGO_MANIFEST_DIR");
    let tmp = tempdir().unwrap();
    let home = tmp.path();

    // Precondition: build .gtxpack (assumes runtime pack already built)
    let build_status = Command::new("bash")
        .arg(format!("{repo}/crates/provider-telegram-extension/build.sh"))
        .status()
        .unwrap();
    assert!(build_status.success());

    let gtxpack = PathBuf::from(format!("{repo}/greentic.provider.telegram-0.1.0.gtxpack"));
    assert!(gtxpack.exists());

    // Install via gtdx
    let install_status = Command::new("gtdx")
        .env("HOME", home)
        .env("GREENTIC_EXT_ALLOW_UNSIGNED", "1")
        .args(["install", gtxpack.to_str().unwrap(), "-y"])
        .status()
        .unwrap();
    assert!(install_status.success(), "gtdx install failed");

    // Verify .gtpack landed in gtdx runner dir
    let runtime_pack = home
        .join(".greentic/runtime/packs/providers/gtdx/greentic.provider.telegram-0.1.0.gtpack");
    assert!(runtime_pack.exists(), "runtime pack not extracted");

    // Verify metadata dir
    let meta = home.join(".greentic/extensions/provider/greentic.provider.telegram/0.1.0");
    assert!(meta.join("describe.json").exists());

    // Uninstall
    let uninstall_status = Command::new("gtdx")
        .env("HOME", home)
        .args(["uninstall", "greentic.provider.telegram", "-y"])
        .status()
        .unwrap();
    assert!(uninstall_status.success());
    assert!(!runtime_pack.exists(), "pack should be removed on uninstall");

    // Cleanup built artifact
    std::fs::remove_file(&gtxpack).ok();
}
```

- [ ] **Step 2: Run locally with gtdx on PATH**

```bash
cargo test --test provider_telegram_install -- --include-ignored
```

Expected: PASS if gtdx + runtime pack present.

- [ ] **Step 3: Commit**

```bash
git add tests/provider_telegram_install.rs
git commit -m "test(e2e): gtdx install + uninstall Telegram .gtxpack smoke test"
```

---

### Task B9: Update pin on Wave A in workspace dependency + bump version

**Files:**
- Modify: `Cargo.toml` (workspace deps)

This task adds git deps so that future tasks (or downstream users) can pin the exact designer-extensions rev the Telegram extension targets.

- [ ] **Step 1: Add dev-dependencies pin to Wave A merge SHA**

In root `Cargo.toml` `[workspace.dependencies]` section, add (if needed for integration tests or future host work):

```toml
[workspace.dependencies]
greentic-extension-sdk-contract = { git = "https://github.com/greentic-biz/greentic-designer-extensions", rev = "<WAVE-A-MERGE-SHA>" }
greentic-ext-cli = { git = "https://github.com/greentic-biz/greentic-designer-extensions", rev = "<WAVE-A-MERGE-SHA>" }
```

Substitute `<WAVE-A-MERGE-SHA>` with the actual merge SHA from Task A13.

If no workspace crate consumes these yet, this task may be deferred to a later batch. Skip if not needed in Batch 0.

- [ ] **Step 2: Commit if changes made**

```bash
git add Cargo.toml
git commit -m "chore: pin greentic-designer-extensions to Wave A merge"
```

---

### Task B10: Open PR #2 — Telegram pilot

- [ ] **Step 1: Push and open**

```bash
git push -u origin feat/provider-extension-telegram-pilot
gh pr create --title "feat: Telegram provider extension pilot (Batch 0)" --body "$(cat <<'EOF'
## Summary

First provider extension `.gtxpack` — Telegram pilot. Validates the end-to-end pattern shipped in `greentic-designer-extensions` Wave A.

Spec: https://github.com/greentic-biz/greentic-designer-extensions/blob/main/docs/superpowers/specs/2026-04-19-provider-extension-design.md
Plan: https://github.com/greentic-biz/greentic-designer-extensions/blob/main/docs/superpowers/plans/2026-04-19-provider-extension-batch-0.md

## What landed
- `crates/provider-telegram-extension/` — WASM ext component (wasm32-wasip1 cdylib)
- `describe.json` with `runtime.gtpack` pointing at embedded pack
- Secret + config schemas (JSON Schema Draft 2020-12)
- 6-locale i18n bundles (en, id, ja, zh, es, de)
- `build.sh` produces signed `.gtxpack` with sha256 integrity + 5 MB size guard
- `ci/steps/30_provider_extensions.sh` CI step
- Sibling rlib test crate (cdylib+WIT linker workaround)
- Vendored WIT contracts in `wit/`

## Acceptance verification
- [ ] `./crates/provider-telegram-extension/build.sh` produces < 5 MB .gtxpack
- [ ] `gtdx install ./greentic.provider.telegram-0.1.0.gtxpack` extracts files correctly
- [ ] `gtdx list --kind provider` shows Telegram
- [ ] `gtdx info greentic.provider.telegram` shows runtime pack + capabilities
- [ ] `gtdx uninstall greentic.provider.telegram` cleans all artifacts
- [ ] Existing Telegram E2E still green without `--features provider-extension`

## Manual checklist before merge
- [ ] `EXT_SIGNING_KEY_PEM` secret added to repo secrets
- [ ] CI green on non-main branch
- [ ] CI signs artifact on main push

## What's next
- Batch 1 — WebChat + Email + Cron (parallel PRs)
- Wave C — greentic-e2e script for nightly testing
EOF
)"
```

- [ ] **Step 2: Observe CI, fix any fmt/clippy/zip/env issues from the gotchas list**

Likely touches:
- `serde_yaml_bw` alias pattern
- `greenticDesigner: "*"` field presence
- Zip output normalization for CI runners
- `std::env::set_var` unsafe guard pattern in any env-manipulating test

Reference `deploy-extension-migration.md` memory for full gotcha list.

---

## Wave C — E2E validation (`greentic-e2e`)

**Can ship after Waves A + B merged, or during Batch 1 observation window.**

### Task C1: Provider extension E2E script

**Files:**
- Create: `scripts/run_provider_extension_e2e.sh`
- Create: `fixtures/provider-telegram-test/.env.example`

- [ ] **Step 1: Author test script**

Create `scripts/run_provider_extension_e2e.sh`:

```bash
#!/usr/bin/env bash
set -euo pipefail

# E2E: install Telegram ext via gtdx → start runner → send test message via bot API

HERE="$(cd "$(dirname "$0")" && pwd)"
REPO="$(cd "$HERE/.." && pwd)"

# Precondition: TELEGRAM_TEST_BOT_TOKEN + TELEGRAM_TEST_CHAT_ID env vars
: "${TELEGRAM_TEST_BOT_TOKEN:?must be set}"
: "${TELEGRAM_TEST_CHAT_ID:?must be set}"

# 1. Fetch latest Telegram .gtxpack (or build from source)
GTXPACK="${PROVIDER_TELEGRAM_GTXPACK:-}"
if [ -z "$GTXPACK" ]; then
  echo "==> Building Telegram .gtxpack from source"
  # Assume greentic-messaging-providers repo checked out as sibling
  MP="${MESSAGING_PROVIDERS_REPO:-$REPO/../greentic-messaging-providers}"
  bash "$MP/crates/provider-telegram-extension/build.sh"
  GTXPACK=$(ls -t "$MP"/greentic.provider.telegram-*.gtxpack | head -1)
fi
test -f "$GTXPACK" || { echo "gtxpack missing"; exit 1; }

# 2. Fresh home dir
TMP=$(mktemp -d)
trap 'rm -rf "$TMP"' EXIT
export HOME="$TMP"
export GREENTIC_EXT_ALLOW_UNSIGNED=1

# 3. Install via gtdx
gtdx install "$GTXPACK" -y

# 4. Verify directory layout
test -f "$HOME/.greentic/runtime/packs/providers/gtdx/greentic.provider.telegram-0.1.0.gtpack" || {
  echo "ERROR: gtpack not extracted"
  exit 1
}
test -f "$HOME/.greentic/extensions/provider/greentic.provider.telegram/0.1.0/describe.json" || {
  echo "ERROR: describe.json not extracted"
  exit 1
}

# 5. Start runner in background with this home
greentic-runner --home "$HOME" --provider-poll-secs 2 &
RUNNER_PID=$!
trap 'kill $RUNNER_PID 2>/dev/null; rm -rf "$TMP"' EXIT
sleep 5  # wait for pack poll + load

# 6. Verify runner picked up pack (check via runner admin endpoint)
curl -sf http://localhost:3000/admin/packs | grep -q "greentic.provider.telegram" || {
  echo "ERROR: runner did not load pack"
  exit 1
}

# 7. Trigger a test send via runner ingress
curl -X POST http://localhost:3000/ingress/telegram/test \
  -H "Content-Type: application/json" \
  -d "{\"chat_id\": $TELEGRAM_TEST_CHAT_ID, \"text\": \"E2E test from provider extension\"}"

# 8. Verify delivery via Telegram getUpdates API
sleep 2
LATEST=$(curl -sf "https://api.telegram.org/bot$TELEGRAM_TEST_BOT_TOKEN/getUpdates?limit=1")
echo "$LATEST" | grep -q "E2E test from provider extension" || {
  echo "ERROR: message not delivered"
  exit 1
}

echo "==> Provider extension E2E passed"
```

Make executable:
```bash
chmod +x scripts/run_provider_extension_e2e.sh
```

- [ ] **Step 2: Create .env.example**

Create `fixtures/provider-telegram-test/.env.example`:

```env
TELEGRAM_TEST_BOT_TOKEN=123456789:ABC...
TELEGRAM_TEST_CHAT_ID=-100123456789
PROVIDER_TELEGRAM_GTXPACK=/path/to/greentic.provider.telegram-0.1.0.gtxpack
MESSAGING_PROVIDERS_REPO=/path/to/greentic-messaging-providers
```

- [ ] **Step 3: Local run (with real bot)**

```bash
source fixtures/provider-telegram-test/.env
./scripts/run_provider_extension_e2e.sh
```

Expected: script prints "==> Provider extension E2E passed"

- [ ] **Step 4: Commit**

```bash
git add scripts/run_provider_extension_e2e.sh fixtures/provider-telegram-test/
git commit -m "test(e2e): Telegram provider extension install + runner pickup + send"
```

---

### Task C2: Wire into nightly CI (optional — can defer)

**Files:**
- Modify: `.github/workflows/nightly.yml`

- [ ] **Step 1: Add provider extension job**

Append to `.github/workflows/nightly.yml`:

```yaml
  provider-extension-telegram:
    runs-on: ubuntu-latest
    needs: [install-check]
    steps:
      - uses: actions/checkout@v4
      - name: Install gtdx
        run: cargo install --git https://github.com/greentic-biz/greentic-designer-extensions greentic-ext-cli --bin gtdx --locked
      - name: Install greentic-runner
        run: cargo install greentic-runner --locked
      - name: Checkout messaging-providers
        uses: actions/checkout@v4
        with:
          repository: greenticai/greentic-messaging-providers
          path: messaging-providers
      - name: Run E2E
        env:
          TELEGRAM_TEST_BOT_TOKEN: ${{ secrets.TELEGRAM_TEST_BOT_TOKEN }}
          TELEGRAM_TEST_CHAT_ID: ${{ secrets.TELEGRAM_TEST_CHAT_ID }}
          MESSAGING_PROVIDERS_REPO: ${{ github.workspace }}/messaging-providers
        run: bash scripts/run_provider_extension_e2e.sh
```

- [ ] **Step 2: Commit**

```bash
git add .github/workflows/nightly.yml
git commit -m "ci(nightly): provider extension Telegram E2E job"
```

---

## Self-Review Checklist

Before marking plan complete:

- [ ] **Spec coverage.** Every spec §-numbered decision has at least one implementing task:
  - §6.2 directory convention → Task A6 creates `gtdx/` subdir
  - §6.3 conflict resolution → Task A6 `check_manual_pack_conflict`
  - §7.1 WIT package location → Task A4
  - §7.3 interface shapes → Task A4 (full WIT)
  - §8.1-8.6 artifact + lifecycle → Tasks A6, B4
  - §9 consumer integration — deferred to Batch 1+ (registry API already covered in A5)
  - §10.2 pilot acceptance gates → Tasks B4-B10
  - §11 testing strategy → covered across A6, B5, B6, B8, C1
- [ ] **No placeholders.** Checked: no "TBD", no "similar to above", all code blocks complete.
- [ ] **Type consistency.** `ProviderRuntime`/`RuntimeGtpack` struct names consistent across A2/A3/A6. `install_from_path` signature consistent between A6/A10/B8.
- [ ] **File paths absolute & correct.** All paths verified against existing repo layout in designer-extensions + messaging-providers.

---

## Summary & Handoff

**Total tasks:** 13 (Wave A) + 10 (Wave B) + 2 (Wave C) = **25 tasks**

**Estimated effort:** Wave A ~1 week, Wave B ~1 week, Wave C ~2 days. Total ~2.5 weeks solo, less with subagent parallelism.

**Critical paths:**
- Tasks A1 → A2 → A3 → A6 → A10 → A13 (Wave A merge chain)
- A13 merge SHA → B1 WIT vendor commit note → B10 PR rev pin (Wave B dep chain)
- B10 merge → C1 E2E script activation

**Parallel opportunities (subagent-driven):**
- A4 (WIT file) independent from A1-A3 contract work
- A5 (registry trait) independent from A1-A4
- A7 registry override tests independent from A8-A10
- A12 docs writeable in parallel with any A1-A11
- B3 (schemas/i18n) independent from B2 (crate scaffold)
- B5 tests independent from B4 build script

**Merge sequence:**
1. A13 PR merges in `greentic-designer-extensions`
2. Record merge SHA
3. B1 through B10 in `greentic-messaging-providers`, pinning to merge SHA in B9
4. C1/C2 in `greentic-e2e`

**Next plan:** `2026-04-??-provider-extension-batch-1.md` — WebChat + Email + Cron, after Batch 0 observation window (~1 sprint).
