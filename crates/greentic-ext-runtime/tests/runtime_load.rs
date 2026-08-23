#[path = "support/mod.rs"]
mod support;

use std::path::PathBuf;

use greentic_ext_runtime::{DiscoveryPaths, ExtensionRuntime, RuntimeConfig};
use greentic_extension_sdk_contract::ExtensionKind;

use support::signed_fixture;

#[tokio::test]
async fn loads_extension_and_registers_caps() {
    let (fixture, _sk) = signed_fixture(ExtensionKind::Design, "greentic.test-ext", "0.1.0");

    // Trust root must be a temp dir: the default is the developer's real
    // ~/.greentic, and signed_fixture mints a fresh key per call, so a
    // default-rooted test pins junk then fails every later run.
    let trust = tempfile::TempDir::new().expect("temp trust root");
    let config = RuntimeConfig::from_paths(DiscoveryPaths::new(PathBuf::from("/dev/null")))
        .with_trust_root(trust.path().to_path_buf());
    let mut rt = ExtensionRuntime::new(config).unwrap();
    rt.register_loaded_from_dir(fixture.root()).unwrap();

    let registry = rt.capability_registry();
    assert!(
        registry
            .offerings()
            .any(|o| o.extension_id == "greentic.test-ext")
    );
}
