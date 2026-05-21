//! Tests for the provider-extension loader path.
//!
//! Provider extensions have a dual-component layout:
//!   - `extension.wasm`          — design-side WASM (metadata, icons, i18n)
//!   - `runtime/provider.gtpack` — placeholder text; real runtime lives in
//!                                  runner-host and is fetched lazily there.
//!
//! `describe.json` points `runtime.components[stub].gtpack.file` at
//! `"runtime/provider.gtpack"`, which is NOT valid WASM. The loader must
//! detect `ExtensionKind::Provider`, prefer the design-side `extension.wasm`
//! at root, and ignore the placeholder gtpack entirely.
//!
//! See `crates/greentic-ext-runtime/src/loaded.rs::wasm_component_path`.

#[path = "support/mod.rs"]
mod support;

use std::path::PathBuf;

use greentic_ext_runtime::{DiscoveryPaths, ExtensionRuntime, RuntimeConfig};
use greentic_extension_sdk_contract::ExtensionKind;

use support::{signed_fixture, signed_provider_fixture_with_placeholder_gtpack};

fn build_runtime() -> ExtensionRuntime {
    let config = RuntimeConfig::from_paths(DiscoveryPaths::new(PathBuf::from("/dev/null")));
    ExtensionRuntime::new(config).unwrap()
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

    let mut runtime = build_runtime();
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

/// Regression guard: `DesignExtension` kind must still load from the path
/// declared in `describe.runtime.components[X].gtpack.file`, not from a
/// hard-coded `extension.wasm`. This guards against accidentally switching
/// the design extension path to the provider-specific fast-path.
#[test]
fn design_extension_loads_from_describe_gtpack_file() {
    // `signed_fixture` patches gtpack.file → "extension.wasm" and signs.
    // That file is what the builder writes. Both paths coincide here, but
    // the important thing is that the loader goes through the
    // `describe.runtime.components` branch, not the Provider shortcut.
    let (fixture, _sk) = signed_fixture(ExtensionKind::Design, "greentic.test-design-ext", "0.2.0");

    let mut runtime = build_runtime();
    runtime
        .register_loaded_from_dir(fixture.root())
        .expect("design extension must load via describe.runtime.components gtpack.file");

    let loaded_map = runtime.loaded();
    assert!(
        loaded_map
            .keys()
            .any(|id| id.as_str() == "greentic.test-design-ext"),
        "loaded extension map must contain 'greentic.test-design-ext'"
    );
}
