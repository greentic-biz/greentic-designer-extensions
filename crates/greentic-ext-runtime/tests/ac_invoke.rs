use std::path::Path;

use greentic_ext_runtime::{DiscoveryPaths, ExtensionRuntime, RuntimeConfig};

#[test]
fn invoke_validate_card_on_ac_extension() {
    let pack = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("reference-extensions/adaptive-cards/greentic.adaptive-cards-1.6.0.gtxpack");

    if !pack.exists() {
        eprintln!(
            "skipping: AC .gtxpack not built. run reference-extensions/adaptive-cards/build.sh first"
        );
        return;
    }

    let tmp = tempfile::TempDir::new().unwrap();
    let ext_dir = tmp.path().join("ext");
    greentic_ext_testing::unpack_to_dir(&pack, &ext_dir).unwrap();

    let config = RuntimeConfig::from_paths(DiscoveryPaths::new(tmp.path().to_path_buf()));
    let mut rt = ExtensionRuntime::new(config).unwrap();
    rt.register_loaded_from_dir(&ext_dir).unwrap();

    let result = rt
        .invoke_tool(
            "greentic.adaptive-cards",
            "validate_card",
            r#"{"card":{"type":"AdaptiveCard","version":"1.6"}}"#,
        )
        .expect("invoke_tool should succeed on valid card");

    let parsed: serde_json::Value =
        serde_json::from_str(&result).expect("result should be valid JSON");
    assert_eq!(
        parsed["valid"],
        serde_json::Value::Bool(true),
        "expected valid=true for a minimal valid AdaptiveCard, got: {parsed:#}"
    );
    eprintln!("AC extension validate_card returned: {parsed:#}");
}
