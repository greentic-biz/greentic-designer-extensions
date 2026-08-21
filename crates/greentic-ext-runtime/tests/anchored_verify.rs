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

/// The key `<root>/trust/publishers.json` pins for `id` — `None` when the store
/// does not exist or holds no entry for it. Reading the store, rather than
/// inferring trust from a bare `Ok`/`Err`, is what makes the anchor's side
/// effect observable: a runtime with no anchor at all returns the same `Ok`.
fn pinned_key(root: &Path, id: &str) -> Option<String> {
    let raw = std::fs::read_to_string(root.join("trust").join("publishers.json")).ok()?;
    let store: serde_json::Value = serde_json::from_str(&raw).ok()?;
    store
        .get("publishers")?
        .get(id)?
        .as_str()
        .map(str::to_owned)
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
    let key = describe_public_key(fx.root());
    let mut rt = runtime_with_trust_root(trust.path());

    rt.register_loaded_from_dir(fx.root()).expect("first load");
    rt.register_loaded_from_dir(fx.root())
        .expect("re-registering the same fixture (same key) must be accepted");

    // Asserting only "the reload returned Ok" would hold just as well with no
    // anchor at all: deleting a check can never turn an accept into a reject,
    // so a pure accept-test is blind to the thing it exists to cover. The
    // observable proof that the accept came from `pin_or_verify`'s verify
    // branch — rather than from nothing running — is the store itself.
    assert_eq!(
        pinned_key(trust.path(), "greentic.tofu-same").as_deref(),
        Some(key.as_str()),
        "the reload must leave the id pinned to the key from the first load"
    );
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

/// A describe that fails step 1 (self-consistency) must never reach the pin —
/// otherwise an attacker could squat an id this runtime has not seen yet by
/// presenting a describe that does not even verify.
///
/// The ordering case that matters more — a VALID signature over a broken
/// manifest, which reaches step 2 and must still not pin — is covered by
/// `manifest_gate.rs::valid_signature_with_broken_manifest_pins_nothing`.
#[test]
fn bad_signature_pins_nothing() {
    let _guard = EnvGuard::remove("GREENTIC_EXT_ALLOW_UNSIGNED");
    let trust = tempfile::TempDir::new().unwrap();
    let mut rt = runtime_with_trust_root(trust.path());

    // Positive control FIRST. Asserting only "the tampered id did not get
    // pinned" is satisfied just as well by a runtime that pins nothing at all
    // — "rejected, so no pin" and "no pinning code exists" are indistinguishable
    // from the outside. Pinning an unrelated good fixture proves the machinery
    // is live in this very runtime, so the negative assert below means something.
    let (control, _ck) = signed_fixture(ExtensionKind::Design, "greentic.tofu-control", "0.1.0");
    let control_key = describe_public_key(control.root());
    rt.register_loaded_from_dir(control.root())
        .expect("a well-formed signed fixture must load and pin");
    assert_eq!(
        pinned_key(trust.path(), "greentic.tofu-control").as_deref(),
        Some(control_key.as_str()),
        "control load must pin — without this the assert below proves nothing"
    );

    let (fx, _sk) = signed_fixture(ExtensionKind::Design, "greentic.tofu-tampered", "0.1.0");
    support::tamper_fixture(&fx);

    rt.register_loaded_from_dir(fx.root())
        .expect_err("a tampered describe must be rejected");

    assert_eq!(
        pinned_key(trust.path(), "greentic.tofu-tampered"),
        None,
        "a rejected describe must not have pinned its id"
    );
}
