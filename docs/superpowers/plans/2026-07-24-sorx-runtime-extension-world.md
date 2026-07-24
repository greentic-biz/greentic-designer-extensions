# `sorx-runtime-extension` world + control/observe dispatch — Implementation Plan (Sub-A)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a `greentic:extension-sorx@0.1.0` WIT world (`control` + `observe`) and two sync dispatch methods on `ExtensionRuntime` to `greentic-ext-runtime`, so `greentic-sorx` (Sub-B) can run WASM extension packs through its `RuntimeExtensionAdapter` contract.

**Architecture:** Follow this crate's "Adding a new world" recipe (CLAUDE.md), mirroring the `render_bundle` / `list_tools` examples: vendor WIT → `bindgen!` sibling module → dispatch methods in an own `impl ExtensionRuntime` block → `RuntimeError::NotFound` smoke tests. JSON-string in/out; `result<string, string>` error ABI.

**Tech Stack:** Rust 1.95 (edition 2024), wasmtime 43 (`wasmtime::component::bindgen!`), wit-bindgen 0.41 (guest-only; not a host dep).

## Global Constraints

- Toolchain Rust 1.95.0, MSRV 1.94, edition 2024 (`rust-toolchain.toml` canonical).
- **wasmtime pinned at 43 / wit-bindgen 0.41 / wit-component 0.221 — DO NOT upgrade** (straggler; coordinate with `greentic-designer` before any bump). `greentic-extension-sdk-contract` pinned `>=1.1.0-dev, <1.2.0-0`.
- **Max 500 lines per source file** — put the new methods in their own `impl ExtensionRuntime` block; if `host_bindings.rs` or `runtime.rs` would exceed 500 lines, split.
- **English only**; Conventional Commits; **NO Claude co-authorship trailer on commits**.
- Git hooks are active (a pre-commit fmt+clippy runs on commit) — do NOT bypass with `--no-verify`.
- Feature branch → PR into **`develop`**; never push to `main`.
- Gate before pushing: `bash ci/local_check.sh` (runs `cargo fmt --all -- --check`; `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings`; `cargo test --workspace --all-features --locked`; release build). Builds are slow (~1-2 min incremental, cold much longer) — run in background and WAIT.
- Do NOT touch `discovery.rs` (no new discovery kind — sorx registers dirs explicitly in Sub-B) or the design/deploy/bundle/provider dispatch paths.

**Reference patterns (mirror, do not reinvent):**
- The `list_tools` method (`crates/greentic-ext-runtime/src/runtime.rs:512-561`) is the cleanest dispatch template: loaded-lookup → `build_store_and_instance` → resolve iface export index → `get_export_index` for the func → `get_typed_func` → sync `call` → map `RuntimeError`.
- The `bundle` bindgen submodule (`src/host_bindings.rs`, `pub mod bundle { bindgen!({ path: "wit", world: "greentic:extension-bundle/bundle-extension" }) }`).
- `RuntimeError` (`src/error.rs`): variants `NotFound(String)`, `Wasmtime(anyhow::Error)` (used via `.map_err(RuntimeError::Wasmtime)` and `RuntimeError::Wasmtime(anyhow::anyhow!(...))`).
- WIT deps live under `wit/deps/extension-<kind>/`; the host-import package `greentic:extension-host` (interfaces `logging`, `i18n`, `secrets`, `broker`, `http`, `llm`) is already vendored at `wit/deps/extension-host/`.

---

### Task 1: WIT world `greentic:extension-sorx@0.1.0`

**Files:**
- Create: `crates/greentic-ext-runtime/wit/deps/extension-sorx/extension-sorx.wit`

**Interfaces:**
- Produces: WIT package `greentic:extension-sorx@0.1.0`, world `sorx-runtime-extension` with exported interfaces `control` and `observe`, importing `greentic:extension-host/logging@0.1.0`.

- [ ] **Step 1: Write the WIT**

Create `crates/greentic-ext-runtime/wit/deps/extension-sorx/extension-sorx.wit`:

