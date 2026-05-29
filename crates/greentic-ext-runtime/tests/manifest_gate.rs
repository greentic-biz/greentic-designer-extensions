//! Audit P5 (consumer hardening): ext-runtime now **fails closed** on the
//! whole-archive integrity ledger. A pack with no `manifest.json` is rejected
//! (only the `dev-allow-unsigned` escape loads it); a present manifest must be
//! bound into the signed describe (`manifestSha256`) AND every listed file must
//! hash-match. The fixtures here are produced bind→sign→manifest, exactly like
//! the SDK producer.

#[path = "support/mod.rs"]
mod support;

use std::path::PathBuf;

use greentic_ext_runtime::{DiscoveryPaths, ExtensionRuntime, RuntimeConfig, RuntimeError};
use greentic_extension_sdk_contract::ExtensionKind;

use support::{EnvGuard, signed_fixture};

fn new_runtime() -> ExtensionRuntime {
    let config = RuntimeConfig::from_paths(DiscoveryPaths::new(PathBuf::from("/dev/null")));
    ExtensionRuntime::new(config).unwrap()
}

fn manifest_path(dir: &std::path::Path) -> PathBuf {
    dir.join(greentic_extension_sdk_contract::MANIFEST_ENTRY_NAME)
}

#[test]
fn pack_without_manifest_is_rejected() {
    // Fail-closed: removing the ledger from an otherwise-valid signed pack must
    // refuse to load (audit P5 — was previously fail-open).
    let _guard = EnvGuard::remove("GREENTIC_EXT_ALLOW_UNSIGNED");
    let (fx, _sk) = signed_fixture(ExtensionKind::Design, "greentic.no-manifest", "0.1.0");
    std::fs::remove_file(manifest_path(fx.root())).unwrap();

    let mut rt = new_runtime();
    let err = rt.register_loaded_from_dir(fx.root()).unwrap_err();
    match err {
        RuntimeError::SignatureInvalid { reason, .. } => assert!(
            reason.contains("manifest.json absent"),
            "unexpected reason: {reason}",
        ),
        other => panic!("expected SignatureInvalid, got {other:?}"),
    }
}

#[test]
fn pack_with_intact_manifest_loads() {
    let _guard = EnvGuard::remove("GREENTIC_EXT_ALLOW_UNSIGNED");
    let (fx, _sk) = signed_fixture(ExtensionKind::Design, "greentic.with-manifest", "0.1.0");
    let mut rt = new_runtime();
    rt.register_loaded_from_dir(fx.root())
        .expect("a bound, intact manifest must verify");
}

#[test]
fn pack_with_tampered_wasm_after_manifest_is_rejected() {
    // Manifest stays intact (so the binding still matches), but the wasm it
    // lists is mutated — caught by the per-entry hash check.
    let _guard = EnvGuard::remove("GREENTIC_EXT_ALLOW_UNSIGNED");
    let (fx, _sk) = signed_fixture(ExtensionKind::Design, "greentic.tampered-wasm", "0.1.0");
    let wasm_path = fx.root().join("extension.wasm");
    let mut bytes = std::fs::read(&wasm_path).unwrap();
    bytes.push(0xff);
    std::fs::write(&wasm_path, bytes).unwrap();

    let mut rt = new_runtime();
    let err = rt.register_loaded_from_dir(fx.root()).unwrap_err();
    match err {
        RuntimeError::SignatureInvalid { reason, .. } => assert!(
            reason.contains("manifest sha256 mismatch"),
            "unexpected reason: {reason}",
        ),
        other => panic!("expected SignatureInvalid, got {other:?}"),
    }
}

#[test]
fn pack_with_manifest_listing_missing_file_is_rejected() {
    // Manifest (bound) lists extension.wasm, but the file is removed from disk —
    // caught by the per-entry existence check, binding still intact.
    let _guard = EnvGuard::remove("GREENTIC_EXT_ALLOW_UNSIGNED");
    let (fx, _sk) = signed_fixture(ExtensionKind::Design, "greentic.missing-file", "0.1.0");
    std::fs::remove_file(fx.root().join("extension.wasm")).unwrap();

    let mut rt = new_runtime();
    let err = rt.register_loaded_from_dir(fx.root()).unwrap_err();
    match err {
        RuntimeError::SignatureInvalid { reason, .. } => assert!(
            reason.contains("manifest lists missing file"),
            "unexpected reason: {reason}",
        ),
        other => panic!("expected SignatureInvalid, got {other:?}"),
    }
}

#[test]
fn tampered_manifest_breaks_binding() {
    // Any post-sign mutation of manifest.json breaks the describe's manifestSha256
    // binding — the signature transitively covers the ledger (audit C2). This is
    // the dominant protection: it fires before the per-entry / schema checks,
    // regardless of what the tampered manifest claims (bad schema, phantom
    // entries, swapped hashes).
    let _guard = EnvGuard::remove("GREENTIC_EXT_ALLOW_UNSIGNED");
    let (fx, _sk) = signed_fixture(ExtensionKind::Design, "greentic.tampered-manifest", "0.1.0");
    let bogus = serde_json::json!({
        "schema": "greentic.gtxpack.manifest/v999",
        "entries": [],
    });
    std::fs::write(
        manifest_path(fx.root()),
        serde_json::to_vec(&bogus).unwrap(),
    )
    .unwrap();

    let mut rt = new_runtime();
    let err = rt.register_loaded_from_dir(fx.root()).unwrap_err();
    match err {
        RuntimeError::SignatureInvalid { reason, .. } => assert!(
            reason.contains("manifest binding"),
            "unexpected reason: {reason}",
        ),
        other => panic!("expected SignatureInvalid, got {other:?}"),
    }
}
