//! Anchored verification (TOFU): the describe signature must not merely be
//! self-consistent, it must be made by the key this runtime pinned for that
//! extension id on first load.
//!
//! Self-consistency proves only that the describe is unmodified *since
//! signing* — an attacker who swaps a guardrail extension and re-signs with
//! their own key passes it trivially. Pinning the publisher key per extension
//! id is what turns "signed" into "signed by the same publisher as last time".

#[path = "support/mod.rs"]
mod support;

use std::path::{Path, PathBuf};

use greentic_ext_runtime::{DiscoveryPaths, ExtensionRuntime, RuntimeConfig, RuntimeError};
use greentic_extension_sdk_contract::ExtensionKind;

use support::{EnvGuard, signed_fixture};

/// A runtime whose trust store is a throwaway temp dir. Every test MUST use
/// this rather than the default root — the default resolves to the developer's
/// real `~/.greentic/trust/publishers.json`, and `signed_fixture` mints a fresh
/// key per call, so a default-rooted test would pin a junk key on its first run
/// and then fail `PublisherKeyChanged` on every run after.
fn runtime_with_trust_root(root: &Path) -> ExtensionRuntime {
    let config = RuntimeConfig::from_paths(DiscoveryPaths::new(PathBuf::from("/dev/null")))
        .with_trust_root(root.to_path_buf());
    ExtensionRuntime::new(config).expect("runtime construction")
}

/// The base64 public key the fixture's signed describe.json advertises.
fn describe_public_key(dir: &Path) -> String {
    let raw = std::fs::read_to_string(dir.join("describe.json")).expect("read describe.json");
    let describe: greentic_extension_sdk_contract::DescribeJson =
        serde_json::from_str(&raw).expect("parse describe.json");
    describe
        .signature
        .expect("fixture describe must be signed")
        .public_key
}

#[test]
fn tofu_pins_on_first_load() {
    let _guard = EnvGuard::remove("GREENTIC_EXT_ALLOW_UNSIGNED");
    let trust = tempfile::TempDir::new().unwrap();

    let (fx, _sk) = signed_fixture(ExtensionKind::Design, "greentic.tofu-pin", "0.1.0");
    let key = describe_public_key(fx.root());

    let mut rt = runtime_with_trust_root(trust.path());
    rt.register_loaded_from_dir(fx.root())
        .expect("first load of a signed fixture must succeed");

    // The pin must land in the store gtdx reads: <root>/trust/publishers.json.
    let store = trust.path().join("trust").join("publishers.json");
    assert!(
        store.exists(),
        "first load must create the TOFU store at {}",
        store.display()
    );
    let body = std::fs::read_to_string(&store).unwrap();
    assert!(
        body.contains("greentic.tofu-pin"),
        "store must name the extension id; got: {body}"
    );
    assert!(
        body.contains(&key),
        "store must pin the signing key {key}; got: {body}"
    );
}

#[test]
fn tofu_accepts_same_key_on_reload() {
    let _guard = EnvGuard::remove("GREENTIC_EXT_ALLOW_UNSIGNED");
    let trust = tempfile::TempDir::new().unwrap();

    let (fx, _sk) = signed_fixture(ExtensionKind::Design, "greentic.tofu-same", "0.1.0");
    let mut rt = runtime_with_trust_root(trust.path());

    rt.register_loaded_from_dir(fx.root()).expect("first load");
    rt.register_loaded_from_dir(fx.root())
        .expect("re-registering the same fixture (same key) must be accepted");
}

#[test]
fn tofu_rejects_different_key_same_id() {
    let _guard = EnvGuard::remove("GREENTIC_EXT_ALLOW_UNSIGNED");
    let trust = tempfile::TempDir::new().unwrap();

    // Two fixtures, SAME extension id, different fresh random keys — exactly
    // the "attacker re-signs a known id with their own key" case. Both are
    // perfectly self-consistent, so only the anchor can tell them apart.
    let (first, _sk1) = signed_fixture(ExtensionKind::Design, "greentic.tofu-swap", "0.1.0");
    let (second, _sk2) = signed_fixture(ExtensionKind::Design, "greentic.tofu-swap", "0.1.0");
    let pinned = describe_public_key(first.root());
    let presented = describe_public_key(second.root());
    assert_ne!(
        pinned, presented,
        "signed_fixture must mint a fresh key per call, or this test proves nothing"
    );

    let mut rt = runtime_with_trust_root(trust.path());
    rt.register_loaded_from_dir(first.root())
        .expect("first load pins");

    let err = rt
        .register_loaded_from_dir(second.root())
        .expect_err("a different key for a pinned id must be rejected");
    match err {
        RuntimeError::SignatureInvalid {
            extension_id,
            reason,
        } => {
            assert_eq!(extension_id, "greentic.tofu-swap");
            // An operator must be able to tell a key rotation from an attack,
            // which needs BOTH keys named in the message.
            assert!(
                reason.contains(&pinned),
                "reason must name the pinned key {pinned}; got: {reason}"
            );
            assert!(
                reason.contains(&presented),
                "reason must name the presented key {presented}; got: {reason}"
            );
        }
        other => panic!("expected SignatureInvalid, got {other:?}"),
    }
}

/// A bad signature must never poison the pin. If step 3 (pin) ran before step 2
/// (verify), an attacker could pre-pin their own key for an id this runtime has
/// not seen yet by presenting a describe that fails verification.
#[test]
fn bad_signature_pins_nothing() {
    let _guard = EnvGuard::remove("GREENTIC_EXT_ALLOW_UNSIGNED");
    let trust = tempfile::TempDir::new().unwrap();

    let (fx, _sk) = signed_fixture(ExtensionKind::Design, "greentic.tofu-tampered", "0.1.0");
    support::tamper_fixture(&fx);

    let mut rt = runtime_with_trust_root(trust.path());
    rt.register_loaded_from_dir(fx.root())
        .expect_err("a tampered describe must be rejected");

    let store = trust.path().join("trust").join("publishers.json");
    assert!(
        !store.exists(),
        "a rejected describe must not have pinned anything, but {} exists",
        store.display()
    );
}