```wit
package greentic:extension-sorx@0.1.0;

/// Control hooks let an extension inspect and gate a SoRX invocation.
interface control {
  /// `hook` is one of: pre_call | post_call | pre_admin | post_admin.
  /// `binding-json` / `request-json` / `response-json` are the serde-JSON of
  /// SoRX's RuntimeExtensionBinding / request / optional response. Returns the
  /// JSON of a SoRX ControlDecision on Ok, or an error message on Err.
  control: func(
    hook: string,
    binding-json: string,
    request-json: string,
    response-json: option<string>,
  ) -> result<string, string>;
}

/// Observe hooks let an extension react to SoRX lifecycle events (side effects
/// only; no control over the invocation).
interface observe {
  /// `subscription` is one of: pre_call | post_call | call_failed | control_denied.
  observe: func(
    subscription: string,
    binding-json: string,
    event-json: string,
  ) -> result<_, string>;
}

world sorx-runtime-extension {
  // Minimal host surface: control/observe need only logging. The host linker
  // provides a superset (i18n/secrets/broker/http/llm); importing a subset is
  // compatible and keeps the operator-runtime extension surface small.
  import greentic:extension-host/logging@0.1.0;

  export control;
  export observe;
}
```

> Confirm the exact `logging` interface id/version by opening `wit/deps/extension-host/*.wit` and matching the `package greentic:extension-host@<ver>;` + `interface logging` names. If the vendored version differs from `@0.1.0`, use the vendored value.

- [ ] **Step 2: Verify the WIT parses**

Run the workspace's WIT lint / a build that resolves WIT:
Run: `cargo build -p greentic-ext-runtime 2>&1 | tail -20`
Expected: still compiles (the new WIT is not yet referenced by any `bindgen!`, so this only proves the file is syntactically discoverable if `_wit-lint` picks it up; if `_wit-lint` has an explicit run, use `cargo test -p _wit-lint`). If a lint step exists in `ci/`, run it. This step's real gate is Task 2 (bindgen compiles the world).

- [ ] **Step 3: Commit**

```bash
git add crates/greentic-ext-runtime/wit/deps/extension-sorx/extension-sorx.wit
git commit -m "feat(ext): add greentic:extension-sorx WIT world (control/observe)"
```

---

### Task 2: `bindgen!` module + build

**Files:**
- Modify: `crates/greentic-ext-runtime/src/host_bindings.rs` (add `pub mod sorx`)

**Interfaces:**
- Consumes: the WIT world from Task 1.
- Produces: `crate::host_bindings::sorx::*` generated bindings — `sorx::add_to_linker`-style import wiring for `logging`, and typed export accessors for `control`/`observe`.

- [ ] **Step 1: Add the bindgen submodule**

Append to `crates/greentic-ext-runtime/src/host_bindings.rs`, mirroring `pub mod bundle`:

```rust
// ---------------------------------------------------------------------------
// SoRX runtime-extension bindings
//
// Isolated submodule (same reason as deploy/bundle): the world shares
// `greentic:extension-host/*` types, so binding at the root would conflict.
// The world exports `control` and `observe` (the host calls them in
// `runtime::{control,observe}`); it imports only `greentic:extension-host/logging`.
// ---------------------------------------------------------------------------
pub mod sorx {
    wasmtime::component::bindgen!({
        path: "wit",
        world: "greentic:extension-sorx/sorx-runtime-extension",
    });
}
```

- [ ] **Step 2: Verify it compiles (proves the world + bindgen are valid)**

Run: `cargo build -p greentic-ext-runtime 2>&1 | tail -20`
Expected: PASS. A WIT/world mismatch surfaces here as a `bindgen!` macro error. If the linker import set complains that the component must import `logging`, that is expected only at instantiate time (Task 3), not at bindgen time.

- [ ] **Step 3: Commit**

```bash
git add crates/greentic-ext-runtime/src/host_bindings.rs
git commit -m "feat(ext): bindgen module for the sorx-runtime-extension world"
```

---

### Task 3: `control` / `observe` dispatch methods + smoke tests

**Files:**
- Modify: `crates/greentic-ext-runtime/src/runtime.rs` (new `impl ExtensionRuntime` block + `#[cfg(test)]` smoke tests)

**Interfaces:**
- Consumes: `crate::host_bindings::sorx` (Task 2); `RuntimeError` (`error.rs`); `LoadedExtension::build_store_and_instance`, `ExtensionId`, `self.engine`, `self.host_overrides`, the `self.loaded` `ArcSwap` map (all used by `list_tools`).
- Produces:
  - `pub fn control(&self, ext_id: &str, hook: &str, binding_json: &str, request_json: &str, response_json: Option<&str>) -> Result<String, RuntimeError>`
  - `pub fn observe(&self, ext_id: &str, subscription: &str, binding_json: &str, event_json: &str) -> Result<(), RuntimeError>`

- [ ] **Step 1: Write the failing smoke tests**

Find the `#[cfg(test)] mod tests` in `runtime.rs` and the helper the existing NotFound smoke tests use to build an empty-tempdir runtime (search for an existing test asserting `RuntimeError::NotFound`, e.g. around a `list_tools`/`invoke_tool` NotFound test — reuse its exact runtime-construction helper). Add:

```rust
    #[test]
    fn control_unknown_extension_is_not_found() {
        let runtime = /* same empty-tempdir runtime builder the other NotFound tests use */;
        let err = runtime
            .control("does.not.exist", "pre_call", "{}", "{}", None)
            .unwrap_err();
        assert!(matches!(err, RuntimeError::NotFound(id) if id == "does.not.exist"));
    }

    #[test]
    fn observe_unknown_extension_is_not_found() {
        let runtime = /* same builder */;
        let err = runtime
            .observe("does.not.exist", "post_call", "{}", "{}")
            .unwrap_err();
        assert!(matches!(err, RuntimeError::NotFound(id) if id == "does.not.exist"));
    }
