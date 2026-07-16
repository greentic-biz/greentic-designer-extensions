//! The watcher hot-reload path must enforce the same signature gate as
//! explicit registration, and the capability registry must stay a faithful
//! reflection of what is actually loaded.
//!
//! These two live together because they are the same class of bug: the
//! watcher path (`handle_added_or_modified` / `handle_removal`) silently did
//! less than the registration path it shadows. It is not dead code —
//! `greentic-designer/src/ui/mod.rs:983` calls `start_watcher()` and holds the
//! guard for the whole server lifetime — so anything it skips is skipped in
//! production.

#[path = "support/mod.rs"]
mod support;

use std::path::{Path, PathBuf};

use greentic_ext_runtime::{DiscoveryPaths, ExtensionRuntime, RuntimeConfig, RuntimeError};
use greentic_extension_sdk_contract::ExtensionKind;

use support::{EnvGuard, signed_fixture, tamper_fixture, unsigned_fixture};

/// See the note in `anchored_verify.rs`: the trust root must be a temp dir or
/// the test pins into the developer's real `~/.greentic`.
fn runtime_with_trust_root(root: &Path) -> ExtensionRuntime {
    let config = RuntimeConfig::from_paths(DiscoveryPaths::new(PathBuf::from("/dev/null")))
        .with_trust_root(root.to_path_buf());
    ExtensionRuntime::new(config).expect("runtime construction")
}

/// Every capability id currently advertised for `extension_id`.
fn offered_caps(rt: &ExtensionRuntime, extension_id: &str) -> Vec<String> {
    let mut caps: Vec<String> = rt
        .capability_registry()
        .offerings()
        .filter(|o| o.extension_id == extension_id)
        .map(|o| o.cap_id.to_string())
        .collect();
    caps.sort();
    caps
}

// ---------------------------------------------------------------------------
// T4 — the watcher path must verify signatures.
// ---------------------------------------------------------------------------

#[test]
fn watcher_path_rejects_unsigned() {
    let _guard = EnvGuard::remove("GREENTIC_EXT_ALLOW_UNSIGNED");
    let trust = tempfile::TempDir::new().unwrap();

    let fx = unsigned_fixture(ExtensionKind::Design, "greentic.watch-unsigned", "0.1.0");
    let rt = runtime_with_trust_root(trust.path());

    let err = rt
        .handle_added_or_modified(fx.root())
        .expect_err("the hot-reload path must refuse an unsigned extension");
    assert!(
        matches!(err, RuntimeError::SignatureInvalid { .. }),
        "expected SignatureInvalid, got {err:?}"
    );
    assert!(
        rt.loaded().is_empty(),
        "a rejected extension must not be loaded"
    );
}

#[test]
fn watcher_path_rejects_tampered() {
    let _guard = EnvGuard::remove("GREENTIC_EXT_ALLOW_UNSIGNED");
    let trust = tempfile::TempDir::new().unwrap();

    let (fx, _sk) = signed_fixture(ExtensionKind::Design, "greentic.watch-tampered", "0.1.0");
    tamper_fixture(&fx);
    let rt = runtime_with_trust_root(trust.path());

    let err = rt
        .handle_added_or_modified(fx.root())
        .expect_err("the hot-reload path must refuse a tampered extension");
    assert!(
        matches!(err, RuntimeError::SignatureInvalid { .. }),
        "expected SignatureInvalid, got {err:?}"
    );
    assert!(
        rt.loaded().is_empty(),
        "a rejected extension must not be loaded"
    );
}

// ---------------------------------------------------------------------------
// T5 — the registry must be a pure function of what is loaded.
// ---------------------------------------------------------------------------

