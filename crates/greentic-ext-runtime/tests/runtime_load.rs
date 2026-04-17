use std::path::PathBuf;

use greentic_ext_contract::ExtensionKind;
use greentic_ext_runtime::{DiscoveryPaths, ExtensionRuntime, RuntimeConfig};
use greentic_ext_testing::ExtensionFixtureBuilder;

#[tokio::test]
async fn loads_extension_and_registers_caps() {
    let minimal_wasm = wat::parse_str(r#"(component)"#).expect("component must compile");
    let fixture = ExtensionFixtureBuilder::new(
        ExtensionKind::Design,
        "greentic.test-ext",
        "0.1.0",
    )
    .offer("greentic:test/ping", "1.0.0")
    .with_wasm(minimal_wasm)
    .build()
    .unwrap();

    let config = RuntimeConfig::from_paths(DiscoveryPaths::new(PathBuf::from("/dev/null")));
    let mut rt = ExtensionRuntime::new(config).unwrap();
    rt.register_loaded_from_dir(fixture.root()).unwrap();

    let registry = rt.capability_registry();
    assert!(registry.offerings().any(|o| o.extension_id == "greentic.test-ext"));
}