```

Replace the `/* ... */` with the actual builder call from the neighbouring NotFound test (e.g. `test_runtime()` / `ExtensionRuntime::new(<cfg pointing at an empty tempdir>)` — copy verbatim from the existing test).

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p greentic-ext-runtime control_unknown_extension_is_not_found observe_unknown_extension_is_not_found 2>&1 | tail -20`
Expected: FAIL to compile — `control`/`observe` methods do not exist.

- [ ] **Step 3: Implement the methods**

Add a new `impl ExtensionRuntime { ... }` block near `list_tools`, mirroring it exactly:

```rust
impl ExtensionRuntime {
    /// Dispatch a SoRX control hook to a loaded `sorx-runtime-extension`.
    ///
    /// Returns the extension's ControlDecision JSON on Ok. Errors (missing
    /// extension, WIT trap, extension-returned error string) surface as
    /// `RuntimeError`; SoRX's caller applies the binding's fail-mode.
    pub fn control(
        &self,
        ext_id: &str,
        hook: &str,
        binding_json: &str,
        request_json: &str,
        response_json: Option<&str>,
    ) -> Result<String, RuntimeError> {
        let loaded = self
            .loaded
            .load()
            .get(&crate::loaded::ExtensionId(ext_id.to_string()))
            .cloned()
            .ok_or_else(|| RuntimeError::NotFound(ext_id.to_string()))?;

        let (mut store, instance) = loaded
            .build_store_and_instance(&self.engine, self.host_overrides.clone())
            .map_err(RuntimeError::Wasmtime)?;

        let iface_idx = instance
            .get_export_index(&mut store, None, "greentic:extension-sorx/control@0.1.0")
            .ok_or_else(|| {
                RuntimeError::Wasmtime(anyhow::anyhow!(
                    "extension '{ext_id}' does not export 'greentic:extension-sorx/control@0.1.0'"
                ))
            })?;
        let func_idx = instance
            .get_export_index(&mut store, Some(&iface_idx), "control")
            .ok_or_else(|| {
                RuntimeError::Wasmtime(anyhow::anyhow!(
                    "sorx control interface does not export 'control'"
                ))
            })?;

        let func = instance
            .get_typed_func::<(String, String, String, Option<String>), (Result<String, String>,)>(
                &mut store, &func_idx,
            )
            .map_err(|e| RuntimeError::Wasmtime(e.into()))?;

        let (result,) = func
            .call(
                &mut store,
                (
                    hook.to_string(),
                    binding_json.to_string(),
                    request_json.to_string(),
                    response_json.map(str::to_string),
                ),
            )
            .map_err(|e| RuntimeError::Wasmtime(e.into()))?;

        result.map_err(|msg| {
            RuntimeError::Wasmtime(anyhow::anyhow!("sorx extension '{ext_id}' control error: {msg}"))
        })
    }

    /// Dispatch a SoRX observer event to a loaded `sorx-runtime-extension`.
    pub fn observe(
        &self,
        ext_id: &str,
        subscription: &str,
        binding_json: &str,
        event_json: &str,
    ) -> Result<(), RuntimeError> {
        let loaded = self
            .loaded
            .load()
            .get(&crate::loaded::ExtensionId(ext_id.to_string()))
            .cloned()
            .ok_or_else(|| RuntimeError::NotFound(ext_id.to_string()))?;

        let (mut store, instance) = loaded
            .build_store_and_instance(&self.engine, self.host_overrides.clone())
            .map_err(RuntimeError::Wasmtime)?;

        let iface_idx = instance
            .get_export_index(&mut store, None, "greentic:extension-sorx/observe@0.1.0")
            .ok_or_else(|| {
                RuntimeError::Wasmtime(anyhow::anyhow!(
                    "extension '{ext_id}' does not export 'greentic:extension-sorx/observe@0.1.0'"
                ))
            })?;
        let func_idx = instance
            .get_export_index(&mut store, Some(&iface_idx), "observe")
            .ok_or_else(|| {
                RuntimeError::Wasmtime(anyhow::anyhow!(
                    "sorx observe interface does not export 'observe'"
                ))
            })?;

        let func = instance
            .get_typed_func::<(String, String, String), (Result<(), String>,)>(&mut store, &func_idx)
            .map_err(|e| RuntimeError::Wasmtime(e.into()))?;

        let (result,) = func
            .call(
                &mut store,
                (
                    subscription.to_string(),
                    binding_json.to_string(),
                    event_json.to_string(),
                ),
            )
            .map_err(|e| RuntimeError::Wasmtime(e.into()))?;

        result.map_err(|msg| {
            RuntimeError::Wasmtime(anyhow::anyhow!("sorx extension '{ext_id}' observe error: {msg}"))
        })
    }
}
```

