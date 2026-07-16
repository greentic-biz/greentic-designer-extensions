//! Tests for the extension loader path covering the dual-component layout.
//!
//! The dual-component layout is used by multiple extension kinds:
//!   - `extension.wasm` — design-side WASM (metadata, icons, i18n)
//!   - runtime gtpack / gtxpack — placeholder or real .gtpack ZIP; real
//!     runtime lives in runner-host and is fetched lazily there.
//!
//! `describe.json` points `runtime.components[stub].gtpack.file` at the
//! runtime gtpack path, which may not be valid WASM. The loader must prefer
//! the design-side `extension.wasm` at root when it is present, regardless of
//! extension kind.
//!
//! Affected kinds in production:
//!   - `ProviderExtension` (e.g. `greentic.provider.telegram-1.3.1-research`)
//!   - `DesignExtension` (e.g. `greentic.llm-openai-1.3.1-research`)
//!   - `BundleExtension` (e.g. `greentic.bundle-standard-1.3.0-research`)
//!
//! See `crates/greentic-ext-runtime/src/loaded.rs::wasm_component_path`.

#[path = "support/mod.rs"]
mod support;

use std::path::PathBuf;

use greentic_ext_runtime::{DiscoveryPaths, ExtensionRuntime, RuntimeConfig};
use greentic_extension_sdk_contract::ExtensionKind;

use support::{
    signed_fixture_with_placeholder_gtpack, signed_fixture_without_root_wasm,
    signed_provider_fixture_with_placeholder_gtpack,
};

/// The returned `TempDir` is the trust root and must be held for the test's
/// lifetime — see the note in `signature_gate.rs`. Without it these tests pin
/// fixture keys into the developer's real `~/.greentic`.
fn build_runtime() -> (ExtensionRuntime, tempfile::TempDir) {
    let trust = tempfile::TempDir::new().expect("temp trust root");
    let config = RuntimeConfig::from_paths(DiscoveryPaths::new(PathBuf::from("/dev/null")))
        .with_trust_root(trust.path().to_path_buf());
    (ExtensionRuntime::new(config).unwrap(), trust)
}

/// Provider extensions must load the design-side `extension.wasm` at root,
/// even though `describe.json` points `gtpack.file` at
/// `"runtime/provider.gtpack"` (a placeholder that is intentionally NOT a
/// valid WASM component).
#[test]
fn provider_extension_prefers_extension_wasm_at_root() {
    let (fixture, _sk) =
        signed_provider_fixture_with_placeholder_gtpack("greentic.provider.telegram", "1.3.1");

    // Sanity-check: confirm the placeholder text file exists and that
    // `extension.wasm` is present at root.
    let placeholder_path = fixture.root().join("runtime").join("provider.gtpack");
    assert!(
        placeholder_path.exists(),
        "test precondition: runtime/provider.gtpack must exist"
    );
    let wasm_path = fixture.root().join("extension.wasm");
    assert!(
        wasm_path.exists(),
        "test precondition: extension.wasm must exist at root"
    );

    let (mut runtime, _trust) = build_runtime();
    // The load must succeed — it picks up `extension.wasm`, not the
    // placeholder gtpack that would fail wasmtime parsing.
    runtime
        .register_loaded_from_dir(fixture.root())
        .expect("provider extension with extension.wasm at root must load successfully");

    // Confirm the extension is registered under its declared id.
    let loaded_map = runtime.loaded();
    assert!(
        loaded_map
            .keys()
            .any(|id| id.as_str() == "greentic.provider.telegram"),
        "loaded extension map must contain 'greentic.provider.telegram'"
    );
}

/// Design extensions with `extension.wasm` at root must prefer it over the
/// runtime gtpack declared in `describe.runtime.components`. This mirrors the
/// behaviour of `greentic.llm-openai-1.3.1-research`, whose `describe.json`
/// points at `runtime/component-llm-openai.gtpack` (a 929 KB .gtpack ZIP that
/// wasmtime cannot parse as a raw component).
#[test]
fn design_extension_prefers_extension_wasm_at_root() {
    let (fixture, _sk) = signed_fixture_with_placeholder_gtpack(
        ExtensionKind::Design,
        "greentic.test-design-ext-dual",
        "1.3.1",
    );

    // Preconditions: extension.wasm at root, placeholder at runtime path.
    let wasm_path = fixture.root().join("extension.wasm");
    assert!(
        wasm_path.exists(),
        "test precondition: extension.wasm must exist at root"
    );
    let placeholder_path = fixture.root().join("runtime").join("component.gtpack");
    assert!(
        placeholder_path.exists(),
        "test precondition: runtime/component.gtpack placeholder must exist"
    );

    let (mut runtime, _trust) = build_runtime();
    // Load must succeed: loader picks extension.wasm, not the placeholder gtpack.
    runtime
        .register_loaded_from_dir(fixture.root())
        .expect("design extension with extension.wasm at root must load successfully");

    let loaded_map = runtime.loaded();
    assert!(
        loaded_map
            .keys()
            .any(|id| id.as_str() == "greentic.test-design-ext-dual"),
        "loaded extension map must contain 'greentic.test-design-ext-dual'"
    );
}

/// Fallback path: when no `extension.wasm` exists at root, the loader must
/// resolve the component via `describe.runtime.components[X].gtpack.file`.
///
/// This covers older single-component extensions and any future kind that ships
/// a single component pointed to directly in describe.json.
#[test]
fn extension_without_root_wasm_loads_from_describe_gtpack_file() {
    // `signed_fixture_without_root_wasm` renames extension.wasm → inner.wasm
    // and patches gtpack.file to point at "inner.wasm". The root WASM check
    // will find nothing at root, so the fallback branch resolves "inner.wasm".
    let (fixture, _sk) = signed_fixture_without_root_wasm(
        ExtensionKind::Design,
        "greentic.test-fallback-ext",
        "0.2.0",
    );

    // Precondition: no extension.wasm at root.
    let root_wasm = fixture.root().join("extension.wasm");
    assert!(
        !root_wasm.exists(),
        "test precondition: extension.wasm must NOT exist at root"
    );
    // The actual component is at inner.wasm.
    let inner_wasm = fixture.root().join("inner.wasm");
    assert!(
        inner_wasm.exists(),
        "test precondition: inner.wasm (fallback target) must exist"
    );

    let (mut runtime, _trust) = build_runtime();
    runtime
        .register_loaded_from_dir(fixture.root())
        .expect("extension without root extension.wasm must load via describe.runtime.components");

    let loaded_map = runtime.loaded();
    assert!(
        loaded_map
            .keys()
            .any(|id| id.as_str() == "greentic.test-fallback-ext"),
        "loaded extension map must contain 'greentic.test-fallback-ext'"
    );
}
