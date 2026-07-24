# `sorx-runtime-extension` WIT world + control/observe dispatch — design (Sub-A)

_Date: 2026-07-24 · Repo: `greentic-designer-extensions` · Crate: `greentic-ext-runtime` · Branch: `feat/sorx-runtime-extension-world` (off `develop`)_

## Context

Part of the SoRLa/SoRX productionization epic, item **#6-B phase 2** (WASM extension
execution for SoRX). Phase 1 (merged to `greentic-sorx` research) made the extension seam
live and shipped a native (in-process) audit observer. Phase 2 lets SoRX run **third-party
WASM extension packs** through the existing `RuntimeExtensionAdapter` trait
(`control`/`observe`, JSON-in/JSON-out, keyed by `pack_ref`, sync).

The architecture decision (made with the user): **reuse `greentic-ext-runtime`** — the
proven wasmtime component host — by adding a new world + dispatch methods here, and have
`greentic-sorx` depend on this crate (git-pinned). This is **Sub-A**: the host-side
dependency root. **Sub-B** (the sorx-side `WasmExtensionRuntime` adapter + wiring) and
**Sub-C** (a guest sample component + end-to-end test) follow, in `greentic-sorx` /
consumer repos, after Sub-A is released and tagged.

## Goal (Sub-A)

Add a `greentic:sorx-runtime-extension@0.1.0` world and two sync dispatch methods on
`ExtensionRuntime` — `control(...)` and `observe(...)` — so a loaded, verified WASM
extension can be invoked with SoRX's control/observe contract. Follow this crate's
established "Adding a new world" recipe (CLAUDE.md), mirroring the most recent example
(`render_bundle`).

Success criteria:

1. `ExtensionRuntime::control(ext_id, hook, binding_json, request_json, response_json)`
   returns the extension's control-decision JSON string (`Ok(String)`), or a
   `RuntimeError` on a missing extension / WIT-level error.
2. `ExtensionRuntime::observe(ext_id, subscription, binding_json, event_json)` returns
   `Ok(())` or a `RuntimeError`.
3. The world imports only the host functions SoRX-side extensions actually need (minimal),
   while remaining compatible with the existing linker wiring.
4. Existing behavior (design/deploy/bundle dispatch) is unchanged.

## Non-goals (Sub-A)

- No sorx-side adapter, registry wiring, or discovery config (that is Sub-B, in
  `greentic-sorx`).
- No new signing/verification logic — reuse the existing `verify_dir_signature` chain
  (runs in `register_loaded_from_dir`).