> The interface export id passed to `get_export_index(store, None, ...)` must match how wasmtime names the exported interface. `list_tools` uses a `resolve_design_iface` helper (`runtime.rs:961`) that tries versioned names. If the plain `"greentic:extension-sorx/control@0.1.0"` lookup returns `None` at test time against a real component (Sub-C), fall back to mirroring `resolve_design_iface`'s newest-first `get_export_index(store, None, "iface@ver")` loop with a single-entry `SORX_VERSIONS = &["0.1.0"]`. For the NotFound smoke tests this path is never reached, so both forms pass Task 3.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p greentic-ext-runtime control_unknown_extension observe_unknown_extension 2>&1 | tail -20`
Expected: PASS (2 tests). Then `cargo test -p greentic-ext-runtime 2>&1 | tail -5` — no regressions.

- [ ] **Step 5: Commit**

```bash
git add crates/greentic-ext-runtime/src/runtime.rs
git commit -m "feat(ext): control/observe dispatch for sorx-runtime-extension"
```

---

### Task 4: Docs

**Files:**
- Modify: `CLAUDE.md` (root) — add `sorx-runtime-extension` to the extension-kinds/public-API notes
- Modify: `crates/greentic-ext-runtime/src/lib.rs` — only if a doc-comment listing dispatch methods exists; add `control`/`observe` there. No new type re-exports are needed (methods use `String`/`Option<String>`/`()`).

- [ ] **Step 1: Document**

In `CLAUDE.md` "Public API surface", add a bullet:
`- `control(ext_id, hook, binding_json, request_json, response_json)` / `observe(ext_id, subscription, binding_json, event_json)` — SoRX runtime-extension dispatch (world `greentic:extension-sorx`); JSON strings in/out; sync store, wrap in `spawn_blocking` from async.`

If `lib.rs` carries a module-level doc list of dispatch entry points, mirror the bullet there. Do not invent a doc structure that isn't there.

- [ ] **Step 2: Commit**

```bash
git add CLAUDE.md crates/greentic-ext-runtime/src/lib.rs
git commit -m "docs(ext): document sorx-runtime-extension dispatch surface"
```

---

### Task 5: Gate, PR, release tag

- [ ] **Step 1: Full local CI**

Run: `bash ci/local_check.sh 2>&1 | tail -30`
Expected: fmt clean, clippy `--all-features --locked -D warnings` clean, `cargo test --workspace --all-features --locked` green, release build ok. (Slow — run in background and WAIT.)

- [ ] **Step 2: Push + PR into develop**

```bash
git push -u origin feat/sorx-runtime-extension-world
gh pr create --base develop --head feat/sorx-runtime-extension-world \
  --title "feat(ext): sorx-runtime-extension world + control/observe dispatch" \
  --body "..."   # summarize: new greentic:extension-sorx world, control/observe methods mirroring list_tools/render_bundle, NotFound smoke tests, minimal (logging-only) host imports; consumed by greentic-sorx phase-2 Sub-B. No Claude co-author trailer.
```

- [ ] **Step 3: Release hand-off (after merge — note for the human)**

After the PR merges to `develop`, cut the workspace + crate tags per repo convention (`v1.2.X`, `extrt-v1.2.X`) so `greentic-sorx` (Sub-B) can git-pin the exact commit/tag that carries `control`/`observe`. Record the tag + the two method signatures in the SoRX phase-2 tracking. (Tagging is a release action — leave it to the human unless explicitly asked.)