/// Re-registering an id whose describe no longer offers a capability must drop
/// that capability. Previously every existing offering was cloned forward
/// before appending, so a dropped cap lingered forever AND the surviving ones
/// were duplicated on every reload — a false positive that lets admin's
/// guardrail preflight pass a policy the runtime then fails closed on.
#[test]
fn registry_evicts_prior_offerings_on_replace() {
    let _guard = EnvGuard::remove("GREENTIC_EXT_ALLOW_UNSIGNED");
    let trust = tempfile::TempDir::new().unwrap();

    let (fx, sk) = signed_fixture(ExtensionKind::Design, "greentic.evict", "0.1.0");
    let mut rt = runtime_with_trust_root(trust.path());
    rt.register_loaded_from_dir(fx.root()).expect("first load");
    assert_eq!(
        offered_caps(&rt, "greentic.evict"),
        vec!["greentic:test/ping".to_string()],
        "the fixture's only offered cap must be registered on first load"
    );

    // Re-sign the SAME id with the SAME key (so TOFU accepts it), but with the
    // capability dropped from the describe.
    let describe_path = fx.root().join("describe.json");
    let raw = std::fs::read_to_string(&describe_path).unwrap();
    let mut describe: greentic_extension_sdk_contract::DescribeJson =
        serde_json::from_str(&raw).unwrap();
    describe.capabilities.offered.clear();
    support::finalize_signed_with_manifest(fx.root(), &mut describe, &sk);

    rt.register_loaded_from_dir(fx.root())
        .expect("re-registering with the same key must succeed");

    assert!(
        offered_caps(&rt, "greentic.evict").is_empty(),
        "a capability dropped from the describe must leave the registry; got {:?}",
        offered_caps(&rt, "greentic.evict")
    );
}

/// Re-registering an unchanged extension must not duplicate its offerings.
#[test]
fn registry_does_not_duplicate_offerings_on_reload() {
    let _guard = EnvGuard::remove("GREENTIC_EXT_ALLOW_UNSIGNED");
    let trust = tempfile::TempDir::new().unwrap();

    let (fx, _sk) = signed_fixture(ExtensionKind::Design, "greentic.dupe", "0.1.0");
    let mut rt = runtime_with_trust_root(trust.path());

    rt.register_loaded_from_dir(fx.root()).expect("first load");
    rt.register_loaded_from_dir(fx.root()).expect("second load");
    rt.register_loaded_from_dir(fx.root()).expect("third load");

    assert_eq!(
        offered_caps(&rt, "greentic.dupe"),
        vec!["greentic:test/ping".to_string()],
        "re-registering the same dir must not duplicate its offerings"
    );
}

/// A removed extension's capabilities must leave the registry. Previously
/// `handle_removal` never touched it, so they stayed advertised forever.
#[test]
fn registry_drops_offerings_on_removal() {
    let _guard = EnvGuard::remove("GREENTIC_EXT_ALLOW_UNSIGNED");
    let trust = tempfile::TempDir::new().unwrap();

    let (fx, _sk) = signed_fixture(ExtensionKind::Design, "greentic.removed", "0.1.0");
    let mut rt = runtime_with_trust_root(trust.path());
    rt.register_loaded_from_dir(fx.root()).expect("load");
    assert_eq!(
        offered_caps(&rt, "greentic.removed"),
        vec!["greentic:test/ping".to_string()],
        "precondition: the cap must be registered before removal"
    );

    rt.handle_removal(fx.root());

    assert!(
        rt.loaded().is_empty(),
        "precondition: the extension must be unloaded"
    );
    assert!(
        offered_caps(&rt, "greentic.removed").is_empty(),
        "a removed extension's caps must leave the registry; got {:?}",
        offered_caps(&rt, "greentic.removed")
    );
}

/// The watcher's add path must register capabilities too — previously it never
/// touched the registry, so a hot-reloaded extension's caps never appeared.
#[test]
fn registry_gains_offerings_on_watcher_add() {
    let _guard = EnvGuard::remove("GREENTIC_EXT_ALLOW_UNSIGNED");
    let trust = tempfile::TempDir::new().unwrap();

    let (fx, _sk) = signed_fixture(ExtensionKind::Design, "greentic.watch-add", "0.1.0");
    let rt = runtime_with_trust_root(trust.path());

    rt.handle_added_or_modified(fx.root())
        .expect("hot-reloading a signed extension must succeed");

    assert_eq!(
        offered_caps(&rt, "greentic.watch-add"),
        vec!["greentic:test/ping".to_string()],
        "a hot-reloaded extension's caps must be registered"
    );
}