- No guest WASM component built here — reference extensions are built in consumer repos
  (per this repo's convention); Sub-A's host tests exercise the error path only (see
  Testing).
- No new discovery **kind** — sorx registers its extension dirs explicitly via
  `register_loaded_from_dir(path)` (Sub-B), the same way the designer registers design/
  deploy/bundle dirs. `discovery.rs` is untouched.

## Architecture

Follows CLAUDE.md "Adding a new world / interface" (5 steps), mirroring `render_bundle`.

### 1. WIT world (`crates/greentic-ext-runtime/wit/deps/sorx-runtime-extension/`)

```wit
package greentic:sorx-runtime-extension@0.1.0;

interface control {
  // `hook` ∈ pre_call | post_call | pre_admin | post_admin.
  // binding-json / request-json / response-json are the serde-JSON of SoRX's
  // RuntimeExtensionBinding / request / optional response. Returns the JSON of a
  // SoRX ControlDecision, or an error message string.
  control: func(
    hook: string,
    binding-json: string,
    request-json: string,
    response-json: option<string>,
  ) -> result<string, string>;
}

interface observe {
  // `subscription` ∈ pre_call | post_call | call_failed | control_denied.
  observe: func(
    subscription: string,
    binding-json: string,
    event-json: string,
  ) -> result<_, string>;
}

world sorx-runtime-extension {
  // Minimal host imports: SoRX control/observe hooks need logging; anything more
  // (secrets/broker/http/llm) is intentionally omitted to keep the operator-runtime
  // extension surface small. The host linker provides a superset, so importing a
  // subset is compatible.
  import greentic:extension-host/logging@0.1.0;
  export control;
  export observe;
}
```

Rationale: `result<string, string>` (plain-string error) is used instead of the
6-variant `extension-error` so this world stays off the per-kind error-ABI version fork —
simpler than the design path, and the caller (SoRX) only needs the message.

### 2. `bindgen!` module (`host_bindings.rs`)

Add a sibling `mod sorx` with
`wasmtime::component::bindgen!({ path: "wit", world: "greentic:sorx-runtime-extension@0.1.0" })`,
isolated in its own module to avoid `extension-base`/`extension-host` shared-type
collisions (the documented reason each world gets its own `mod`). Add a `SORX_VERSIONS`
table (single entry `"0.1.0"` for now) mirroring `DESIGN_VERSIONS`/`BUNDLE_VERSIONS`, used
by `resolve_iface_versions`.

### 3. Types (`types.rs` + `lib.rs`)

The dispatch surface is plain JSON strings, so minimal new Rust types are needed. Do NOT
model `ControlDecision`/`RuntimeExtensionBinding` here — they belong to SoRX; this crate
passes their JSON through opaquely. If the two methods' signatures read cleanly with raw
`String`/`Option<String>` params and `String`/`()` returns, add no new struct. Re-export
the two new methods' error variant if a new `RuntimeError` case is introduced (see §4).

### 4. Dispatch methods (`runtime.rs`, own `impl ExtensionRuntime` block)

Clone the `invoke_tool_ctx` template:

- `pub fn control(&self, ext_id: &ExtensionId, hook: &str, binding_json: &str, request_json: &str, response_json: Option<&str>) -> Result<String, RuntimeError>`
- `pub fn observe(&self, ext_id: &ExtensionId, subscription: &str, binding_json: &str, event_json: &str) -> Result<(), RuntimeError>`

Each:
1. Look up the `LoadedExtension` in the `ArcSwap` map by `ext_id`; `RuntimeError::NotFound`
   if absent.
2. `build_store_and_instance(...)` (sync).
3. `resolve_iface_versions(store, instance, "greentic:sorx-runtime-extension/control"
   (resp. `/observe`), SORX_VERSIONS)` → export index.
4. `get_export_index(store, Some(&iface), "control"|"observe")`.
5. `get_typed_func::<(String, String, String, Option<String>), (Result<String, String>,)>`
   for control (resp. `<(String, String, String), (Result<(), String>,)>` for observe);
   sync `call`; map the inner `Err(String)` to `RuntimeError::Wasmtime` (or a new
   `RuntimeError::Extension` message case) and the outer wasmtime error likewise.

Keep the file section under the 500-line/file limit by putting this in its own `impl`
block, as the recipe instructs.

### 5. Error handling & fail-mode

The method returns `Err` on any failure; SoRX's caller already honors per-binding
`fail_mode` (Open = swallow+continue, Closed = propagate). This crate does not decide
fail-open/closed — it just surfaces the error, consistent with `invoke_tool`.

## Verification (reused, unchanged)

A sorx extension dir is registered via the existing `register_loaded_from_dir(path)`, which
runs `verify_dir_signature` (describe self-consistency + manifest binding, fail-closed on a
missing manifest) before load. The `dev-allow-unsigned` feature + `GREENTIC_EXT_ALLOW_UNSIGNED`
env escape already covers local unsigned dev. No change here.

## Testing (Sub-A)

Per this crate's convention (reference guest components are built in consumer repos, so the
host repo has no `.wasm` fixtures), add a `#[cfg(test)]` smoke test in `runtime.rs`'s test
module mirroring the existing recipe step 5:

- `control` / `observe` against an unknown `ext_id` (empty tempdir runtime, e.g.
  `ExtensionRuntime::for_test()`) return `RuntimeError::NotFound` — proving the lookup +
  error mapping is wired, without needing a built component.
- A `wit-lint` / build check that the new WIT world parses and `bindgen!` compiles (the
  crate builds = the macro expanded cleanly).

Actual control/observe **execution** against a real component is proven in Sub-C
(a guest fixture built for `wasm32-wasip2`) and Sub-B's end-to-end test — out of scope here.

## Release / hand-off to Sub-B

After merge to `develop`, cut the workspace release tag (`v0.X.Y`) per this repo's tagging
convention so `greentic-sorx` (Sub-B) can git-pin it. Record the tag + the two method
signatures in the SoRX phase-2 tracking so Sub-B consumes them exactly.

## Global constraints (from this repo)

- Rust 1.95.0, edition 2024 (`rust-toolchain.toml` canonical).
- **Max 500 lines per source file** — split before exceeding.
- **English only**; Conventional Commits; **no Claude co-authorship trailer on commits**.
- Husky hooks (pre-commit fmt+clippy, pre-push `ci/local_check.sh`) — do not bypass.
- Feature branch + PR into `develop`; never push to `main`.
- wasmtime / wasmtime-wasi version = whatever `greentic-ext-runtime` pins at implementation
  time (currently `43` on `develop`; dependabot PRs move toward `46` — target the live
  pin, do not introduce a second wasmtime version).
- Use `wasmtime::component::bindgen!` (host macro shipped in the `wasmtime` crate) — the
  host takes no `wit-bindgen` dependency; `wit-bindgen` is a guest-only concern.
