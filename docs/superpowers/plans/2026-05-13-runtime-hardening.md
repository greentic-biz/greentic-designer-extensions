# Runtime Hardening (Phase C) Implementation Plan

> **Status (2026-05-17): PARTIALLY SHIPPED on `research`. v1→v2 migration tasks complete; deeper hardening (resource limits / async safety / cwasm cache / two-version coexistence) NOT in scope for the 1.2.x research line.**
>
> | Task | Status | PR |
> |---|---|---|
> | C.1 (sdk-contract dep → `=1.2.0-research`) | DONE | greentic-biz/greentic-designer-extensions#54 |
> | C.2 (loaded.rs v2 describe migration) | DONE | same PR |
> | C.3 (sweep ext-runtime + sibling crates for v1-shape uses) | DONE | same PR |
> | C.4 (open the contract-bump PR) | DONE | landed as #54 |
> | gtdx-version cascade across 8 extension repos | DONE | 8 cascade PRs (`gtdx-bump-1.2.1-research`) |
>
> **NOT shipped — deferred (substantial design work each, not blocking 1.2.x line):**
> - `HashMap<(ExtensionId, Version), _>` re-keying for two-version coexistence
> - wasmtime `fuel` / `epoch_interruption` / `StoreLimits` resource caps
> - `async fn` wrappers around `tokio::task::spawn_blocking` for sync wasmtime dispatch
> - Content-addressed `.cwasm` cache
> - sha256 verification of installed wasm vs describe-declared hash (D.4 manifest verifies dir contents — overlapping but not identical)
> - Per-extension error attribution + metrics
> - In-flight upgrade safety
> - Required-capability resolution at install
> - Atomic state mutations
> - State-file schema migration scaffold
>
> These remain on the audit punch list as P1 items for a future hardening pass.
>
> Original plan body preserved below as historical record + design rationale.
>
> ---

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Close the 11 P0/P1 runtime hardening findings from the May-2026 audit: two-version coexistence, wasmtime resource limits, async-safety, content-addressed `.cwasm` cache, sha256 install verification, per-extension error attribution + metrics, in-flight upgrade safety, required-capability resolution at install, atomic state mutations, and a state-file schema migration scaffold.

**Architecture:**
- All work lands in `greentic-designer-extensions/crates/greentic-ext-runtime` (and a small consumer migration in `greentic-designer/src/ui/tool_bridge/dispatch.rs` + `greentic-extension-sdk-registry/src/lifecycle.rs`).
- Two foundational shifts: re-key `loaded: HashMap<ExtensionId, _>` to `HashMap<(ExtensionId, Version), _>` (a new strong type `LoadedKey`), and lift `invoke_tool` (and the other sync-wasmtime dispatch methods) into `async fn` wrappers around `tokio::task::spawn_blocking`.
- wasmtime hardening is centralized at engine construction (`fuel`, `epoch_interruption`) plus a per-`Store` configuration helper that wires `StoreLimits` from `describe.runtime.memory_limit_mb`, a fuel budget, and an epoch deadline. A single background ticker task in `ExtensionRuntime::new` increments the engine epoch every second.
- The `.cwasm` cache is content-addressed by SHA-256 of the WASM bytes under `<cache_root>/<sha256>.cwasm`. Cache hits skip recompilation via `unsafe`-free `Component::deserialize_file` (wasmtime 43 accepts a sealed path; we wrap the call in our own crate keeping `#![forbid(unsafe_code)]` intact).
- Metrics use the `metrics` + `metrics-exporter-prometheus` crates. The exporter is owned by the designer (axum), the runtime only emits counters/histograms.

**Tech Stack:** Rust 1.95, edition 2024, `#![forbid(unsafe_code)]`. wasmtime 43 (component-model + async), tokio 1, `metrics` 0.23, `metrics-exporter-prometheus` 0.15, `sha2` 0.10, `fs2` 0.4 (already in workspace). PRs target `research`. Conventional Commits, no Claude attribution.

**Dependency:** Plan A (`greentic-designer-sdk/docs/superpowers/plans/2026-05-13-contract-0.5.0-bump.md`) MUST be merged first — Plan C consumes the new `Permissions`/`Compat` types and the `ExtensionMetadata.artifact_sha256` field as their canonical shape. Bump `greentic-extension-sdk-contract = "0.5"` in `crates/greentic-ext-runtime/Cargo.toml` as the first action of Task C.1.

---

## File map

**Modified (existing files):**
- `crates/greentic-ext-runtime/Cargo.toml` — add deps (`sha2`, `metrics`, `metrics-exporter-prometheus`, `once_cell`, `fs2`, `sysinfo` not needed).
- `crates/greentic-ext-runtime/src/lib.rs` — new module exports (`cache`, `metrics`, `state_migrate`, `LoadedKey`).
- `crates/greentic-ext-runtime/src/loaded.rs` — add `LoadedKey(ExtensionId, Version)`, change `LoadedExtension::load_from_dir` to also return key; keep `id` field on `LoadedExtension`, add `version: Version` field.
- `crates/greentic-ext-runtime/src/runtime.rs` — re-key `loaded` to `HashMap<LoadedKey, _>`, wire engine config (fuel + epoch + cache), spawn epoch ticker, async wrappers for dispatch methods, capability resolution on install, in-flight migration tracking, error attribution.
- `crates/greentic-ext-runtime/src/host_state.rs` — add `ext_id` + `tool_name` fields on every `tracing::error!` / `tracing::warn!`.
- `crates/greentic-ext-runtime/src/error.rs` — add new variants `IntegrityError`, `UnsatisfiedRequiredCapabilities`, `MigrationInFlight`.
- `crates/greentic-ext-runtime/src/discovery.rs` — add `cache_root()` helper.
- `greentic-designer-sdk/crates/greentic-extension-sdk-registry/src/lifecycle.rs` — add sha256 verification + required-capability resolver hooks.
- `greentic-designer-sdk/crates/greentic-extension-sdk-state/src/state.rs` — add `from_v1_to_v2` migration stub + `UnsupportedSchema` error variant + read-modify-write helper.
- `greentic-designer-sdk/crates/greentic-extension-sdk-state/src/error.rs` — add `UnsupportedSchema` variant.
- `greentic-designer-sdk/crates/greentic-extension-sdk-state/src/atomic.rs` — add `update_atomic` helper (read-modify-write under lock).
- `greentic-designer/src/ui/tool_bridge/dispatch.rs:483` — `.await` the async `invoke_tool`.
- `greentic-designer/src/ui/state.rs` and `greentic-designer/src/ui/routes/mod.rs` — wire `/metrics` Prometheus endpoint.

**New files:**
- `crates/greentic-ext-runtime/src/cache.rs` — `.cwasm` content-addressed cache.
- `crates/greentic-ext-runtime/src/metrics.rs` — counter/histogram registration + recording helpers.
- `crates/greentic-ext-runtime/src/store_config.rs` — `apply_limits_and_deadline()` for each new `Store`.
- `crates/greentic-ext-runtime/src/migration.rs` — `MigrationTracker` for in-flight Plan pinning during `ExtensionUpdated`.
- `crates/greentic-ext-runtime/tests/two_version_coexistence.rs` — integration test for C.1.
- `crates/greentic-ext-runtime/tests/resource_limits.rs` — integration tests for C.2 (memory/epoch/fuel traps).
- `crates/greentic-ext-runtime/tests/async_invoke.rs` — integration test for C.3 (parallel non-blocking).
- `crates/greentic-ext-runtime/tests/cwasm_cache.rs` — integration tests for C.4.
- `crates/greentic-ext-runtime/tests/install_integrity.rs` — under registry repo, but local fixture here.
- `crates/greentic-ext-runtime/tests/migration_in_flight.rs` — integration test for C.8.
- `crates/greentic-ext-runtime/tests/metrics_scrape.rs` — integration test for C.7.
- `crates/greentic-ext-runtime/tests/support/wasm_fixtures.rs` — shared `wat::parse_str` fixtures for memory/epoch/fuel/slow tools.
- `greentic-designer-sdk/crates/greentic-extension-sdk-registry/tests/install_sha256.rs` — sha256 verification test.
- `greentic-designer-sdk/crates/greentic-extension-sdk-state/tests/schema_migration.rs` — schema rejection test.
- `greentic-designer-sdk/crates/greentic-extension-sdk-state/tests/concurrent_set_state.rs` — parallel enable race test (C.10).

---

## Task C.1 — Re-key `loaded` map by `(ExtensionId, Version)`

**Files:**
- Modify: `crates/greentic-ext-runtime/Cargo.toml`
- Modify: `crates/greentic-ext-runtime/src/loaded.rs`
- Modify: `crates/greentic-ext-runtime/src/runtime.rs`
- Modify: `crates/greentic-ext-runtime/src/lib.rs`
- Create: `crates/greentic-ext-runtime/tests/two_version_coexistence.rs`
- Create: `crates/greentic-ext-runtime/tests/support/mod.rs`
- Create: `crates/greentic-ext-runtime/tests/support/wasm_fixtures.rs`

- [ ] **Step 1: Bump SDK dep + add sha2 in `Cargo.toml`**

Modify `[dependencies]` block in `crates/greentic-ext-runtime/Cargo.toml`:

```toml
greentic-extension-sdk-contract = "0.5"

[dependencies.sha2]
workspace = true
```

And under `[dev-dependencies]`:

```toml
greentic-extension-sdk-testing = "0.5"
```

Also pin the existing workspace `semver` dep (already listed). No other changes in this step.

- [ ] **Step 2: Write the failing two-version coexistence test**

Create `crates/greentic-ext-runtime/tests/support/mod.rs`:

```rust
//! Shared test scaffolding for runtime integration tests.

pub mod wasm_fixtures;
```

Create `crates/greentic-ext-runtime/tests/support/wasm_fixtures.rs`:

```rust
//! Hand-written WIT component fixtures used by integration tests.
//!
//! Each `build_*` returns the bytes of a component WASM and a synthetic
//! `describe.json` that the runtime can ingest via `register_loaded_from_dir`.
//!
//! Fixtures intentionally export *only* what the test needs — keeping them
//! ~30 lines of WAT means a fixture parse error fails the test fast.

use std::path::Path;

/// Write a synthetic extension dir at `dir` consisting of `extension.wasm`
/// (the supplied bytes) and a minimal valid `describe.json` for kind=Design
/// at the given id+version. The describe is unsigned — tests must use
/// `GREENTIC_EXT_ALLOW_UNSIGNED=1` until Plan D lands the proper trust root.
pub fn write_design_extension(
    dir: &Path,
    id: &str,
    version: &str,
    wasm_bytes: &[u8],
    memory_limit_mb: u32,
) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)?;
    std::fs::write(dir.join("extension.wasm"), wasm_bytes)?;
    let describe = serde_json::json!({
        "$schema": "https://schemas.greentic.dev/extensions/describe-v1.json",
        "apiVersion": "greentic.dev/v1",
        "kind": "DesignExtension",
        "metadata": {
            "id": id,
            "name": id,
            "version": version,
            "summary": "fixture",
            "author": {"name": "test"},
            "license": "MIT"
        },
        "engine": {
            "greenticDesigner": ">=0.7.0",
            "extRuntime": ">=1.2.0"
        },
        "capabilities": {"offered": [], "required": []},
        "runtime": {
            "component": "extension.wasm",
            "memoryLimitMB": memory_limit_mb,
            "permissions": {}
        },
        "contributions": {}
    });
    std::fs::write(dir.join("describe.json"), serde_json::to_vec_pretty(&describe)?)?;
    Ok(())
}

/// Build a design component that exports
/// `greentic:extension-design/tools@0.2.0` with `invoke-tool` returning the
/// supplied constant string for any args. Used to prove two versions of
/// the same id produce two different responses.
pub fn echo_design_component(reply: &str) -> Vec<u8> {
    // Minimal WIT component: imports nothing, exports tools.invoke-tool
    // that ignores arguments and returns `reply`. We hand-roll WAT here
    // because cargo-component would balloon the test compile time.
    let wat = format!(r#"
        (component
          (core module $m
            (memory (export "memory") 1)
            (func (export "alloc") (param i32) (result i32) (local.get 0))
            (func (export "len") (result i32) (i32.const {len}))
            (func (export "ptr") (result i32) (i32.const 1024))
            (data (i32.const 1024) {data})
          )
          (core instance $i (instantiate $m))
          (func $reply (result string)
            (canon lift (core func $i "ptr") (memory $i "memory"))
          )
          (component $tools
            (export "invoke-tool" (func (param "name" string) (param "args" string) (result (result string)))))
          ;; NOTE: this fixture is intentionally minimal — full WIT
          ;; component synthesis lives in extension repos; tests only
          ;; assert distinct return values.
        )
    "#,
        len = reply.len(),
        data = format!("\"{}\"", reply.escape_default()));
    wat::parse_str(&wat).expect("fixture WAT compiles")
}
```

Note: the WAT fixture above is illustrative — Step 3 below replaces it with the simpler approach of using a prebuilt no-op echo component checked into `tests/fixtures/`. We'll commit that fixture binary in Step 3.

Create `crates/greentic-ext-runtime/tests/two_version_coexistence.rs`:

```rust
//! C.1 — Two versions of the same extension id must co-exist under
//! distinct (id, version) keys in `loaded`. Invoking each must dispatch
//! to the right component.

mod support;

use greentic_ext_runtime::{ExtensionRuntime, RuntimeConfig, DiscoveryPaths};
use semver::Version;
use support::wasm_fixtures::{write_design_extension, echo_design_component};

#[tokio::test]
async fn two_versions_of_same_id_coexist() {
    let _g = scopeguard::guard((), |_| {});
    // Tests run unsigned until Plan D's trust root lands.
    // SAFETY: env vars are process-global; tests in this crate are
    // already serialised via `--test-threads=1` in `ci/local_check.sh`.
    unsafe { std::env::set_var("GREENTIC_EXT_ALLOW_UNSIGNED", "1"); }

    let tmp = tempfile::TempDir::new().unwrap();
    let user_root = tmp.path().join("extensions");
    let design = user_root.join("design");
    std::fs::create_dir_all(&design).unwrap();

    let v1_dir = design.join("ext.echo-1.9.0");
    let v2_dir = design.join("ext.echo-1.10.0");
    write_design_extension(&v1_dir, "ext.echo", "1.9.0", &echo_design_component("v1"), 64).unwrap();
    write_design_extension(&v2_dir, "ext.echo", "1.10.0", &echo_design_component("v2"), 64).unwrap();

    let cfg = RuntimeConfig::from_paths(DiscoveryPaths::new(user_root.clone()));
    let mut rt = ExtensionRuntime::new(cfg).unwrap();
    rt.register_loaded_from_dir(&v1_dir).unwrap();
    rt.register_loaded_from_dir(&v2_dir).unwrap();

    let loaded = rt.loaded();
    assert_eq!(loaded.len(), 2, "both versions of ext.echo must be loaded");
    let mut versions: Vec<_> = loaded.keys().map(|k| k.version().clone()).collect();
    versions.sort();
    assert_eq!(versions, vec![Version::new(1, 9, 0), Version::new(1, 10, 0)]);

    // Distinct dispatch on each.
    let r1 = rt.invoke_tool_versioned("ext.echo", &Version::new(1, 9, 0), "echo", "{}").await.unwrap();
    let r2 = rt.invoke_tool_versioned("ext.echo", &Version::new(1, 10, 0), "echo", "{}").await.unwrap();
    assert_ne!(r1, r2, "two versions must produce distinct results");
}
```

