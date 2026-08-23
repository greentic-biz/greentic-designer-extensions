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

/// The returned `TempDir` is the trust root and must be held for the test's
/// lifetime. It is not optional: the default root is the developer's real
/// `~/.greentic`, and `signed_fixture` mints a fresh key per call, so a
/// default-rooted test pins junk on its first run and then fails every later
/// run with `PublisherKeyChanged`.
fn new_runtime() -> (ExtensionRuntime, tempfile::TempDir) {
    let trust = tempfile::TempDir::new().expect("temp trust root");
    let config = RuntimeConfig::from_paths(DiscoveryPaths::new(PathBuf::from("/dev/null")))
        .with_trust_root(trust.path().to_path_buf());
    (ExtensionRuntime::new(config).unwrap(), trust)
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

    let (mut rt, _trust) = new_runtime();
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
    let (mut rt, _trust) = new_runtime();
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

    let (mut rt, _trust) = new_runtime();
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

    let (mut rt, _trust) = new_runtime();
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

    let (mut rt, _trust) = new_runtime();
    let err = rt.register_loaded_from_dir(fx.root()).unwrap_err();
    match err {
        RuntimeError::SignatureInvalid { reason, .. } => assert!(
            reason.contains("manifest binding"),
            "unexpected reason: {reason}",
        ),
        other => panic!("expected SignatureInvalid, got {other:?}"),
    }
}

#[test]
fn valid_signature_with_broken_manifest_pins_nothing() {
    // Ordering gate: the anchor must not be written until the artifact's
    // integrity ledger has passed. The signature here is genuinely valid — only
    // the ledger is broken — so steps 1 and 2 accept and the pin is the next
    // thing that would run.
    //
    // The trust store is deliberately the same one gtdx writes. A pin from a
    // *rejected* load is therefore not a local mistake: it permanently blocks
    // the real publisher for this id, in both tools, until someone hand-edits
    // publishers.json. An attacker who cannot complete a load must not be able
    // to squat an id this way.
    let _guard = EnvGuard::remove("GREENTIC_EXT_ALLOW_UNSIGNED");
    let (fx, _sk) = signed_fixture(ExtensionKind::Design, "greentic.guardrail-pii", "0.1.0");
    std::fs::remove_file(manifest_path(fx.root())).unwrap();

    let (mut rt, trust) = new_runtime();
    rt.register_loaded_from_dir(fx.root())
        .expect_err("a pack with no manifest must be rejected");

    // Asserted one level above publishers.json: `pin_or_verify` creates the
    // trust dir and its lockfile before inserting, so the directory existing at
    // all proves the store was reached.
    assert!(
        !trust.path().join("trust").exists(),
        "a rejected load touched the shared trust store"
    );
}
