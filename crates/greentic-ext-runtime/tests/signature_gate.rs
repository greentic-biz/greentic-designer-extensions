#[path = "support/mod.rs"]
mod support;

use std::path::PathBuf;

use greentic_ext_runtime::{DiscoveryPaths, ExtensionRuntime, RuntimeConfig, RuntimeError};
use greentic_extension_sdk_contract::ExtensionKind;

use support::{EnvGuard, signed_fixture, tamper_fixture, unsigned_fixture};

/// A runtime rooted at a throwaway trust store.
///
/// The returned `TempDir` must be held for the test's lifetime — dropping it
/// deletes the store. Pointing this at a temp dir is not optional: the default
/// root resolves to the developer's real `~/.greentic/trust/publishers.json`,
/// and `signed_fixture` mints a fresh random key per call, so a default-rooted
/// test would pin a junk key on its first run and then fail every subsequent
/// run with `PublisherKeyChanged`.
fn new_runtime() -> (ExtensionRuntime, tempfile::TempDir) {
    let trust = tempfile::TempDir::new().expect("temp trust root");
    let config = RuntimeConfig::from_paths(DiscoveryPaths::new(PathBuf::from("/dev/null")))
        .with_trust_root(trust.path().to_path_buf());
    (ExtensionRuntime::new(config).unwrap(), trust)
}

#[test]
fn rejects_unsigned_by_default() {
    let _guard = EnvGuard::remove("GREENTIC_EXT_ALLOW_UNSIGNED");

    let fx = unsigned_fixture(ExtensionKind::Design, "greentic.unsigned", "0.1.0");
    let (mut rt, _trust) = new_runtime();
    let err = rt.register_loaded_from_dir(fx.root()).unwrap_err();
    match err {
        RuntimeError::SignatureInvalid {
            extension_id,
            reason,
        } => {
            assert_eq!(extension_id, "greentic.unsigned");
            assert!(reason.contains("missing signature"), "got: {reason}");
        }
        other => panic!("expected SignatureInvalid, got {other:?}"),
    }
}

#[test]
fn rejects_tampered_signature() {
    let _guard = EnvGuard::remove("GREENTIC_EXT_ALLOW_UNSIGNED");

    let (fx, _sk) = signed_fixture(ExtensionKind::Design, "greentic.tampered", "0.1.0");
    tamper_fixture(&fx);
    let (mut rt, _trust) = new_runtime();
    let err = rt.register_loaded_from_dir(fx.root()).unwrap_err();
    assert!(matches!(err, RuntimeError::SignatureInvalid { .. }));
}

#[test]
fn accepts_signed_by_default() {
    let _guard = EnvGuard::remove("GREENTIC_EXT_ALLOW_UNSIGNED");

    let (fx, _sk) = signed_fixture(ExtensionKind::Design, "greentic.signed", "0.1.0");
    let (mut rt, _trust) = new_runtime();
    rt.register_loaded_from_dir(fx.root()).expect("load signed");
}

#[cfg(feature = "dev-allow-unsigned")]
#[test]
fn allow_unsigned_env_bypasses() {
    let _guard = EnvGuard::set("GREENTIC_EXT_ALLOW_UNSIGNED", "1");
    let fx = unsigned_fixture(ExtensionKind::Design, "greentic.bypass", "0.1.0");
    let (mut rt, _trust) = new_runtime();
    rt.register_loaded_from_dir(fx.root())
        .expect("load unsigned with env");
}

#[cfg(feature = "dev-allow-unsigned")]
#[test]
fn allow_unsigned_env_bypasses_even_if_tampered() {
    let _guard = EnvGuard::set("GREENTIC_EXT_ALLOW_UNSIGNED", "1");
    let (fx, _sk) = signed_fixture(ExtensionKind::Design, "greentic.bypass-tampered", "0.1.0");
    tamper_fixture(&fx);
    let (mut rt, _trust) = new_runtime();
    // Skip-entirely semantics per design §4: env set = don't even verify.
    rt.register_loaded_from_dir(fx.root())
        .expect("load tampered with env");
}

/// When the `dev-allow-unsigned` feature is OFF (production build), the env
/// var must NOT bypass signature verification — even if set.
#[cfg(not(feature = "dev-allow-unsigned"))]
#[test]
fn allow_unsigned_env_is_ignored_without_feature() {
    let _guard = EnvGuard::set("GREENTIC_EXT_ALLOW_UNSIGNED", "1");
    let fx = unsigned_fixture(ExtensionKind::Design, "greentic.no-bypass", "0.1.0");
    let (mut rt, _trust) = new_runtime();
    let err = rt.register_loaded_from_dir(fx.root()).unwrap_err();
    assert!(
        matches!(err, RuntimeError::SignatureInvalid { .. }),
        "without dev-allow-unsigned, env var must NOT bypass signature check; got {err:?}",
    );
}

/// A production build must not honour the escape hatch at all.
///
/// The bypass lives behind `#[cfg(feature = "dev-allow-unsigned")]`, so with
/// the feature off the branch is not compiled and the env var is inert. That is
/// the claim `CLAUDE.md` makes about production builds, and it is exactly the
/// claim nothing was checking: every other test in this suite runs under
/// `--all-features`, where the hatch *is* compiled in and is merely unset.
///
/// This test compiles only in the default (no-feature) shape, which is why
/// `ci/local_check.sh` runs the suite both ways.
#[cfg(not(feature = "dev-allow-unsigned"))]
#[test]
fn a_production_build_ignores_the_unsigned_escape_hatch() {
    let _guard = EnvGuard::set("GREENTIC_EXT_ALLOW_UNSIGNED", "1");

    let fx = unsigned_fixture(ExtensionKind::Design, "greentic.hatch-off", "0.1.0");
    let (mut rt, _trust) = new_runtime();
    let err = rt
        .register_loaded_from_dir(fx.root())
        .expect_err("the escape hatch must not exist without the dev-allow-unsigned feature");
    assert!(
        matches!(err, RuntimeError::SignatureInvalid { .. }),
        "expected the signature gate to reject regardless of the env var, got {err:?}"
    );
}