- [ ] **Step 3: Run the test to verify it fails**

Run: `cargo test -p greentic-ext-runtime --test two_version_coexistence -- --test-threads=1`
Expected: FAIL with `LoadedKey`, `version()`, `invoke_tool_versioned` not found.

- [ ] **Step 4: Replace fixture with checked-in component bytes**

The hand-WAT fixture in Step 2 is fragile. Replace `echo_design_component` and `write_design_extension` so the fixture uses a tiny prebuilt WASM checked in under `tests/fixtures/echo_v1.wasm` + `echo_v2.wasm`. Author each as a cargo-component crate under `tests/fixtures/echo-src/` and commit the artefacts.

Build instructions (in `tests/fixtures/echo-src/README.md`):

```
cd tests/fixtures/echo-src/echo-v1 && cargo component build --release --target wasm32-wasip2
cp target/wasm32-wasip2/release/echo_v1.wasm ../../echo_v1.wasm
```

For this step, commit the produced `.wasm` files directly and rewrite the fixture loader:

```rust
pub fn echo_design_component(variant: &str) -> Vec<u8> {
    let path = match variant {
        "v1" => "tests/fixtures/echo_v1.wasm",
        "v2" => "tests/fixtures/echo_v2.wasm",
        other => panic!("unknown echo variant: {other}"),
    };
    std::fs::read(path).expect("fixture wasm exists")
}
```

Author the echo extension as a normal cargo-component crate that exports
`greentic:extension-design/tools@0.2.0` and whose `invoke-tool` body is
literally `Ok(VARIANT.to_string())` where `VARIANT` is a `const &str`. The
two crates differ only in the const. Resulting `.wasm` is ~30 KB each.

- [ ] **Step 5: Implement `LoadedKey` and re-key `loaded` map**

Modify `crates/greentic-ext-runtime/src/loaded.rs`:

```rust
use semver::Version;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct LoadedKey {
    id: ExtensionId,
    version: Version,
}

impl LoadedKey {
    #[must_use]
    pub fn new(id: ExtensionId, version: Version) -> Self {
        Self { id, version }
    }

    #[must_use]
    pub fn from_describe(describe: &DescribeJson) -> Result<Self, semver::Error> {
        let version: Version = describe.metadata.version.parse()?;
        Ok(Self {
            id: ExtensionId::from_describe(describe),
            version,
        })
    }

    #[must_use]
    pub fn id(&self) -> &ExtensionId {
        &self.id
    }

    #[must_use]
    pub fn version(&self) -> &Version {
        &self.version
    }
}

impl std::fmt::Display for LoadedKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}@{}", self.id.as_str(), self.version)
    }
}
```

Add `pub version: Version` field to `LoadedExtension` and populate it in `load_from_dir`:

```rust
pub struct LoadedExtension {
    pub id: ExtensionId,
    pub version: Version,
    pub describe: Arc<DescribeJson>,
    pub kind: ExtensionKind,
    pub source_dir: PathBuf,
    pub component: Component,
    pub pool: InstancePool,
    pub health: ExtensionHealth,
}

impl LoadedExtension {
    pub fn key(&self) -> LoadedKey {
        LoadedKey::new(self.id.clone(), self.version.clone())
    }

    pub fn load_from_dir(engine: &wasmtime::Engine, source_dir: &Path) -> anyhow::Result<Self> {
        let describe_path = source_dir.join("describe.json");
        let describe_bytes = std::fs::read(&describe_path)?;
        let describe_value: serde_json::Value = serde_json::from_slice(&describe_bytes)?;
        greentic_extension_sdk_contract::schema::validate_describe_json(&describe_value)
            .map_err(|e| anyhow::anyhow!("invalid describe.json: {e}"))?;
        let describe: DescribeJson = serde_json::from_value(describe_value)?;
        let id = ExtensionId::from_describe(&describe);
        let version: Version = describe.metadata.version.parse()
            .map_err(|e: semver::Error| anyhow::anyhow!("invalid version '{}': {e}", describe.metadata.version))?;
        let wasm_path = source_dir.join(&describe.runtime.component);
        let component = Component::from_file(engine, &wasm_path)?;
        let pool = InstancePool::new(2);
        let kind = describe.kind;
        Ok(Self {
            id,
            version,
            describe: Arc::new(describe),
            kind,
            source_dir: source_dir.to_path_buf(),
            component,
            pool,
            health: ExtensionHealth::Healthy,
        })
    }
}
```

Add `pub use self::loaded::LoadedKey;` to `crates/greentic-ext-runtime/src/lib.rs`.

- [ ] **Step 6: Update `runtime.rs` to use `LoadedKey`**

Replace every `HashMap<ExtensionId, LoadedExtensionRef>` with `HashMap<LoadedKey, LoadedExtensionRef>`. Add a versioned + a back-compat lookup:

```rust
loaded: ArcSwap<HashMap<LoadedKey, LoadedExtensionRef>>,
```

Add helper methods:

```rust
impl ExtensionRuntime {
    /// Find a loaded extension by (id, version). Returns None if either is
    /// absent.
    #[must_use]
    pub fn get_loaded(&self, id: &str, version: &Version) -> Option<LoadedExtensionRef> {
        let key = LoadedKey::new(ExtensionId(id.to_string()), version.clone());
        self.loaded.load().get(&key).cloned()
    }

    /// Find the highest-version loaded extension for `id`. Used by
    /// invocation paths that don't pin a version (designer's tool-bridge
    /// dispatch hasn't been version-aware yet; it picks the newest).
    #[must_use]
    pub fn get_latest(&self, id: &str) -> Option<LoadedExtensionRef> {
        self.loaded
            .load()
            .iter()
            .filter(|(k, _)| k.id().as_str() == id)
            .max_by(|(a, _), (b, _)| a.version().cmp(b.version()))
            .map(|(_, v)| v.clone())
    }
}
```

Update `register_loaded_from_dir` to insert under `loaded.key()`, update `handle_removal` to match by `source_dir` (still unambiguous because two installed dirs differ by version suffix), update `handle_added_or_modified` to use the new key + emit `ExtensionUpdated` only when a strictly newer version replaces an older one for the same id.

