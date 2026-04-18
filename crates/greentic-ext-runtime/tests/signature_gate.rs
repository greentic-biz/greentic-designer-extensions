#[path = "support/mod.rs"]
mod support;

use std::path::PathBuf;

use greentic_ext_contract::ExtensionKind;
use greentic_ext_runtime::{DiscoveryPaths, ExtensionRuntime, RuntimeConfig, RuntimeError};

use support::{signed_fixture, tamper_fixture, unsigned_fixture, EnvGuard};

fn new_runtime() -> ExtensionRuntime {
    let config = RuntimeConfig::from_paths(DiscoveryPaths::new(PathBuf::from("/dev/null")));
    ExtensionRuntime::new(config).unwrap()
}

#[test]
fn rejects_unsigned_by_default() {
    let _guard = EnvGuard::remove("GREENTIC_EXT_ALLOW_UNSIGNED");

    let fx = unsigned_fixture(ExtensionKind::Design, "greentic.unsigned", "0.1.0");
    let mut rt = new_runtime();
    let err = rt.register_loaded_from_dir(fx.root()).unwrap_err();
    match err {
        RuntimeError::SignatureInvalid { extension_id, reason } => {
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
    let mut rt = new_runtime();
    let err = rt.register_loaded_from_dir(fx.root()).unwrap_err();
    assert!(matches!(err, RuntimeError::SignatureInvalid { .. }));
}

#[test]
fn accepts_signed_by_default() {
    let _guard = EnvGuard::remove("GREENTIC_EXT_ALLOW_UNSIGNED");

    let (fx, _sk) = signed_fixture(ExtensionKind::Design, "greentic.signed", "0.1.0");
    let mut rt = new_runtime();
    rt.register_loaded_from_dir(fx.root()).expect("load signed");
}

#[test]
fn allow_unsigned_env_bypasses() {
    let _guard = EnvGuard::set("GREENTIC_EXT_ALLOW_UNSIGNED", "1");
    let fx = unsigned_fixture(ExtensionKind::Design, "greentic.bypass", "0.1.0");
    let mut rt = new_runtime();
    rt.register_loaded_from_dir(fx.root()).expect("load unsigned with env");
}

#[test]
fn allow_unsigned_env_bypasses_even_if_tampered() {
    let _guard = EnvGuard::set("GREENTIC_EXT_ALLOW_UNSIGNED", "1");
    let (fx, _sk) = signed_fixture(ExtensionKind::Design, "greentic.bypass-tampered", "0.1.0");
    tamper_fixture(&fx);
    let mut rt = new_runtime();
    // Skip-entirely semantics per design §4: env set = don't even verify.
    rt.register_loaded_from_dir(fx.root()).expect("load tampered with env");
}