Update every sync `invoke_tool` / `validate_content` / `list_tools` etc. to use `get_latest` for now (we'll add the async + versioned variants in C.3).

```rust
let loaded = self
    .get_latest(ext_id)
    .ok_or_else(|| RuntimeError::NotFound(ext_id.to_string()))?;
```

Update the `RuntimeEvent::ExtensionUpdated` variant to also carry the new version:

```rust
ExtensionUpdated {
    id: ExtensionId,
    prev_version: Version,
    new_version: Version,
},
```

Migrate the existing `ExtensionRemoved(ExtensionId)` to `ExtensionRemoved { id: ExtensionId, version: Version }` similarly.

- [ ] **Step 7: Add `invoke_tool_versioned` (stub — async variant in C.3)**

Inside `impl ExtensionRuntime` in `runtime.rs`:

```rust
/// Like `invoke_tool` but pinned to an exact (id, version). Used by
/// integration tests and the in-flight migration tracker (see C.8) to
/// route to a specific instance.
pub async fn invoke_tool_versioned(
    &self,
    ext_id: &str,
    version: &Version,
    tool_name: &str,
    args_json: &str,
) -> Result<String, RuntimeError> {
    let key = LoadedKey::new(ExtensionId(ext_id.to_string()), version.clone());
    let loaded = self
        .loaded
        .load()
        .get(&key)
        .cloned()
        .ok_or_else(|| RuntimeError::NotFound(format!("{ext_id}@{version}")))?;
    self.invoke_on_loaded(loaded, tool_name, args_json).await
}
```

`invoke_on_loaded` is the shared body — it currently runs synchronously; C.3 wraps it in `spawn_blocking`. For now, mark it `async` and call the body directly:

```rust
async fn invoke_on_loaded(
    &self,
    loaded: LoadedExtensionRef,
    tool_name: &str,
    args_json: &str,
) -> Result<String, RuntimeError> {
    // C.3 replaces this body with spawn_blocking. For now keep it sync
    // inside async so the public surface is stable.
    let engine = self.engine.clone();
    let tool_name = tool_name.to_string();
    let args_json = args_json.to_string();
    invoke_on_loaded_sync(&engine, &loaded, &tool_name, &args_json)
}
```

Move the existing dispatch body into a free `fn invoke_on_loaded_sync(...)` that returns `Result<String, RuntimeError>`. Keep the resolve-iface logic intact.

- [ ] **Step 8: Run the integration test to verify it passes**

Run: `cargo test -p greentic-ext-runtime --test two_version_coexistence -- --test-threads=1`
Expected: PASS.

- [ ] **Step 9: Update internal call sites + fix the build**

Run `cargo build -p greentic-ext-runtime` and fix any remaining call sites that referenced the old `HashMap<ExtensionId, _>` shape. In particular:
- `runtime.rs:handle_removal` — replace `iter().find(|(_, v)| v.source_dir == dir)` find logic; `LoadedKey` is still indexable by source_dir.
- All `RuntimeEvent::ExtensionRemoved(id)` constructions — update to struct variant.

Run: `cargo build -p greentic-ext-runtime`
Expected: builds clean.

- [ ] **Step 10: Run full crate test suite**

Run: `cargo test -p greentic-ext-runtime -- --test-threads=1`
Expected: all existing tests still pass + new two-version test passes.

- [ ] **Step 11: Commit**

```bash
git add crates/greentic-ext-runtime/Cargo.toml \
        crates/greentic-ext-runtime/src/loaded.rs \
        crates/greentic-ext-runtime/src/runtime.rs \
        crates/greentic-ext-runtime/src/lib.rs \
        crates/greentic-ext-runtime/tests/two_version_coexistence.rs \
        crates/greentic-ext-runtime/tests/support/ \
        crates/greentic-ext-runtime/tests/fixtures/
git commit -m "feat(runtime): key loaded extensions by (id, version) to support coexistence

Audit P0 #9 — two versions of the same extension id can now load
simultaneously under distinct LoadedKey entries. invoke_tool_versioned
pins dispatch to an exact version; invoke_tool keeps picking the latest
for back-compat.

Refs: docs/superpowers/specs/2026-05-13-extensions-1.0-cleanup.md §4"
```

---

## Task C.2 — Wasmtime engine: fuel + epoch + StoreLimits

**Files:**
- Modify: `crates/greentic-ext-runtime/Cargo.toml`
- Modify: `crates/greentic-ext-runtime/src/runtime.rs`
- Modify: `crates/greentic-ext-runtime/src/loaded.rs`
- Create: `crates/greentic-ext-runtime/src/store_config.rs`
- Modify: `crates/greentic-ext-runtime/src/host_state.rs`
- Modify: `crates/greentic-ext-runtime/src/lib.rs`
- Create: `crates/greentic-ext-runtime/tests/resource_limits.rs`
- Create: `crates/greentic-ext-runtime/tests/fixtures/balloon_src/` (cargo-component fixture)
- Create: `crates/greentic-ext-runtime/tests/fixtures/loop_src/`
- Create: `crates/greentic-ext-runtime/tests/fixtures/fuel_src/`

- [ ] **Step 1: Add `RuntimeLimits` struct to `RuntimeConfig`**

Modify `crates/greentic-ext-runtime/src/runtime.rs`:

```rust
#[derive(Clone, Debug)]
pub struct RuntimeLimits {
    /// Per-invocation fuel quota. Default 10_000_000_000 (~10s of dense
    /// numeric work on a modern x86_64).
    pub fuel_per_invocation: u64,
    /// Hard wall-clock cap per invocation, in seconds. The runtime bumps
    /// the engine epoch every second, so the actual trap happens within
    /// `epoch_deadline_seconds + 1` of the call.
    pub epoch_deadline_seconds: u64,
    /// Epoch ticker period. Defaults to 1s. Tests may shorten this.
    pub epoch_tick_period: std::time::Duration,
}

impl Default for RuntimeLimits {
    fn default() -> Self {
        Self {
            fuel_per_invocation: 10_000_000_000,
            epoch_deadline_seconds: 30,
            epoch_tick_period: std::time::Duration::from_secs(1),
        }
    }
}

#[derive(Clone, Debug)]
pub struct RuntimeConfig {
    pub paths: DiscoveryPaths,
    pub limits: RuntimeLimits,
}

impl RuntimeConfig {
    #[must_use]
    pub fn from_paths(paths: DiscoveryPaths) -> Self {
        Self { paths, limits: RuntimeLimits::default() }
    }

    #[must_use]
    pub fn with_limits(mut self, limits: RuntimeLimits) -> Self {
        self.limits = limits;
        self
    }
}
```

- [ ] **Step 2: Wire `fuel` + `epoch_interruption` on engine + spawn ticker**

Modify `ExtensionRuntime::new`:

```rust
pub fn new(config: RuntimeConfig) -> Result<Self, RuntimeError> {
    let mut ec = wasmtime::Config::new();
    ec.wasm_component_model(true);
    ec.consume_fuel(true);
    ec.epoch_interruption(true);
    let engine = Engine::new(&ec).map_err(|e| RuntimeError::Wasmtime(e.into()))?;
    let (tx, _) = broadcast::channel(64);

    // Spawn the epoch ticker. The handle is owned by `Self` so it's
    // joined cleanly on Drop.
    let epoch_engine = engine.clone();
    let period = config.limits.epoch_tick_period;
    let (stop_tx, stop_rx) = std::sync::mpsc::channel::<()>();
    let join = std::thread::Builder::new()
        .name("greentic-ext-epoch-ticker".into())
        .spawn(move || loop {
            match stop_rx.recv_timeout(period) {
                Ok(()) | Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                    epoch_engine.increment_epoch();
                }
            }
        })
        .map_err(|e| RuntimeError::Wasmtime(anyhow::anyhow!("spawn epoch ticker: {e}")))?;

    Ok(Self {
        engine,
        config,
        loaded: ArcSwap::from_pointee(HashMap::new()),
        capability_registry: ArcSwap::from_pointee(CapabilityRegistry::default()),
        events: tx,
        epoch_ticker: Some(EpochTicker { stop_tx: Some(stop_tx), join: Some(join) }),
        migration_tracker: Arc::new(crate::migration::MigrationTracker::default()),
    })
}
```

Add struct + Drop near the top of the file:

```rust
struct EpochTicker {
    stop_tx: Option<std::sync::mpsc::Sender<()>>,
    join: Option<std::thread::JoinHandle<()>>,
}

impl Drop for EpochTicker {
    fn drop(&mut self) {
        drop(self.stop_tx.take());
        if let Some(h) = self.join.take() {
            let _ = h.join();
        }
    }
}
```

Add `epoch_ticker: Option<EpochTicker>` field on `ExtensionRuntime`.

(The `migration_tracker` is wired here so the field exists from this point — its body lands in C.8.)

- [ ] **Step 3: Create `store_config.rs`**

Create `crates/greentic-ext-runtime/src/store_config.rs`:

```rust
//! Per-`Store` resource limit configuration.
//!
//! Each call into a WASM extension creates a fresh `Store<HostState>`.
//! We attach `StoreLimits` honouring the extension's declared
//! `memoryLimitMB`, set a fuel quota, and arm the epoch deadline so
//! infinite loops trap within `epoch_deadline_seconds + 1`.

use wasmtime::{Store, StoreLimits, StoreLimitsBuilder};

use crate::host_state::HostState;
use crate::runtime::RuntimeLimits;

/// Apply per-invocation resource limits to a freshly created store.
///
/// Returns the configured store builder; caller must `into_data()`-style
/// retain references. Currently mutates in place.
pub fn apply_limits(
    store: &mut Store<HostState>,
    memory_limit_mb: u32,
    limits: &RuntimeLimits,
) -> anyhow::Result<()> {
    let bytes = u64::from(memory_limit_mb) * 1024 * 1024;
    let store_limits: StoreLimits = StoreLimitsBuilder::new()
        .memory_size(usize::try_from(bytes).unwrap_or(usize::MAX))
        .build();
    store.data_mut().store_limits = store_limits;
    store.limiter(|data| &mut data.store_limits);

    store.set_fuel(limits.fuel_per_invocation)?;
    // Epoch deadline is counted in ticks. The ticker bumps every
    // `epoch_tick_period` (default 1s), so deadline = seconds.
    store.set_epoch_deadline(limits.epoch_deadline_seconds);
    Ok(())
}
```

- [ ] **Step 4: Carry `store_limits` on `HostState`**

Modify `crates/greentic-ext-runtime/src/host_state.rs`:

```rust
pub struct HostState {
    pub extension_id: String,
    pub permissions: Permissions,
    pub store_limits: wasmtime::StoreLimits,
    wasi: WasiCtx,
    table: ResourceTable,
}

impl HostState {
    #[must_use]
    pub fn new(extension_id: String, permissions: Permissions) -> Self {
        let wasi = WasiCtxBuilder::new().build();
        let table = ResourceTable::new();
        Self {
            extension_id,
            permissions,
            store_limits: wasmtime::StoreLimits::default(),
            wasi,
            table,
        }
    }
}
```

- [ ] **Step 5: Use `apply_limits` in `LoadedExtension::build_store_and_instance`**

Modify `crates/greentic-ext-runtime/src/loaded.rs`:

```rust
pub fn build_store_and_instance(
    &self,
    engine: &wasmtime::Engine,
    limits: &crate::runtime::RuntimeLimits,
) -> anyhow::Result<(Store<HostState>, Instance)> {
    // ... existing linker setup ...

    let state = HostState::new(
        self.id.as_str().to_string(),
        self.describe.runtime.permissions.clone(),
    );
    let mut store = Store::new(engine, state);
    crate::store_config::apply_limits(&mut store, self.describe.runtime.memory_limit_mb, limits)?;
    let instance = linker.instantiate(&mut store, &self.component)?;
    Ok((store, instance))
}
```

Update every caller in `runtime.rs` to thread `&self.config.limits`:

```rust
let (mut store, instance) = loaded
    .build_store_and_instance(&self.engine, &self.config.limits)
    .map_err(RuntimeError::Wasmtime)?;
```

- [ ] **Step 6: Write the failing memory-trap test**

Create three cargo-component fixtures under `tests/fixtures/`:
- `balloon-src/` — exports `invoke-tool` that allocates a 200 MB `Vec<u8>` and returns success.
- `loop-src/` — exports `invoke-tool` that runs `loop {}` forever.
- `fuel-src/` — exports `invoke-tool` with a tight numeric loop that consumes ~30 G fuel.

Build each + check the resulting `.wasm` into the repo.

Create `crates/greentic-ext-runtime/tests/resource_limits.rs`:

```rust
mod support;

use greentic_ext_runtime::{ExtensionRuntime, RuntimeConfig, RuntimeLimits, DiscoveryPaths};
use semver::Version;
use std::time::Duration;
use support::wasm_fixtures::write_design_extension;

fn make_runtime(limits: RuntimeLimits) -> (tempfile::TempDir, ExtensionRuntime) {
    // SAFETY: serialised by --test-threads=1.
    unsafe { std::env::set_var("GREENTIC_EXT_ALLOW_UNSIGNED", "1"); }
    let tmp = tempfile::TempDir::new().unwrap();
    let cfg = RuntimeConfig::from_paths(DiscoveryPaths::new(tmp.path().join("extensions")))
        .with_limits(limits);
    let rt = ExtensionRuntime::new(cfg).unwrap();
    (tmp, rt)
}

#[tokio::test]
async fn memory_limit_traps_large_allocation() {
    let (tmp, mut rt) = make_runtime(RuntimeLimits::default());
    let dir = tmp.path().join("extensions/design/balloon");
    let wasm = std::fs::read("tests/fixtures/balloon.wasm").unwrap();
    write_design_extension(&dir, "ext.balloon", "1.0.0", &wasm, 64).unwrap();
    rt.register_loaded_from_dir(&dir).unwrap();

    let err = rt.invoke_tool_versioned("ext.balloon", &Version::new(1, 0, 0), "balloon", "{}").await.unwrap_err();
    let msg = format!("{err}");
    assert!(msg.contains("memory") || msg.contains("trap"), "unexpected error: {msg}");
}

#[tokio::test]
async fn epoch_deadline_traps_infinite_loop() {
    let limits = RuntimeLimits {
        epoch_deadline_seconds: 1,
        epoch_tick_period: Duration::from_millis(100),
        ..Default::default()
    };
    let (tmp, mut rt) = make_runtime(limits);
    let dir = tmp.path().join("extensions/design/loop");
    let wasm = std::fs::read("tests/fixtures/loop.wasm").unwrap();
    write_design_extension(&dir, "ext.loop", "1.0.0", &wasm, 64).unwrap();
    rt.register_loaded_from_dir(&dir).unwrap();

    let start = std::time::Instant::now();
    let err = rt.invoke_tool_versioned("ext.loop", &Version::new(1, 0, 0), "spin", "{}").await.unwrap_err();
    let elapsed = start.elapsed();
    assert!(elapsed < Duration::from_secs(5), "trap took too long: {elapsed:?}");
    let msg = format!("{err}");
    assert!(msg.contains("epoch") || msg.contains("interrupt") || msg.contains("trap"), "unexpected error: {msg}");
}

#[tokio::test]
async fn fuel_exhaustion_traps_loop() {
    let limits = RuntimeLimits {
        fuel_per_invocation: 100_000,
        ..Default::default()
    };
    let (tmp, mut rt) = make_runtime(limits);
    let dir = tmp.path().join("extensions/design/fuel");
    let wasm = std::fs::read("tests/fixtures/fuel.wasm").unwrap();
    write_design_extension(&dir, "ext.fuel", "1.0.0", &wasm, 64).unwrap();
    rt.register_loaded_from_dir(&dir).unwrap();

    let err = rt.invoke_tool_versioned("ext.fuel", &Version::new(1, 0, 0), "burn", "{}").await.unwrap_err();
    let msg = format!("{err}");
    assert!(msg.contains("fuel") || msg.contains("trap"), "unexpected error: {msg}");
}
```

- [ ] **Step 7: Run the tests to verify they fail**

Run: `cargo test -p greentic-ext-runtime --test resource_limits -- --test-threads=1`
Expected: tests compile but FAIL (memory test currently lets the alloc succeed; loop test runs forever and hits the test harness timeout; fuel test runs to completion).

- [ ] **Step 8: Add module declaration + lib export**

Modify `crates/greentic-ext-runtime/src/lib.rs`:

```rust
mod store_config;

pub use self::runtime::{ExtensionRuntime, RuntimeConfig, RuntimeEvent, RuntimeLimits, WatcherGuard};
```

- [ ] **Step 9: Run tests to verify the wiring**

Run: `cargo test -p greentic-ext-runtime --test resource_limits -- --test-threads=1`
Expected: PASS — memory traps, epoch traps within ~1.5s, fuel traps.

- [ ] **Step 10: Re-run full suite**

Run: `cargo test -p greentic-ext-runtime -- --test-threads=1`
Expected: all green.

- [ ] **Step 11: Commit**

```bash
git add crates/greentic-ext-runtime/Cargo.toml \
        crates/greentic-ext-runtime/src/runtime.rs \
        crates/greentic-ext-runtime/src/loaded.rs \
        crates/greentic-ext-runtime/src/store_config.rs \
        crates/greentic-ext-runtime/src/host_state.rs \
        crates/greentic-ext-runtime/src/lib.rs \
        crates/greentic-ext-runtime/tests/resource_limits.rs \
        crates/greentic-ext-runtime/tests/fixtures/
git commit -m "feat(runtime): enforce wasmtime fuel, epoch, and memory limits

Audit P0 #4 — every Store now honours describe.runtime.memoryLimitMB
via StoreLimits, gets a 10 G fuel budget, and is armed with a 30 s
epoch deadline. A background ticker bumps the engine epoch every 1 s
so runaway extensions trap deterministically. RuntimeLimits exposes
the knobs for tests.

Refs: docs/superpowers/specs/2026-05-13-extensions-1.0-cleanup.md §4"
```

---

## Task C.3 — Async-safe dispatch via `spawn_blocking`

**Files:**
- Modify: `crates/greentic-ext-runtime/src/runtime.rs`
- Modify: `greentic-designer/src/ui/tool_bridge/dispatch.rs`
- Modify: `greentic-designer/src/ui/routes/validate.rs` (and any other sync callers)
- Create: `crates/greentic-ext-runtime/tests/async_invoke.rs`
- Create: `crates/greentic-ext-runtime/tests/fixtures/slow-src/` + `slow.wasm`

- [ ] **Step 1: Build a `slow.wasm` fixture**

Create a `slow-src/` cargo-component crate whose `invoke-tool` body sleeps for 500 ms (use `std::thread::sleep`) and returns OK. Build it and commit `tests/fixtures/slow.wasm`.

- [ ] **Step 2: Write the failing parallelism test**

Create `crates/greentic-ext-runtime/tests/async_invoke.rs`:

```rust
mod support;

use greentic_ext_runtime::{ExtensionRuntime, RuntimeConfig, DiscoveryPaths};
use semver::Version;
use std::sync::Arc;
use std::time::{Duration, Instant};
use support::wasm_fixtures::write_design_extension;

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn parallel_invocations_dont_block_runtime() {
    // SAFETY: serialised by --test-threads=1.
    unsafe { std::env::set_var("GREENTIC_EXT_ALLOW_UNSIGNED", "1"); }
    let tmp = tempfile::TempDir::new().unwrap();
    let user = tmp.path().join("extensions");
    let dir = user.join("design/slow");
    let wasm = std::fs::read("tests/fixtures/slow.wasm").unwrap();
    write_design_extension(&dir, "ext.slow", "1.0.0", &wasm, 64).unwrap();

    let cfg = RuntimeConfig::from_paths(DiscoveryPaths::new(user));
    let mut rt = ExtensionRuntime::new(cfg).unwrap();
    rt.register_loaded_from_dir(&dir).unwrap();
    let rt = Arc::new(rt);

    let n = 10usize;
    let start = Instant::now();
    let mut handles = Vec::with_capacity(n);
    for _ in 0..n {
        let rt = rt.clone();
        handles.push(tokio::spawn(async move {
            rt.invoke_tool_versioned("ext.slow", &Version::new(1, 0, 0), "sleep", "{}")
                .await
        }));
    }
    for h in handles {
        h.await.unwrap().unwrap();
    }
    let elapsed = start.elapsed();
    // Each call sleeps 500 ms. With true parallelism, 10 calls finish
    // in well under 5 s (the serial-worst-case). We use 2 s as a
    // conservative bound — 4 worker threads × 500 ms = 1.25 s ideal,
    // 2 s allows for scheduler jitter and test-harness overhead.
    assert!(elapsed < Duration::from_secs(2), "ran too long: {elapsed:?} (parallelism broken)");
}
```

- [ ] **Step 3: Run it to verify it fails**

Run: `cargo test -p greentic-ext-runtime --test async_invoke -- --test-threads=1`
Expected: FAIL — `invoke_tool_versioned` currently runs the sync body inline on the tokio runtime, blocking other tokio tasks.

- [ ] **Step 4: Wrap dispatch in `spawn_blocking`**

Modify `runtime.rs` — rewrite `invoke_on_loaded`:

```rust
async fn invoke_on_loaded(
    &self,
    loaded: LoadedExtensionRef,
    tool_name: &str,
    args_json: &str,
) -> Result<String, RuntimeError> {
    let engine = self.engine.clone();
    let limits = self.config.limits.clone();
    let tool_name = tool_name.to_string();
    let args_json = args_json.to_string();
    tokio::task::spawn_blocking(move || {
        invoke_on_loaded_sync(&engine, &limits, &loaded, &tool_name, &args_json)
    })
    .await
    .map_err(|e| RuntimeError::Wasmtime(anyhow::anyhow!("spawn_blocking join: {e}")))?
}
```

`invoke_on_loaded_sync` signature becomes:

```rust
fn invoke_on_loaded_sync(
    engine: &Engine,
    limits: &RuntimeLimits,
    loaded: &LoadedExtension,
    tool_name: &str,
    args_json: &str,
) -> Result<String, RuntimeError> {
    let (mut store, instance) = loaded
        .build_store_and_instance(engine, limits)
        .map_err(RuntimeError::Wasmtime)?;
    // ... existing dispatch body using `store` + `instance` ...
}
```

Also replace the existing public `invoke_tool` with an async wrapper that delegates to `invoke_tool_versioned` after picking the latest version:

```rust
pub async fn invoke_tool(
    &self,
    ext_id: &str,
    tool_name: &str,
    args_json: &str,
) -> Result<String, RuntimeError> {
    let loaded = self
        .get_latest(ext_id)
        .ok_or_else(|| RuntimeError::NotFound(ext_id.to_string()))?;
    self.invoke_on_loaded(loaded, tool_name, args_json).await
}
```

Do the same `async + spawn_blocking` treatment to:
- `validate_content`
- `list_tools`
- `prompt_fragments`
- `knowledge_list` / `knowledge_get` / `knowledge_suggest`
- `validate_credentials` / `credential_schema` / `list_targets`
- `render_bundle`

For each, extract the dispatch body into a free `fn <name>_sync(...)`, then make the `impl` method `async` and call `spawn_blocking`.

- [ ] **Step 5: Run the test to verify it passes**

Run: `cargo test -p greentic-ext-runtime --test async_invoke -- --test-threads=1`
Expected: PASS.

- [ ] **Step 6: Migrate designer callers**

In `greentic-designer/src/ui/tool_bridge/dispatch.rs` line 483:

```rust
let result = runtime.invoke_tool(&ext_id, name, args).await?;
```

Verify the surrounding function is already `async`. The `dispatch` and `dispatch_with_opts` functions are called from axum handlers which are async — no signature change should be necessary, but check `find_owning_extension`:

```rust
async fn find_owning_extension(
    runtime: &greentic_ext_runtime::ExtensionRuntime,
    tool_name: &str,
) -> Option<String> {
    for (key, _ext) in runtime.loaded().iter() {
        if let Ok(tools) = runtime.list_tools(key.id().as_str()).await
            && tools.iter().any(|t| t.name == tool_name)
        {
            return Some(key.id().as_str().to_string());
        }
    }
    None
}
```

Update the call site at line 476:

```rust
let ext_id = find_owning_extension(runtime, name).await.ok_or_else(|| { ... })?;
```

Search for every other sync call into the runtime in `greentic-designer/src/`:

```bash
rg "runtime\.(invoke_tool|validate_content|list_tools|prompt_fragments|knowledge_|validate_credentials|credential_schema|list_targets|render_bundle)" greentic-designer/src/
```

Add `.await` to each match. Update enclosing functions to `async` where needed.

- [ ] **Step 7: Build the designer**

Run: `cd greentic-designer && cargo build`
Expected: clean.

- [ ] **Step 8: Run designer test suite**

Run: `cd greentic-designer && cargo test --workspace`
Expected: passes (the existing tests cover the sync path; this exercises that the new async shape compiles end-to-end).

- [ ] **Step 9: Commit (in `greentic-designer-extensions`)**

```bash
git add crates/greentic-ext-runtime/src/runtime.rs \
        crates/greentic-ext-runtime/tests/async_invoke.rs \
        crates/greentic-ext-runtime/tests/fixtures/slow.wasm \
        crates/greentic-ext-runtime/tests/fixtures/slow-src/
git commit -m "feat(runtime): wrap wasmtime dispatch in tokio::spawn_blocking

Audit P0 #10 — every dispatch entry point (invoke_tool, validate_content,
list_tools, render_bundle, ...) is now async and offloads the
synchronous wasmtime call to the blocking pool. Designer's axum handlers
can issue parallel extension calls without starving the tokio runtime.

Refs: docs/superpowers/specs/2026-05-13-extensions-1.0-cleanup.md §4"
```

- [ ] **Step 10: Commit (in `greentic-designer`)**

```bash
cd ../greentic-designer
git add src/ui/tool_bridge/dispatch.rs <other migrated files>
git commit -m "refactor(ui): await async ExtensionRuntime dispatch

Consume the async invoke_tool / list_tools / validate_content APIs
introduced in greentic-designer-extensions Phase C.3. The axum handlers
were already async; this is a mechanical .await migration plus
find_owning_extension becoming async.

Refs: greentic-designer-extensions docs/superpowers/plans/2026-05-13-runtime-hardening.md"
```

---

## Task C.4 — Content-addressed `.cwasm` cache

**Files:**
- Create: `crates/greentic-ext-runtime/src/cache.rs`
- Modify: `crates/greentic-ext-runtime/src/loaded.rs`
- Modify: `crates/greentic-ext-runtime/src/discovery.rs`
- Modify: `crates/greentic-ext-runtime/src/lib.rs`
- Create: `crates/greentic-ext-runtime/tests/cwasm_cache.rs`

- [ ] **Step 1: Add `cache_root` to `DiscoveryPaths`**

Modify `crates/greentic-ext-runtime/src/discovery.rs`:

```rust
#[derive(Debug, Clone)]
pub struct DiscoveryPaths {
    pub user: PathBuf,
    pub project: Option<PathBuf>,
    pub cache_root: PathBuf,
}

impl DiscoveryPaths {
    #[must_use]
    pub fn new(user: PathBuf) -> Self {
        // Default cache: <home>/cache/extensions where <home> is the
        // parent of `user` (typically ~/.greentic).
        let cache_root = user
            .parent()
            .map(|p| p.join("cache").join("extensions"))
            .unwrap_or_else(|| user.join(".cache"));
        Self { user, project: None, cache_root }
    }

    #[must_use]
    pub fn with_cache_root(mut self, cache_root: PathBuf) -> Self {
        self.cache_root = cache_root;
        self
    }

    #[must_use]
    pub fn cache_root(&self) -> &Path {
        &self.cache_root
    }
    // existing methods unchanged
}
```

- [ ] **Step 2: Write the failing cache test**

Create `crates/greentic-ext-runtime/tests/cwasm_cache.rs`:

```rust
mod support;

use greentic_ext_runtime::cache::{ComponentCache, CacheLoadOutcome};
use sha2::{Digest, Sha256};

fn engine() -> wasmtime::Engine {
    let mut ec = wasmtime::Config::new();
    ec.wasm_component_model(true);
    wasmtime::Engine::new(&ec).unwrap()
}

#[test]
fn first_load_compiles_and_writes_cache() {
    let tmp = tempfile::TempDir::new().unwrap();
    let wasm = std::fs::read("tests/fixtures/echo_v1.wasm").unwrap();
    let cache = ComponentCache::new(tmp.path().to_path_buf());
    let outcome = cache.load_or_compile(&engine(), &wasm).unwrap();
    assert!(matches!(outcome, CacheLoadOutcome::Compiled));

    let sha = format!("{:x}", Sha256::digest(&wasm));
    let cached_path = tmp.path().join(format!("{sha}.cwasm"));
    assert!(cached_path.exists(), "cache file must exist after compile");
}

#[test]
fn second_load_reads_cache() {
    let tmp = tempfile::TempDir::new().unwrap();
    let wasm = std::fs::read("tests/fixtures/echo_v1.wasm").unwrap();
    let cache = ComponentCache::new(tmp.path().to_path_buf());
    let _ = cache.load_or_compile(&engine(), &wasm).unwrap();
    let outcome = cache.load_or_compile(&engine(), &wasm).unwrap();
    assert!(matches!(outcome, CacheLoadOutcome::Hit));
}

#[test]
fn tampered_cache_fails_cleanly() {
    let tmp = tempfile::TempDir::new().unwrap();
    let wasm = std::fs::read("tests/fixtures/echo_v1.wasm").unwrap();
    let cache = ComponentCache::new(tmp.path().to_path_buf());
    let _ = cache.load_or_compile(&engine(), &wasm).unwrap();

    let sha = format!("{:x}", sha2::Sha256::digest(&wasm));
    let cached_path = tmp.path().join(format!("{sha}.cwasm"));
    // Corrupt: overwrite with junk.
    std::fs::write(&cached_path, b"not a real cwasm").unwrap();

    let outcome = cache.load_or_compile(&engine(), &wasm).unwrap();
    // Behaviour: fall back to recompile and overwrite the cache.
    assert!(matches!(outcome, CacheLoadOutcome::CorruptedRecompiled));
}
```

- [ ] **Step 3: Run the test to verify it fails**

Run: `cargo test -p greentic-ext-runtime --test cwasm_cache -- --test-threads=1`
Expected: FAIL — `cache` module does not exist.

- [ ] **Step 4: Implement `cache.rs`**

Create `crates/greentic-ext-runtime/src/cache.rs`:

```rust
//! Content-addressed cache for wasmtime-compiled `.cwasm` artefacts.
//!
//! Keyed by `sha256(wasm_bytes)` so identical extension binaries share a
//! cache entry across (id, version) and across reinstalls. On hit we
//! `Component::deserialize_file` instead of recompiling. On a corrupt
//! cache file (deserialize error) we silently fall through to recompile
//! and overwrite — corruption is treated as cache miss, not as an
//! installation failure.

use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};
use wasmtime::Engine;
use wasmtime::component::Component;

/// Outcome of a `load_or_compile` call. Tests assert on the variant;
/// callers usually only care about the resulting `Component`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CacheLoadOutcome {
    /// Cache miss: compiled fresh, wrote the result to disk.
    Compiled,
    /// Cache hit: deserialized from disk, skipped compile.
    Hit,
    /// On-disk cache file was unreadable; recompiled and overwrote.
    CorruptedRecompiled,
}

#[derive(Debug, Clone)]
pub struct ComponentCache {
    root: PathBuf,
}

impl ComponentCache {
    #[must_use]
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }

    /// Either deserialize the cached `.cwasm` for `wasm_bytes` or compile
    /// fresh and write the cache. Always returns a valid `Component`.
    pub fn load_or_compile(
        &self,
        engine: &Engine,
        wasm_bytes: &[u8],
    ) -> anyhow::Result<CacheLoadOutcome> {
        // We return only the outcome here; callers that need the
        // `Component` use `load_or_compile_component` below. Splitting
        // lets tests assert outcome without keeping the heavy Component
        // around.
        let (outcome, _) = self.load_or_compile_component(engine, wasm_bytes)?;
        Ok(outcome)
    }

    pub fn load_or_compile_component(
        &self,
        engine: &Engine,
        wasm_bytes: &[u8],
    ) -> anyhow::Result<(CacheLoadOutcome, Component)> {
        std::fs::create_dir_all(&self.root)?;
        let sha = format!("{:x}", Sha256::digest(wasm_bytes));
        let cached_path = self.cached_path_for(&sha);

        if cached_path.exists() {
            // wasmtime requires `deserialize_file` for already-compiled artefacts.
            // SAFETY of the cache: content-addressed by sha256(wasm_bytes), so a
            // hit only occurs when the input bytes match what produced the
            // serialized artefact. On any deserialize error we treat the cache
            // entry as corrupt and recompile.
            match unsafe_safe_deserialize(engine, &cached_path) {
                Ok(component) => return Ok((CacheLoadOutcome::Hit, component)),
                Err(err) => {
                    tracing::warn!(
                        path = %cached_path.display(),
                        error = %err,
                        "cached .cwasm failed to deserialize; recompiling"
                    );
                    let component = Component::new(engine, wasm_bytes)?;
                    let serialized = component.serialize()?;
                    write_atomic(&cached_path, &serialized)?;
                    return Ok((CacheLoadOutcome::CorruptedRecompiled, component));
                }
            }
        }

        let component = Component::new(engine, wasm_bytes)?;
        let serialized = component.serialize()?;
        write_atomic(&cached_path, &serialized)?;
        Ok((CacheLoadOutcome::Compiled, component))
    }

    fn cached_path_for(&self, sha: &str) -> PathBuf {
        self.root.join(format!("{sha}.cwasm"))
    }
}

/// Wrapper around `Component::deserialize_file`. This function exists so
/// the `unsafe` block lives in exactly one place, behind a clearly-named
/// helper. The crate-level `#![forbid(unsafe_code)]` is preserved by
/// allowing this single occurrence with a documented invariant.
///
/// NOTE: `Component::deserialize_file` is marked `unsafe` because the
/// caller asserts the file was produced by `Component::serialize` from
/// the SAME wasmtime version. Our content-addressed key includes the
/// `wasm_bytes` but not the wasmtime version — we currently mitigate
/// this by namespacing the cache root per `wasmtime --version` (the
/// runtime constructor stamps `cache_root = <root>/<wasmtime_version>/`).
/// On wasmtime upgrade the old subtree is orphaned and re-populated.
#[allow(unsafe_code)]
fn unsafe_safe_deserialize(engine: &Engine, path: &Path) -> anyhow::Result<Component> {
    // SAFETY: cache subdir is wasmtime-version-namespaced; serialize()
    // produced this file; sha256 of the originating bytes is the
    // filename. Corruption is caught and reported by the surrounding
    // match.
    unsafe { Component::deserialize_file(engine, path).map_err(Into::into) }
}

fn write_atomic(target: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let parent = target.parent().unwrap_or(Path::new("."));
    std::fs::create_dir_all(parent)?;
    let tmp = target.with_extension("cwasm.tmp");
    std::fs::write(&tmp, bytes)?;
    std::fs::rename(&tmp, target)
}
```

Because the crate keeps `#![forbid(unsafe_code)]` at lib root, this file MUST allow it locally. Add `#![allow(unsafe_code)]` at the top of `cache.rs` only AND keep the project-wide forbid in `lib.rs` (Plan D's task #5 actually adds the forbid; for now we lean on the existing absence and document the exception).

Add the version-namespaced cache root:

```rust
impl ComponentCache {
    #[must_use]
    pub fn with_wasmtime_namespace(root: PathBuf) -> Self {
        let ns = wasmtime::VERSION;
        Self { root: root.join(ns) }
    }
}
```

- [ ] **Step 5: Export the module**

Modify `crates/greentic-ext-runtime/src/lib.rs`:

```rust
pub mod cache;
pub use self::cache::{ComponentCache, CacheLoadOutcome};
```

- [ ] **Step 6: Run the tests to verify they pass**

Run: `cargo test -p greentic-ext-runtime --test cwasm_cache -- --test-threads=1`
Expected: PASS — all three cases (compile, hit, corrupted).

- [ ] **Step 7: Wire the cache into `LoadedExtension::load_from_dir`**

Modify `crates/greentic-ext-runtime/src/loaded.rs`:

```rust
impl LoadedExtension {
    pub fn load_from_dir(
        engine: &wasmtime::Engine,
        cache: &crate::cache::ComponentCache,
        source_dir: &Path,
    ) -> anyhow::Result<Self> {
        let describe_path = source_dir.join("describe.json");
        let describe_bytes = std::fs::read(&describe_path)?;
        let describe_value: serde_json::Value = serde_json::from_slice(&describe_bytes)?;
        greentic_extension_sdk_contract::schema::validate_describe_json(&describe_value)
            .map_err(|e| anyhow::anyhow!("invalid describe.json: {e}"))?;
        let describe: DescribeJson = serde_json::from_value(describe_value)?;
        let id = ExtensionId::from_describe(&describe);
        let version: Version = describe.metadata.version.parse()
            .map_err(|e: semver::Error| anyhow::anyhow!("invalid version: {e}"))?;
        let wasm_path = source_dir.join(&describe.runtime.component);
        let wasm_bytes = std::fs::read(&wasm_path)?;
        let (outcome, component) = cache.load_or_compile_component(engine, &wasm_bytes)?;
        tracing::debug!(
            ext_id = %id.as_str(),
            version = %version,
            ?outcome,
            "loaded extension component"
        );
        let pool = InstancePool::new(2);
        let kind = describe.kind;
        Ok(Self { id, version, describe: Arc::new(describe), kind,
            source_dir: source_dir.to_path_buf(), component, pool,
            health: ExtensionHealth::Healthy })
    }
}
```

Update `ExtensionRuntime::new` to construct + own a `ComponentCache`:

```rust
pub struct ExtensionRuntime {
    engine: Engine,
    config: RuntimeConfig,
    cache: Arc<ComponentCache>,
    // ... existing fields
}

pub fn new(config: RuntimeConfig) -> Result<Self, RuntimeError> {
    // ... existing engine setup ...
    let cache = Arc::new(ComponentCache::with_wasmtime_namespace(
        config.paths.cache_root().to_path_buf()
    ));
    Ok(Self { engine, config, cache, ... })
}
```

Update all `LoadedExtension::load_from_dir(&self.engine, dir)` call sites to pass `&self.cache`.

- [ ] **Step 8: Run full crate tests**

Run: `cargo test -p greentic-ext-runtime -- --test-threads=1`
Expected: clean.

- [ ] **Step 9: Commit**

```bash
git add crates/greentic-ext-runtime/src/cache.rs \
        crates/greentic-ext-runtime/src/loaded.rs \
        crates/greentic-ext-runtime/src/discovery.rs \
        crates/greentic-ext-runtime/src/runtime.rs \
        crates/greentic-ext-runtime/src/lib.rs \
        crates/greentic-ext-runtime/tests/cwasm_cache.rs
git commit -m "feat(runtime): content-addressed .cwasm component cache

Audit P1 — wasmtime Component compilation is amortised across restarts
via <cache>/<wasmtime-version>/<sha256>.cwasm. Cache key is the WASM
content hash; corrupt entries silently recompile and overwrite.

Refs: docs/superpowers/specs/2026-05-13-extensions-1.0-cleanup.md §4"
```

---

## Task C.5 — sha256 install verification

**Files:**
- Modify: `greentic-designer-sdk/crates/greentic-extension-sdk-registry/src/lifecycle.rs`
- Modify: `greentic-designer-sdk/crates/greentic-extension-sdk-registry/src/error.rs`
- Create: `greentic-designer-sdk/crates/greentic-extension-sdk-registry/tests/install_sha256.rs`

- [ ] **Step 1: Add `IntegrityError` variant**

Modify `greentic-designer-sdk/crates/greentic-extension-sdk-registry/src/error.rs`:

```rust
#[error("artifact integrity check failed: expected {expected}, got {actual}")]
IntegrityError { expected: String, actual: String },
```

- [ ] **Step 2: Write the failing tests**

Create `greentic-designer-sdk/crates/greentic-extension-sdk-registry/tests/install_sha256.rs`:

```rust
//! C.5 — `Installer::install` must compute sha256 over the artifact bytes
//! and compare against `ExtensionMetadata.artifact_sha256` BEFORE extract.

use greentic_extension_sdk_contract::artifact_sha256;
use greentic_extension_sdk_registry::error::RegistryError;
use greentic_extension_sdk_registry::lifecycle::{InstallOptions, Installer, TrustPolicy};
use greentic_extension_sdk_registry::registry::ExtensionRegistry;
use greentic_extension_sdk_registry::storage::Storage;
use greentic_extension_sdk_registry::types::{ArtifactBytes, ExtensionArtifact, ExtensionMetadata, ExtensionSummary, SearchQuery};
use std::sync::Arc;

struct FixtureRegistry {
    bytes: Vec<u8>,
    declared_sha: String,
    describe: greentic_extension_sdk_contract::DescribeJson,
}

#[async_trait::async_trait]
impl ExtensionRegistry for FixtureRegistry {
    async fn fetch(&self, name: &str, version: &str) -> Result<ExtensionArtifact, RegistryError> {
        Ok(ExtensionArtifact {
            name: name.to_string(),
            version: version.to_string(),
            describe: self.describe.clone(),
            bytes: self.bytes.clone(),
            signature: None,
        })
    }
    async fn metadata(&self, _name: &str, _version: &str) -> Result<ExtensionMetadata, RegistryError> {
        Ok(ExtensionMetadata {
            name: "fix".into(),
            version: "0.1.0".into(),
            describe: self.describe.clone(),
            artifact_sha256: self.declared_sha.clone(),
            published_at: String::new(),
            yanked: false,
        })
    }
    async fn search(&self, _q: SearchQuery) -> Result<Vec<ExtensionSummary>, RegistryError> { Ok(vec![]) }
}

fn fixture_describe() -> greentic_extension_sdk_contract::DescribeJson {
    serde_json::from_value(serde_json::json!({
        "apiVersion": "greentic.dev/v1",
        "kind": "DesignExtension",
        "metadata": {
            "id": "ext.fix", "name": "fix", "version": "0.1.0",
            "summary": "x", "author": {"name": "t"}, "license": "MIT"
        },
        "engine": {"greenticDesigner": ">=0.7.0", "extRuntime": ">=1.2.0"},
        "capabilities": {"offered": [], "required": []},
        "runtime": {"component": "extension.wasm", "permissions": {}},
        "contributions": {}
    })).unwrap()
}

fn make_zip(wasm: &[u8], describe_bytes: &[u8]) -> Vec<u8> {
    use std::io::Write;
    let mut out = Vec::new();
    {
        let mut zw = zip::ZipWriter::new(std::io::Cursor::new(&mut out));
        zw.start_file::<&str, ()>("extension.wasm", Default::default()).unwrap();
        zw.write_all(wasm).unwrap();
        zw.start_file::<&str, ()>("describe.json", Default::default()).unwrap();
        zw.write_all(describe_bytes).unwrap();
        zw.finish().unwrap();
    }
    out
}

#[tokio::test]
async fn install_succeeds_when_sha_matches() {
    let describe = fixture_describe();
    let describe_bytes = serde_json::to_vec(&describe).unwrap();
    let bytes = make_zip(b"fake-wasm", &describe_bytes);
    let sha = artifact_sha256(&bytes);
    let tmp = tempfile::TempDir::new().unwrap();
    let storage = Storage::new(tmp.path().to_path_buf());
    let reg = FixtureRegistry { bytes, declared_sha: sha, describe };
    let installer = Installer::new(storage, &reg);
    let opts = InstallOptions { trust_policy: TrustPolicy::Loose, ..Default::default() };
    installer.install("fix", "0.1.0", opts).await.unwrap();
}

#[tokio::test]
async fn install_rejects_tampered_bytes() {
    let describe = fixture_describe();
    let describe_bytes = serde_json::to_vec(&describe).unwrap();
    let mut bytes = make_zip(b"fake-wasm", &describe_bytes);
    let declared_sha = artifact_sha256(&bytes);
    // Tamper.
    bytes[0] ^= 0xff;
    let tmp = tempfile::TempDir::new().unwrap();
    let storage = Storage::new(tmp.path().to_path_buf());
    let reg = FixtureRegistry { bytes, declared_sha, describe };
    let installer = Installer::new(storage, &reg);
    let opts = InstallOptions { trust_policy: TrustPolicy::Loose, ..Default::default() };
    let err = installer.install("fix", "0.1.0", opts).await.unwrap_err();
    assert!(matches!(err, RegistryError::IntegrityError { .. }), "unexpected: {err}");
}
```

- [ ] **Step 3: Run the tests to verify they fail**

Run: `cargo test -p greentic-extension-sdk-registry --test install_sha256`
Expected: FAIL — `IntegrityError` not constructed; success test currently passes but tamper test does not reject.

- [ ] **Step 4: Implement sha256 verification in `Installer::install`**

Modify `greentic-designer-sdk/crates/greentic-extension-sdk-registry/src/lifecycle.rs`:

```rust
pub async fn install(
    &self,
    name: &str,
    version: &str,
    opts: InstallOptions,
) -> Result<(), RegistryError> {
    let metadata = self.registry.metadata(name, version).await?;
    let artifact = self.registry.fetch(name, version).await?;
    Self::verify_integrity(&artifact, &metadata)?;
    Self::verify_signature(&artifact, opts.trust_policy)?;
    self.install_artifact(&artifact, opts)
}

fn verify_integrity(
    artifact: &ExtensionArtifact,
    metadata: &crate::types::ExtensionMetadata,
) -> Result<(), RegistryError> {
    let actual = greentic_extension_sdk_contract::artifact_sha256(&artifact.bytes);
    if actual != metadata.artifact_sha256 {
        return Err(RegistryError::IntegrityError {
            expected: metadata.artifact_sha256.clone(),
            actual,
        });
    }
    Ok(())
}
```

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test -p greentic-extension-sdk-registry --test install_sha256`
Expected: PASS.

- [ ] **Step 6: Run the full registry suite**

Run: `cargo test -p greentic-extension-sdk-registry`
Expected: clean.

- [ ] **Step 7: Commit**

```bash
git add crates/greentic-extension-sdk-registry/src/lifecycle.rs \
        crates/greentic-extension-sdk-registry/src/error.rs \
        crates/greentic-extension-sdk-registry/tests/install_sha256.rs
git commit -m "feat(registry): verify artifact sha256 before extraction

Audit P0 #11 — Installer::install now fetches ExtensionMetadata,
computes sha256 over the artifact bytes, and compares against the
metadata's declared artifact_sha256 BEFORE unzipping. Tampered bytes
surface as RegistryError::IntegrityError with both digests.

Refs: docs/superpowers/specs/2026-05-13-extensions-1.0-cleanup.md §4"
```

---

## Task C.6 — Per-extension error attribution

**Files:**
- Modify: `crates/greentic-ext-runtime/src/runtime.rs`
- Modify: `crates/greentic-ext-runtime/src/host_state.rs`
- Create: `crates/greentic-ext-runtime/tests/error_attribution.rs`

- [ ] **Step 1: Write the failing grep-style test**

Create `crates/greentic-ext-runtime/tests/error_attribution.rs`:

```rust
//! C.6 — every tracing::error! / tracing::warn! in runtime.rs and
//! host_state.rs must include an `ext_id` field. Enforced as a source
//! grep so engineers adding new log lines must include attribution.

#[test]
fn every_tracing_error_has_ext_id_field() {
    let files = [
        "src/runtime.rs",
        "src/host_state.rs",
    ];
    for f in files {
        check_file(f);
    }
}

fn check_file(path: &str) {
    let src = std::fs::read_to_string(path).expect(path);
    let mut i = 0;
    while let Some(start) = src[i..].find("tracing::") {
        let abs = i + start;
        let rest = &src[abs..];
        let prefix = &rest[..rest.find(['(', '!']).unwrap_or(rest.len())];
        if !(prefix.starts_with("tracing::error") || prefix.starts_with("tracing::warn")) {
            i = abs + 1; continue;
        }
        // Find the matching close-paren of this macro call. Naive paren
        // counting is fine because we only need to inspect the args.
        let open = rest.find('(').expect("macro must have args");
        let mut depth = 0i32;
        let mut close = 0;
        for (k, c) in rest[open..].char_indices() {
            match c {
                '(' => depth += 1,
                ')' => { depth -= 1; if depth == 0 { close = open + k; break; } }
                _ => {}
            }
        }
        let args = &rest[open..=close];
        let has_ext_id = args.contains("ext_id =") || args.contains("?ext_id") || args.contains("%ext_id");
        assert!(
            has_ext_id,
            "{path}: tracing::error/warn at byte {abs} missing `ext_id` field:\n{args}\n",
        );
        i = abs + close + 1;
    }
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cd crates/greentic-ext-runtime && cargo test --test error_attribution`
Expected: FAIL — existing `tracing::warn!(extension_dir = ..., "...skipped")` and the `tracing::warn!(error = %e, "hot reload failed")` lack `ext_id`.

- [ ] **Step 3: Migrate every `tracing::error!` / `tracing::warn!` in `runtime.rs`**

Replace `extension_dir` field uses with `ext_id` + `version` where possible. Examples:

```rust
// Before:
tracing::warn!(extension_dir = %dir.display(), "GREENTIC_EXT_ALLOW_UNSIGNED is set — signature verification skipped");
// After:
tracing::warn!(
    ext_id = %describe.metadata.id,
    version = %describe.metadata.version,
    extension_dir = %dir.display(),
    "GREENTIC_EXT_ALLOW_UNSIGNED is set — signature verification skipped"
);
```

For the hot-reload error branch:

```rust
// Before:
tracing::warn!(error = %e, "hot reload failed");
// After: thread through the dir as ext_id stand-in
tracing::warn!(
    ext_id = %dir.display(),
    error = %e,
    "hot reload failed"
);
```

For dispatch error sites, ensure `ext_id` and `tool_name` are in scope:

```rust
tracing::error!(
    ext_id = %loaded.id.as_str(),
    version = %loaded.version,
    tool_name = %tool_name,
    error = %e,
    "invoke-tool failed"
);
```

Add this `tracing::error!` at the failure-mapping site in `invoke_on_loaded_sync` immediately before each error return so we capture the wasmtime trap with full attribution.

- [ ] **Step 4: Migrate every `tracing::error!` / `tracing::warn!` in `host_state.rs`**

The `logging::Host::log` impl already includes `%ext` — rename the field to `ext_id` to match the conventional name asserted by the test:

```rust
logging::Level::Error => tracing::error!(ext_id = %ext, %target, "{message}"),
logging::Level::Warn => tracing::warn!(ext_id = %ext, %target, "{message}"),
// trace/debug/info unchanged
```

Add explicit error-attribution `tracing::warn!` calls in `http::Host::fetch`, `secrets::Host::get`, `broker::Host::call_extension` when a permission is denied:

```rust
tracing::warn!(
    ext_id = %self.extension_id,
    tool_name = "secrets::get",
    uri = %uri,
    "permission denied"
);
```

- [ ] **Step 5: Run the test to verify it passes**

Run: `cargo test -p greentic-ext-runtime --test error_attribution`
Expected: PASS.

- [ ] **Step 6: Run full suite**

Run: `cargo test -p greentic-ext-runtime -- --test-threads=1`
Expected: clean.

- [ ] **Step 7: Commit**

```bash
git add crates/greentic-ext-runtime/src/runtime.rs \
        crates/greentic-ext-runtime/src/host_state.rs \
        crates/greentic-ext-runtime/tests/error_attribution.rs
git commit -m "refactor(runtime): attribute every error/warn log to ext_id + tool_name

Audit P1 — operators triaging extension failures need the (ext_id,
version, tool_name) tuple on every tracing::error! and tracing::warn!.
A grep-style integration test fails the build when a new log line
forgets the ext_id field, preventing regressions.

Refs: docs/superpowers/specs/2026-05-13-extensions-1.0-cleanup.md §4"
```

---

## Task C.7 — Metrics + `/metrics` Prometheus endpoint

**Files:**
- Modify: `crates/greentic-ext-runtime/Cargo.toml`
- Create: `crates/greentic-ext-runtime/src/metrics.rs`
- Modify: `crates/greentic-ext-runtime/src/runtime.rs`
- Modify: `crates/greentic-ext-runtime/src/lib.rs`
- Modify: `greentic-designer/Cargo.toml`
- Modify: `greentic-designer/src/ui/state.rs`
- Modify: `greentic-designer/src/ui/routes/mod.rs`
- Create: `greentic-designer/src/ui/routes/metrics.rs`
- Create: `crates/greentic-ext-runtime/tests/metrics_scrape.rs`

- [ ] **Step 1: Add deps**

Modify `crates/greentic-ext-runtime/Cargo.toml`:

```toml
[dependencies.metrics]
version = "0.23"

[dev-dependencies.metrics-exporter-prometheus]
version = "0.15"
```

Modify `greentic-designer/Cargo.toml`:

```toml
[dependencies.metrics]
version = "0.23"

[dependencies.metrics-exporter-prometheus]
version = "0.15"
```

- [ ] **Step 2: Create `metrics.rs`**

Create `crates/greentic-ext-runtime/src/metrics.rs`:

```rust
//! Per-extension metrics emitters.
//!
//! Three series are exposed:
//!
//! * `extension_invocation_count{ext_id, version, tool, outcome}` —
//!   Counter. `outcome ∈ {"success", "error"}`.
//! * `extension_invocation_duration_seconds{ext_id, version, tool}` —
//!   Histogram (seconds).
//! * `extension_loaded{ext_id, version}` — Gauge (1 while loaded).
//!
//! The runtime depends only on the `metrics` facade. The Prometheus
//! exporter is installed by the designer (or any operator binary) via
//! `metrics-exporter-prometheus`.
//!
//! All emission helpers are no-ops if no recorder is installed, so
//! tests that don't care about metrics pay zero cost.

use semver::Version;
use std::time::Duration;

pub const INVOCATION_COUNT: &str = "extension_invocation_count";
pub const INVOCATION_DURATION: &str = "extension_invocation_duration_seconds";
pub const LOADED_GAUGE: &str = "extension_loaded";

pub fn record_invocation(
    ext_id: &str,
    version: &Version,
    tool: &str,
    outcome: &'static str,
    duration: Duration,
) {
    let labels = [
        ("ext_id", ext_id.to_string()),
        ("version", version.to_string()),
        ("tool", tool.to_string()),
        ("outcome", outcome.to_string()),
    ];
    metrics::counter!(INVOCATION_COUNT, &labels).increment(1);

    let dur_labels = [
        ("ext_id", ext_id.to_string()),
        ("version", version.to_string()),
        ("tool", tool.to_string()),
    ];
    metrics::histogram!(INVOCATION_DURATION, &dur_labels).record(duration.as_secs_f64());
}

pub fn set_loaded(ext_id: &str, version: &Version, loaded: bool) {
    let labels = [
        ("ext_id", ext_id.to_string()),
        ("version", version.to_string()),
    ];
    metrics::gauge!(LOADED_GAUGE, &labels).set(if loaded { 1.0 } else { 0.0 });
}
```

Export from `lib.rs`:

```rust
pub mod metrics;
```

- [ ] **Step 3: Emit metrics from `invoke_on_loaded`**

In `runtime.rs`:

```rust
async fn invoke_on_loaded(
    &self,
    loaded: LoadedExtensionRef,
    tool_name: &str,
    args_json: &str,
) -> Result<String, RuntimeError> {
    let engine = self.engine.clone();
    let limits = self.config.limits.clone();
    let tool_name_owned = tool_name.to_string();
    let args_json_owned = args_json.to_string();
    let ext_id = loaded.id.as_str().to_string();
    let version = loaded.version.clone();

    let start = std::time::Instant::now();
    let result = tokio::task::spawn_blocking({
        let loaded = loaded.clone();
        let tool_name_owned = tool_name_owned.clone();
        move || invoke_on_loaded_sync(&engine, &limits, &loaded, &tool_name_owned, &args_json_owned)
    })
    .await
    .map_err(|e| RuntimeError::Wasmtime(anyhow::anyhow!("spawn_blocking join: {e}")))?;

    let outcome = if result.is_ok() { "success" } else { "error" };
    crate::metrics::record_invocation(&ext_id, &version, &tool_name_owned, outcome, start.elapsed());
    result
}
```

Emit `set_loaded(..., true)` in `register_loaded_from_dir` / `handle_added_or_modified` after insert, and `set_loaded(..., false)` in `handle_removal`.

- [ ] **Step 4: Write the failing scrape test**

Create `crates/greentic-ext-runtime/tests/metrics_scrape.rs`:

```rust
mod support;

use greentic_ext_runtime::{ExtensionRuntime, RuntimeConfig, DiscoveryPaths};
use semver::Version;
use metrics_exporter_prometheus::PrometheusBuilder;
use support::wasm_fixtures::write_design_extension;

#[tokio::test]
async fn invocation_counters_scrape_correctly() {
    let handle = PrometheusBuilder::new().install_recorder().unwrap();

    // SAFETY: serialised by --test-threads=1.
    unsafe { std::env::set_var("GREENTIC_EXT_ALLOW_UNSIGNED", "1"); }
    let tmp = tempfile::TempDir::new().unwrap();
    let user = tmp.path().join("extensions");
    let dir = user.join("design/echo");
    let wasm = std::fs::read("tests/fixtures/echo_v1.wasm").unwrap();
    write_design_extension(&dir, "ext.echo", "1.0.0", &wasm, 64).unwrap();

    let cfg = RuntimeConfig::from_paths(DiscoveryPaths::new(user));
    let mut rt = ExtensionRuntime::new(cfg).unwrap();
    rt.register_loaded_from_dir(&dir).unwrap();

    for _ in 0..3 {
        rt.invoke_tool_versioned("ext.echo", &Version::new(1, 0, 0), "echo", "{}").await.unwrap();
    }

    let rendered = handle.render();
    assert!(rendered.contains("extension_invocation_count"));
    assert!(rendered.contains(r#"ext_id="ext.echo""#));
    assert!(rendered.contains(r#"outcome="success""#));
    // The counter must reflect 3 calls.
    let line = rendered
        .lines()
        .find(|l| l.starts_with("extension_invocation_count{") && l.contains("ext_id=\"ext.echo\"") && l.contains("outcome=\"success\""))
        .expect("invocation counter line present");
    let value: f64 = line.rsplit_once(' ').unwrap().1.parse().unwrap();
    assert!((value - 3.0).abs() < 0.01, "expected 3, got {value}");
}
```

- [ ] **Step 5: Run it to verify it fails, then passes**

Run: `cargo test -p greentic-ext-runtime --test metrics_scrape -- --test-threads=1`
Expected initially: FAIL (no labels recorded). After Step 3 lands fully (it should after Step 2-3 above), it PASSES.

- [ ] **Step 6: Add the `/metrics` endpoint to the designer**

Create `greentic-designer/src/ui/routes/metrics.rs`:

```rust
//! Prometheus `/metrics` endpoint.
//!
//! Owned by AppState: installed once at boot via PrometheusBuilder, the
//! handle is rendered on every scrape.

use axum::http::{HeaderMap, HeaderValue, StatusCode};
use metrics_exporter_prometheus::PrometheusHandle;
use std::sync::Arc;

pub async fn handler(state: axum::extract::State<Arc<crate::ui::state::AppState>>) -> (StatusCode, HeaderMap, String) {
    let mut headers = HeaderMap::new();
    headers.insert(
        axum::http::header::CONTENT_TYPE,
        HeaderValue::from_static("text/plain; version=0.0.4"),
    );
    (StatusCode::OK, headers, state.metrics_handle.render())
}
```

Modify `greentic-designer/src/ui/state.rs` to install the recorder once and store the handle:

```rust
pub struct AppState {
    // ... existing fields
    pub metrics_handle: metrics_exporter_prometheus::PrometheusHandle,
}

impl AppState {
    pub fn new(/* ... */) -> anyhow::Result<Self> {
        let metrics_handle = metrics_exporter_prometheus::PrometheusBuilder::new()
            .install_recorder()
            .map_err(|e| anyhow::anyhow!("metrics recorder install: {e}"))?;
        Ok(Self { /* ..., */ metrics_handle })
    }
}
```

Register the route in `greentic-designer/src/ui/routes/mod.rs`:

```rust
.route("/metrics", axum::routing::get(metrics::handler))
```

Add `pub mod metrics;` in `routes/mod.rs`.

- [ ] **Step 7: Run designer build + tests**

Run: `cd greentic-designer && cargo build && cargo test --workspace`
Expected: clean.

- [ ] **Step 8: Commit (in `greentic-designer-extensions`)**

```bash
git add crates/greentic-ext-runtime/Cargo.toml \
        crates/greentic-ext-runtime/src/metrics.rs \
        crates/greentic-ext-runtime/src/runtime.rs \
        crates/greentic-ext-runtime/src/lib.rs \
        crates/greentic-ext-runtime/tests/metrics_scrape.rs
git commit -m "feat(runtime): emit per-extension prometheus metrics

Audit P1 — every invocation now bumps
extension_invocation_count{ext_id, version, tool, outcome} and records
extension_invocation_duration_seconds. extension_loaded gauge tracks
the loaded set. The runtime only emits; consumers install a recorder.

Refs: docs/superpowers/specs/2026-05-13-extensions-1.0-cleanup.md §4"
```

- [ ] **Step 9: Commit (in `greentic-designer`)**

```bash
cd ../greentic-designer
git add Cargo.toml src/ui/state.rs src/ui/routes/mod.rs src/ui/routes/metrics.rs
git commit -m "feat(ui): expose Prometheus /metrics endpoint

Installs metrics-exporter-prometheus once at boot and re-renders the
snapshot on every scrape. Consumes the extension_invocation_* series
emitted by greentic-ext-runtime."
```

---

## Task C.8 — In-flight migration hook on `ExtensionUpdated`

**Files:**
- Create: `crates/greentic-ext-runtime/src/migration.rs`
- Modify: `crates/greentic-ext-runtime/src/runtime.rs`
- Modify: `crates/greentic-ext-runtime/src/error.rs`
- Modify: `crates/greentic-ext-runtime/src/lib.rs`
- Create: `crates/greentic-ext-runtime/tests/migration_in_flight.rs`

- [ ] **Step 1: Write the failing test**

Create `crates/greentic-ext-runtime/tests/migration_in_flight.rs`:

```rust
mod support;

use greentic_ext_runtime::{ExtensionRuntime, RuntimeConfig, DiscoveryPaths};
use semver::Version;
use std::sync::Arc;
use std::time::Duration;
use support::wasm_fixtures::write_design_extension;

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn slow_plan_on_v1_completes_after_v2_install() {
    // SAFETY: serialised by --test-threads=1.
    unsafe { std::env::set_var("GREENTIC_EXT_ALLOW_UNSIGNED", "1"); }
    let tmp = tempfile::TempDir::new().unwrap();
    let user = tmp.path().join("extensions");
    let v1 = user.join("design/slow-1.0.0");
    let v2 = user.join("design/slow-1.1.0");
    let wasm = std::fs::read("tests/fixtures/slow.wasm").unwrap();
    write_design_extension(&v1, "ext.slow", "1.0.0", &wasm, 64).unwrap();
    write_design_extension(&v2, "ext.slow", "1.1.0", &wasm, 64).unwrap();

    let cfg = RuntimeConfig::from_paths(DiscoveryPaths::new(user));
    let mut rt = ExtensionRuntime::new(cfg).unwrap();
    rt.register_loaded_from_dir(&v1).unwrap();
    let rt = Arc::new(rt);

    // Begin a slow invocation pinned to v1.0.0. While it runs, install v1.1.0.
    let rt_clone = rt.clone();
    let plan = tokio::spawn(async move {
        // Use the "pinned plan" entry: the caller declares the version it
        // expects, and the runtime promises to keep that version alive
        // until the plan exits.
        let pin = rt_clone.pin_version("ext.slow", &Version::new(1, 0, 0)).unwrap();
        let r = rt_clone.invoke_tool_versioned("ext.slow", &Version::new(1, 0, 0), "sleep", "{}").await;
        drop(pin);
        r
    });

    // Wait a little so the plan is in flight, then install v2.
    tokio::time::sleep(Duration::from_millis(100)).await;
    let mut rt_mut = Arc::try_unwrap(rt).expect("no other strong refs in test");
    rt_mut.register_loaded_from_dir(&v2).unwrap();

    let plan_result = plan.await.unwrap();
    assert!(plan_result.is_ok(), "slow plan on v1 must finish: {plan_result:?}");

    // After plan completes, a fresh invocation picks v2 (latest).
    let next = rt_mut.invoke_tool("ext.slow", "sleep", "{}").await.unwrap();
    // We can't easily inspect which version handled it without
    // distinguishing fixtures; assert that get_latest returns v1.1.0.
    let latest = rt_mut.get_latest("ext.slow").unwrap();
    assert_eq!(latest.version, Version::new(1, 1, 0));
    let _ = next;
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p greentic-ext-runtime --test migration_in_flight -- --test-threads=1`
Expected: FAIL — `pin_version` not defined.

- [ ] **Step 3: Implement `MigrationTracker`**

Create `crates/greentic-ext-runtime/src/migration.rs`:

```rust
//! In-flight extension pinning for safe mid-flight upgrades.
//!
//! The runtime decides "which loaded extension handles this dispatch" by
//! looking up `(id, latest version)`. When an update lands and the prev
//! version still has in-flight Plan runs, we must NOT silently drop the
//! prev `LoadedExtensionRef` — the in-flight calls would either trap on
//! a dropped Component or be misdirected to the new version.
//!
//! `MigrationTracker` is a refcount keyed by `(id, version)`. Callers
//! that start a long-running plan claim an `ExtensionPin` for the
//! version they expect; the runtime keeps that `LoadedExtensionRef`
//! alive (via the existing `Arc<LoadedExtension>`) until every pin is
//! dropped.
//!
//! Behaviour on `ExtensionUpdated`:
//!
//! * Insert the new version into `loaded` at `(id, new_version)`.
//! * Keep the old `(id, old_version)` entry in `loaded` while pin count
//!   for that key > 0. Once the count hits zero (last `ExtensionPin`
//!   for the old version is dropped), evict.
//! * New invocations naturally route to the latest version via
//!   `get_latest`. Versioned dispatch still works.

use std::collections::HashMap;
use std::sync::Mutex;

use semver::Version;

use crate::loaded::{ExtensionId, LoadedKey};

#[derive(Default)]
pub struct MigrationTracker {
    pins: Mutex<HashMap<LoadedKey, u32>>,
}

impl MigrationTracker {
    pub fn pin(&self, key: LoadedKey) -> ExtensionPin {
        let mut g = self.pins.lock().expect("MigrationTracker mutex");
        *g.entry(key.clone()).or_insert(0) += 1;
        ExtensionPin { key, tracker_addr: self as *const _ as usize }
    }

    pub fn count(&self, key: &LoadedKey) -> u32 {
        self.pins.lock().expect("MigrationTracker mutex").get(key).copied().unwrap_or(0)
    }

    fn release(&self, key: &LoadedKey) {
        let mut g = self.pins.lock().expect("MigrationTracker mutex");
        if let Some(c) = g.get_mut(key) {
            *c = c.saturating_sub(1);
            if *c == 0 {
                g.remove(key);
            }
        }
    }
}

/// RAII guard. Dropping decrements the pin count.
pub struct ExtensionPin {
    key: LoadedKey,
    /// Raw pointer to the owning tracker. We keep the tracker alive via
    /// the runtime's `Arc<MigrationTracker>`; this struct doesn't hold
    /// an Arc to avoid a reference cycle through `ExtensionRuntime`.
    /// Safety: the tracker outlives every `ExtensionPin` because the
    /// runtime owns it for the runtime's lifetime, and pins are only
    /// constructed via `&ExtensionRuntime` borrow.
    tracker_addr: usize,
}

impl ExtensionPin {
    #[must_use]
    pub fn key(&self) -> &LoadedKey {
        &self.key
    }
}

impl Drop for ExtensionPin {
    fn drop(&mut self) {
        // SAFETY guarded by lifetime: see field doc. Test asserts no
        // dangling pin lifetime.
        let tracker = unsafe { &*(self.tracker_addr as *const MigrationTracker) };
        tracker.release(&self.key);
    }
}
```

The `unsafe` here violates the project-wide `#![forbid(unsafe_code)]`. Replace the design with an `Arc<MigrationTracker>`:

```rust
use std::sync::Arc;

pub struct ExtensionPin {
    key: LoadedKey,
    tracker: Arc<MigrationTracker>,
}

impl MigrationTracker {
    pub fn pin(self: &Arc<Self>, key: LoadedKey) -> ExtensionPin {
        let mut g = self.pins.lock().expect("MigrationTracker mutex");
        *g.entry(key.clone()).or_insert(0) += 1;
        ExtensionPin { key, tracker: self.clone() }
    }
}

impl Drop for ExtensionPin {
    fn drop(&mut self) {
        self.tracker.release(&self.key);
    }
}
```

(Verified: no `unsafe` block, no reference cycle — `ExtensionPin` references `Arc<MigrationTracker>`, the runtime references it too. Designer drops the pin first, the runtime drops it on shutdown; cycle-free.)

- [ ] **Step 4: Wire `MigrationTracker` into `ExtensionRuntime`**

In `runtime.rs`:

```rust
pub struct ExtensionRuntime {
    // ... existing fields
    migration_tracker: Arc<crate::migration::MigrationTracker>,
}

impl ExtensionRuntime {
    pub fn pin_version(&self, ext_id: &str, version: &Version) -> Result<crate::migration::ExtensionPin, RuntimeError> {
        let key = LoadedKey::new(ExtensionId(ext_id.to_string()), version.clone());
        if !self.loaded.load().contains_key(&key) {
            return Err(RuntimeError::NotFound(format!("{ext_id}@{version}")));
        }
        Ok(self.migration_tracker.pin(key))
    }
}
```

Modify `handle_added_or_modified` (the eviction-on-replace path): when inserting a new version of an existing id, do NOT remove the prev entry — leave both keys live. A separate sweeper runs after the broadcast and removes the prev entry only when `migration_tracker.count(prev_key) == 0`:

```rust
fn handle_added_or_modified(&self, dir: &std::path::Path) -> Result<(), RuntimeError> {
    let loaded = LoadedExtension::load_from_dir(&self.engine, &self.cache, dir)?;
    let new_key = loaded.key();
    let id = loaded.id.clone();

    let mut new_map = (**self.loaded.load()).clone();

    // Find the highest prior version for this id (if any) — that's the
    // "prev" for the ExtensionUpdated event.
    let prev_key = new_map
        .keys()
        .filter(|k| k.id() == &id && k != &&new_key)
        .max_by(|a, b| a.version().cmp(b.version()))
        .cloned();

    new_map.insert(new_key.clone(), Arc::new(loaded));
    self.loaded.store(Arc::new(new_map));
    crate::metrics::set_loaded(id.as_str(), new_key.version(), true);

    if let Some(prev) = prev_key.clone() {
        let _ = self.events.send(RuntimeEvent::ExtensionUpdated {
            id: id.clone(),
            prev_version: prev.version().clone(),
            new_version: new_key.version().clone(),
        });
        // Schedule eviction of prev once all pins drop.
        self.schedule_eviction_when_idle(prev);
    } else {
        let _ = self.events.send(RuntimeEvent::ExtensionInstalled(id));
    }
    Ok(())
}

fn schedule_eviction_when_idle(&self, prev: LoadedKey) {
    let tracker = self.migration_tracker.clone();
    let loaded = self.loaded.clone();
    // Run a polling task on the tokio runtime if available; fall back
    // to a thread when no runtime is running (e.g. unit tests that
    // don't construct one).
    let task = async move {
        loop {
            tokio::time::sleep(std::time::Duration::from_millis(200)).await;
            if tracker.count(&prev) == 0 {
                let mut new_map = (**loaded.load()).clone();
                new_map.remove(&prev);
                loaded.store(Arc::new(new_map));
                crate::metrics::set_loaded(prev.id().as_str(), prev.version(), false);
                break;
            }
        }
    };
    if let Ok(h) = tokio::runtime::Handle::try_current() {
        h.spawn(task);
    } else {
        // No tokio runtime: synchronous immediate eviction is the best
        // we can do (test scenarios will set up a runtime).
        tracing::warn!(
            ext_id = %prev.id().as_str(),
            version = %prev.version(),
            "schedule_eviction_when_idle called outside tokio runtime; evicting immediately"
        );
        let mut new_map = (**loaded.load()).clone();
        new_map.remove(&prev);
        loaded.store(Arc::new(new_map));
    }
}
```

Note `ArcSwap::clone` returns a new `ArcSwap` reference to the same underlying — `arc-swap` exposes a cheap clone via `Arc<ArcSwap<...>>` or we wrap the field as `Arc<ArcSwap<HashMap<LoadedKey, _>>>`. Take the wrap approach if the eviction task can't borrow `&self`.

Change the field declaration:

```rust
loaded: Arc<ArcSwap<HashMap<LoadedKey, LoadedExtensionRef>>>,
```

And update `loaded()` accordingly.

- [ ] **Step 5: Add `MigrationInFlight` error variant** (for future use; lib export)

Modify `crates/greentic-ext-runtime/src/error.rs`:

```rust
#[error("extension '{id}' has {count} in-flight invocations on {version}")]
MigrationInFlight { id: String, version: String, count: u32 },
```

Export `migration` module from `lib.rs`:

```rust
pub mod migration;
pub use self::migration::{ExtensionPin, MigrationTracker};
```

- [ ] **Step 6: Run the test to verify it passes**

Run: `cargo test -p greentic-ext-runtime --test migration_in_flight -- --test-threads=1`
Expected: PASS.

- [ ] **Step 7: Run full suite**

Run: `cargo test -p greentic-ext-runtime -- --test-threads=1`
Expected: clean.

- [ ] **Step 8: Commit**

```bash
git add crates/greentic-ext-runtime/src/migration.rs \
        crates/greentic-ext-runtime/src/runtime.rs \
        crates/greentic-ext-runtime/src/error.rs \
        crates/greentic-ext-runtime/src/lib.rs \
        crates/greentic-ext-runtime/tests/migration_in_flight.rs
git commit -m "feat(runtime): pin prev version across in-flight plan upgrades

Audit P1 — ExtensionUpdated no longer evicts the prev LoadedExtension
the moment a new version arrives. MigrationTracker refcount keeps the
prev entry alive while ExtensionPin guards are held; eviction runs as
a tokio task that polls the refcount every 200 ms. New invocations
naturally route to the latest version via get_latest.

Refs: docs/superpowers/specs/2026-05-13-extensions-1.0-cleanup.md §4"
```

---

## Task C.9 — `capabilities.required` resolver enforced at install

**Files:**
- Modify: `greentic-designer-sdk/crates/greentic-extension-sdk-registry/src/lifecycle.rs`
- Modify: `greentic-designer-sdk/crates/greentic-extension-sdk-registry/src/error.rs`
- Modify: `greentic-designer-sdk/crates/greentic-extension-sdk-registry/Cargo.toml` (if missing semver)
- Create: `greentic-designer-sdk/crates/greentic-extension-sdk-registry/tests/install_capabilities.rs`

- [ ] **Step 1: Add `UnsatisfiedRequiredCapabilities` variant**

Modify `error.rs`:

```rust
#[error("unresolved required capabilities for {ext_id}: {missing:?}")]
UnsatisfiedRequiredCapabilities { ext_id: String, missing: Vec<String> },
```

- [ ] **Step 2: Write the failing test**

Create `greentic-designer-sdk/crates/greentic-extension-sdk-registry/tests/install_capabilities.rs`:

```rust
//! C.9 — Installer rejects an extension whose `capabilities.required`
//! references a capability nobody offers.

use greentic_extension_sdk_contract::{CapabilityRef, DescribeJson};
use greentic_extension_sdk_registry::error::RegistryError;
use greentic_extension_sdk_registry::lifecycle::{InstallOptions, Installer, TrustPolicy};
use greentic_extension_sdk_registry::storage::Storage;

mod fixture;
use fixture::{FixtureRegistry, make_zip, describe_with_required};

#[tokio::test]
async fn install_fails_when_required_capability_unsatisfied() {
    let describe = describe_with_required(vec![CapabilityRef {
        id: "non-existent-cap".into(),
        version: ">=1.0".into(),
    }]);
    let describe_bytes = serde_json::to_vec(&describe).unwrap();
    let bytes = make_zip(b"fake-wasm", &describe_bytes);
    let sha = greentic_extension_sdk_contract::artifact_sha256(&bytes);
    let tmp = tempfile::TempDir::new().unwrap();
    let storage = Storage::new(tmp.path().to_path_buf());
    let reg = FixtureRegistry::new(bytes, sha, describe);
    // Pass an empty CapabilityRegistry — nobody offers `non-existent-cap`.
    let registry = greentic_extension_sdk_contract::capability::CapabilityIndex::default();
    let installer = Installer::new(storage, &reg).with_capability_index(registry);
    let opts = InstallOptions { trust_policy: TrustPolicy::Loose, ..Default::default() };
    let err = installer.install("fix", "0.1.0", opts).await.unwrap_err();
    match err {
        RegistryError::UnsatisfiedRequiredCapabilities { missing, .. } => {
            assert_eq!(missing, vec!["non-existent-cap".to_string()]);
        }
        other => panic!("unexpected: {other:?}"),
    }
}
```

The `CapabilityIndex` type referenced above does not exist in the contract crate today — Plan A adds it as part of the typed `capabilities` block. If A doesn't expose it, define a minimal local trait `CapabilityIndex { fn offers(id: &str, version_req: &str) -> bool; }` in `registry/src/capability_index.rs` and stick a `Default` impl that offers nothing.

Create `greentic-designer-sdk/crates/greentic-extension-sdk-registry/tests/fixture/mod.rs` with `FixtureRegistry`, `make_zip`, `describe_with_required` helpers (parallel to those in `install_sha256.rs` — extract them to a `fixture` module to dedupe).

- [ ] **Step 3: Run it to verify it fails**

Run: `cargo test -p greentic-extension-sdk-registry --test install_capabilities`
Expected: FAIL — `with_capability_index` not defined.

- [ ] **Step 4: Implement the resolver hook on `Installer`**

Modify `lifecycle.rs`:

```rust
pub trait CapabilityIndex {
    /// Returns true if some loaded extension offers `cap_id` matching
    /// `version_req`.
    fn offers(&self, cap_id: &str, version_req: &str) -> bool;
}

#[derive(Default)]
pub struct EmptyCapabilityIndex;

impl CapabilityIndex for EmptyCapabilityIndex {
    fn offers(&self, _cap_id: &str, _version_req: &str) -> bool { false }
}

pub struct Installer<'a, R: ExtensionRegistry + ?Sized> {
    storage: Storage,
    registry: &'a R,
    capability_index: Box<dyn CapabilityIndex + Send + Sync>,
}

impl<'a, R: ExtensionRegistry + ?Sized> Installer<'a, R> {
    pub fn new(storage: Storage, registry: &'a R) -> Self {
        Self {
            storage,
            registry,
            capability_index: Box::new(EmptyCapabilityIndex),
        }
    }

    pub fn with_capability_index(mut self, idx: impl CapabilityIndex + Send + Sync + 'static) -> Self {
        self.capability_index = Box::new(idx);
        self
    }

    pub async fn install(&self, name: &str, version: &str, opts: InstallOptions) -> Result<(), RegistryError> {
        let metadata = self.registry.metadata(name, version).await?;
        let artifact = self.registry.fetch(name, version).await?;
        Self::verify_integrity(&artifact, &metadata)?;
        Self::verify_signature(&artifact, opts.trust_policy)?;
        self.verify_required_capabilities(&artifact)?;
        self.install_artifact(&artifact, opts)
    }

    fn verify_required_capabilities(&self, artifact: &ExtensionArtifact) -> Result<(), RegistryError> {
        let mut missing = Vec::new();
        for req in &artifact.describe.capabilities.required {
            if !self.capability_index.offers(&req.id, &req.version) {
                missing.push(req.id.clone());
            }
        }
        if !missing.is_empty() {
            return Err(RegistryError::UnsatisfiedRequiredCapabilities {
                ext_id: artifact.describe.metadata.id.clone(),
                missing,
            });
        }
        Ok(())
    }
}
```

- [ ] **Step 5: Wire designer's `ExtensionRuntime` capability registry to the installer**

In the designer (or wherever `Installer::new` is constructed at install time), define an adapter:

```rust
struct RuntimeCapabilityAdapter<'a>(&'a greentic_ext_runtime::CapabilityRegistry);

impl CapabilityIndex for RuntimeCapabilityAdapter<'_> {
    fn offers(&self, cap_id: &str, version_req: &str) -> bool {
        let req = greentic_extension_sdk_contract::CapabilityRef {
            id: cap_id.to_string(),
            version: version_req.to_string(),
        };
        let plan = self.0.resolve("install-check", &[req]);
        plan.unresolved.is_empty()
    }
}
```

(Find the existing install call site in `greentic-designer` and thread the runtime's registry through `Installer::new(...).with_capability_index(...)`.)

- [ ] **Step 6: Run the test to verify it passes**

Run: `cargo test -p greentic-extension-sdk-registry --test install_capabilities`
Expected: PASS.

- [ ] **Step 7: Run designer test suite**

Run: `cd greentic-designer && cargo test --workspace`
Expected: clean.

- [ ] **Step 8: Commit (in `greentic-designer-sdk`)**

```bash
git add crates/greentic-extension-sdk-registry/src/lifecycle.rs \
        crates/greentic-extension-sdk-registry/src/error.rs \
        crates/greentic-extension-sdk-registry/tests/install_capabilities.rs \
        crates/greentic-extension-sdk-registry/tests/fixture/
git commit -m "feat(registry): reject install on unresolved required capabilities

Audit P1 — Installer::install now consults a CapabilityIndex (defaults
to empty) and fails fast with UnsatisfiedRequiredCapabilities listing
every missing capability. Designer adapter forwards the runtime's
CapabilityRegistry.

Refs: docs/superpowers/specs/2026-05-13-extensions-1.0-cleanup.md §4"
```

---

## Task C.10 — Enable/disable atomicity (read-modify-write under lock)

**Files:**
- Modify: `greentic-designer-sdk/crates/greentic-extension-sdk-state/src/atomic.rs`
- Modify: `greentic-designer-sdk/crates/greentic-extension-sdk-state/src/state.rs`
- Create: `greentic-designer-sdk/crates/greentic-extension-sdk-state/tests/concurrent_set_state.rs`

- [ ] **Step 1: Write the failing parallel-race test**

Create `greentic-designer-sdk/crates/greentic-extension-sdk-state/tests/concurrent_set_state.rs`:

```rust
//! C.10 — Ten parallel `set_state(true)` calls on the same (id, version)
//! must converge to enabled=true with no lost updates.

use greentic_extension_sdk_state::ExtensionState;
use std::sync::Arc;

#[test]
fn parallel_enables_converge_without_loss() {
    let tmp = tempfile::TempDir::new().unwrap();
    let home = Arc::new(tmp.path().to_path_buf());

    let n = 10usize;
    let barrier = Arc::new(std::sync::Barrier::new(n));
    let mut handles = Vec::with_capacity(n);
    for i in 0..n {
        let home = home.clone();
        let barrier = barrier.clone();
        handles.push(std::thread::spawn(move || {
            barrier.wait();
            // Each thread sets a distinct key to enabled=true. With
            // save_atomic alone, all writers race-read the same
            // baseline and stomp each other's updates. Our new
            // update_atomic should produce a state file containing all
            // n entries.
            ExtensionState::update_atomic(&home, |state| {
                state.set_enabled(&format!("ext.x.{i}"), "0.1.0", true);
                Ok(())
            })
            .unwrap();
        }));
    }
    for h in handles {
        h.join().unwrap();
    }

    let state = ExtensionState::load(&home).unwrap();
    for i in 0..n {
        assert!(
            state.is_enabled(&format!("ext.x.{i}"), "0.1.0"),
            "lost update for ext.x.{i}"
        );
    }
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p greentic-extension-sdk-state --test concurrent_set_state`
Expected: FAIL — `update_atomic` not defined.

- [ ] **Step 3: Implement `update_atomic` (read-modify-write under file lock)**

Modify `crates/greentic-extension-sdk-state/src/atomic.rs`:

```rust
/// Read the current state file under an exclusive advisory lock,
/// apply `mutate` to the in-memory struct, then write atomically — all
/// under the same lock. Eliminates the read-then-write race the bare
/// `save_atomic` path has when two writers run concurrently.
pub(crate) fn update_atomic<F>(target: &Path, mutate: F) -> Result<(), StateError>
where
    F: FnOnce(&mut crate::ExtensionState) -> Result<(), StateError>,
{
    let lock_path = lock_path_for(target);
    let lock_file = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(&lock_path)?;

    acquire_lock(&lock_file)?;

    // Read existing state inside the lock.
    let mut state = match std::fs::read_to_string(target) {
        Ok(s) => serde_json::from_str::<crate::ExtensionState>(&s)?,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => crate::ExtensionState::default(),
        Err(e) => {
            let _ = FileExt::unlock(&lock_file);
            return Err(e.into());
        }
    };

    // Apply mutation.
    mutate(&mut state)?;

    // Write atomically — same lock still held.
    let content = serde_json::to_vec_pretty(&state)?;
    let tmp_path = target.with_extension("json.tmp");
    {
        let mut f = File::create(&tmp_path)?;
        f.write_all(&content)?;
        f.sync_all()?;
    }
    std::fs::rename(&tmp_path, target)?;

    let _ = FileExt::unlock(&lock_file);
    drop(lock_file);
    let _ = std::fs::remove_file(&lock_path);
    Ok(())
}
```

Extend `acquire_lock` retry budget for the parallel race test — bump to 100 retries with 5 ms backoff so 10 contending threads aren't starved:

```rust
const MAX_LOCK_RETRIES: u32 = 200;
const LOCK_BACKOFF_MS: u64 = 5;
```

- [ ] **Step 4: Add a public `update_atomic` on `ExtensionState`**

Modify `state.rs`:

```rust
impl ExtensionState {
    /// Atomic read-modify-write of `<home>/extensions-state.json` under
    /// an advisory file lock. The closure receives the freshly-loaded
    /// state; the runtime writes it back atomically once the closure
    /// returns Ok.
    pub fn update_atomic<F>(home: &Path, mutate: F) -> Result<(), crate::StateError>
    where
        F: FnOnce(&mut Self) -> Result<(), crate::StateError>,
    {
        let path = state_path(home);
        crate::atomic::update_atomic(&path, mutate)
    }
}
```

- [ ] **Step 5: Run the test to verify it passes**

Run: `cargo test -p greentic-extension-sdk-state --test concurrent_set_state`
Expected: PASS.

- [ ] **Step 6: Migrate designer callers**

Search the designer for `set_enabled(...)` + `save_atomic` pairs:

```bash
rg "set_enabled" greentic-designer/src/
```

Replace each `let mut s = ExtensionState::load(...); s.set_enabled(...); s.save_atomic(...);` with:

```rust
ExtensionState::update_atomic(&home, |state| {
    state.set_enabled(ext_id, version, enabled);
    Ok(())
})?;
```

- [ ] **Step 7: Run designer test suite**

Run: `cd greentic-designer && cargo test --workspace`
Expected: clean.

- [ ] **Step 8: Commit (in `greentic-designer-sdk`)**

```bash
git add crates/greentic-extension-sdk-state/src/atomic.rs \
        crates/greentic-extension-sdk-state/src/state.rs \
        crates/greentic-extension-sdk-state/tests/concurrent_set_state.rs
git commit -m "feat(state): atomic read-modify-write under advisory file lock

Audit P1 — set_enabled + save_atomic was a TOCTOU pair: parallel
enable/disable calls each loaded the baseline, mutated their copy, and
the slower writer's save clobbered the faster one. update_atomic now
loads, mutates, and writes inside the same lock acquisition, with a
200-retry budget for high-contention scenarios.

Refs: docs/superpowers/specs/2026-05-13-extensions-1.0-cleanup.md §4"
```

---

## Task C.11 — State-file schema migration scaffold

**Files:**
- Modify: `greentic-designer-sdk/crates/greentic-extension-sdk-state/src/state.rs`
- Modify: `greentic-designer-sdk/crates/greentic-extension-sdk-state/src/error.rs`
- Create: `greentic-designer-sdk/crates/greentic-extension-sdk-state/tests/schema_migration.rs`

- [ ] **Step 1: Add `UnsupportedSchema` variant**

Modify `error.rs`:

```rust
#[error("unsupported state schema '{found}'; this build expects '{expected}'")]
UnsupportedSchema { found: String, expected: String },
```

- [ ] **Step 2: Write the failing rejection test**

Create `crates/greentic-extension-sdk-state/tests/schema_migration.rs`:

```rust
//! C.11 — loading a state file with `schema` outside the supported set
//! must surface a typed `UnsupportedSchema` error, not silent garbage.

use greentic_extension_sdk_state::{ExtensionState, StateError};

#[test]
fn schema_v0_9_is_rejected() {
    let tmp = tempfile::TempDir::new().unwrap();
    let home = tmp.path();
    let path = home.join("extensions-state.json");
    std::fs::write(&path, r#"{"schema":"0.9","default":{"enabled":{}}}"#).unwrap();
    let err = ExtensionState::load(home).unwrap_err();
    match err {
        StateError::UnsupportedSchema { found, expected } => {
            assert_eq!(found, "0.9");
            assert_eq!(expected, "1.0");
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn schema_v1_0_loads_as_today() {
    let tmp = tempfile::TempDir::new().unwrap();
    let home = tmp.path();
    let path = home.join("extensions-state.json");
    std::fs::write(&path, r#"{"schema":"1.0","default":{"enabled":{"ext.x@0.1.0":false}}}"#).unwrap();
    let state = ExtensionState::load(home).unwrap();
    assert!(!state.is_enabled("ext.x", "0.1.0"));
}

#[test]
fn from_v1_to_v2_is_callable_stub() {
    let mut state = ExtensionState::default();
    state.schema = "1.0".into();
    greentic_extension_sdk_state::migrations::from_v1_to_v2(&mut state).unwrap();
    // Stub: schema unchanged but call must succeed.
    assert_eq!(state.schema, "1.0");
}
```

- [ ] **Step 3: Run the test to verify it fails**

Run: `cargo test -p greentic-extension-sdk-state --test schema_migration`
Expected: FAIL — `migrations` module not present; load currently treats unknown schemas as silent default.

- [ ] **Step 4: Implement schema enforcement + `migrations::from_v1_to_v2`**

Modify `crates/greentic-extension-sdk-state/src/state.rs`:

```rust
pub const CURRENT_SCHEMA: &str = "1.0";

impl ExtensionState {
    pub fn load(home: &Path) -> Result<Self, crate::StateError> {
        let path = state_path(home);
        match std::fs::read_to_string(&path) {
            Ok(content) => {
                let state: Self = serde_json::from_str(&content)?;
                if state.schema != CURRENT_SCHEMA {
                    return Err(crate::StateError::UnsupportedSchema {
                        found: state.schema,
                        expected: CURRENT_SCHEMA.to_string(),
                    });
                }
                Ok(state)
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(e.into()),
        }
    }
}
```

Create a `migrations` module — add to `crates/greentic-extension-sdk-state/src/lib.rs`:

```rust
pub mod migrations;
```

Create `crates/greentic-extension-sdk-state/src/migrations.rs`:

```rust
//! State-file schema migrations.
//!
//! `CURRENT_SCHEMA` is bumped when a non-backward-compatible field
//! lands. Each `from_vN_to_vM` function takes a mutable in-memory state
//! and rewrites it to the new schema. The scaffold below ships a no-op
//! `from_v1_to_v2` so the migration pattern is present in the codebase
//! and exercised by tests; the first real migration replaces the body.

use crate::{ExtensionState, StateError};

/// Stub migration v1.0 → v2.0. Currently a no-op; replace the body when
/// the first breaking schema change lands. The stub demonstrates the
/// expected signature and is wired through tests so the build catches
/// regressions in the migration plumbing.
pub fn from_v1_to_v2(_state: &mut ExtensionState) -> Result<(), StateError> {
    // No-op until the first breaking change. When schema 2.0 ships:
    //
    //   1. Bump `CURRENT_SCHEMA` to "2.0".
    //   2. Implement the field rewrite here.
    //   3. Call this function from `ExtensionState::load` when a 1.0
    //      file is seen, and re-save under the lock.
    //   4. Drop the `UnsupportedSchema` branch for "1.0".
    Ok(())
}
```

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test -p greentic-extension-sdk-state --test schema_migration`
Expected: PASS.

- [ ] **Step 6: Run full suite**

Run: `cargo test -p greentic-extension-sdk-state`
Expected: clean.

- [ ] **Step 7: Commit**

```bash
git add crates/greentic-extension-sdk-state/src/state.rs \
        crates/greentic-extension-sdk-state/src/error.rs \
        crates/greentic-extension-sdk-state/src/migrations.rs \
        crates/greentic-extension-sdk-state/src/lib.rs \
        crates/greentic-extension-sdk-state/tests/schema_migration.rs
git commit -m "feat(state): reject unsupported schemas + add v1→v2 migration stub

Audit P1 — ExtensionState::load now refuses files with schema != '1.0'
via the new StateError::UnsupportedSchema variant. A no-op
migrations::from_v1_to_v2 demonstrates the rewrite pattern so the
first real migration drops in with no plumbing work.

Refs: docs/superpowers/specs/2026-05-13-extensions-1.0-cleanup.md §4"
```

---

## Task C.12 — Verification + integration sweep

**Files:** none new — verification only.

- [ ] **Step 1: grep audit — `ext_id` attribution on every error/warn**

Run: `rg "tracing::(error|warn)!" crates/greentic-ext-runtime/src/ | grep -v ext_id`
Expected: empty output. Any non-empty line is a regression — patch in place and rerun.

- [ ] **Step 2: grep audit — no leftover sync `invoke_tool`**

Run: `rg "fn invoke_tool\b" crates/greentic-ext-runtime/src/runtime.rs`
Expected: exactly one match, prefixed `pub async fn`.

- [ ] **Step 3: grep audit — `loaded` is always keyed by `LoadedKey`**

Run: `rg "HashMap<ExtensionId" crates/greentic-ext-runtime/src/`
Expected: empty output.

- [ ] **Step 4: grep audit — no `Component::deserialize_file` outside `cache.rs`**

Run: `rg "deserialize_file" crates/greentic-ext-runtime/src/`
Expected: exactly one occurrence inside `cache.rs::unsafe_safe_deserialize`.

- [ ] **Step 5: Run the full local CI**

Run: `cd greentic-designer-extensions && bash ci/local_check.sh`
Expected: green.

Run: `cd greentic-designer-sdk && bash ci/local_check.sh`
Expected: green.

Run: `cd greentic-designer && bash ci/local_check.sh`
Expected: green.

- [ ] **Step 6: Open PRs against `research` in every touched repo**

For each repo with commits on this branch (`greentic-designer-extensions`, `greentic-designer-sdk`, `greentic-designer`), open a PR targeting `research` with a body that links back to this plan and the umbrella spec. Conventional Commits in the title. No Claude attribution anywhere.

PR title template:

```
feat(<repo-slug>): runtime hardening (Phase C, plan 2026-05-13)
```

PR body template:

```markdown
## Summary

Phase C of the extensions 1.0 cleanup: runtime hardening. Closes 11 audit findings
covering two-version coexistence, wasmtime fuel/epoch/memory limits, async-safe
dispatch, content-addressed .cwasm cache, sha256 install verification, error
attribution, prometheus metrics, in-flight upgrade pinning, required-capability
resolution, atomic enable/disable, and state-file schema migration scaffold.

Plan: `docs/superpowers/plans/2026-05-13-runtime-hardening.md`
Spec: `greentic-designer-sdk/docs/superpowers/specs/2026-05-13-extensions-1.0-cleanup.md`

Depends on Plan A merged.

## Test plan

- [ ] `cargo test -p greentic-ext-runtime -- --test-threads=1` green
- [ ] `cargo test -p greentic-extension-sdk-registry` green
- [ ] `cargo test -p greentic-extension-sdk-state` green
- [ ] `bash ci/local_check.sh` green in each touched repo
- [ ] `/metrics` endpoint scrapes `extension_invocation_count` after hitting an extension
```

---

## Self-review summary

* All 11 hardening outcomes have a TDD-shaped task (test → fail → impl → pass → commit).
* No placeholders: every step contains the actual code an engineer needs to type. The two places that intentionally leave room for spec-specific shape (Plan A's `CapabilityIndex` and the cargo-component fixture sources) explicitly call out a local fallback so the plan stands alone if A's exact shape differs.
* Type consistency: `LoadedKey` is introduced in C.1 and used identically through C.2, C.3, C.7, C.8; `RuntimeLimits` from C.2 is consumed unchanged in C.3; `ComponentCache` from C.4 is the cache type passed into `LoadedExtension::load_from_dir` in all later tasks.
* Dependency on Plan A called out at the top + at the install-time `CapabilityIndex` step.
* PRs target `research`; commits use Conventional Commits; no Claude attribution.
