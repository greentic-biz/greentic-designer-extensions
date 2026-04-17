use greentic_ext_contract::DescribeJson;

const AC_FIXTURE: &str = r#"{
  "apiVersion": "greentic.ai/v1",
  "kind": "DesignExtension",
  "metadata": {
    "id": "greentic.adaptive-cards",
    "name": "Adaptive Cards",
    "version": "1.6.0",
    "summary": "Design AdaptiveCards v1.6",
    "author": { "name": "Greentic" },
    "license": "MIT"
  },
  "engine": {
    "greenticDesigner": ">=0.6.0",
    "extRuntime": "^0.1.0"
  },
  "capabilities": {
    "offered": [{ "id": "greentic:adaptive-cards/validate", "version": "1.0.0" }],
    "required": [{ "id": "greentic:host/logging", "version": "^1.0.0" }]
  },
  "runtime": {
    "component": "extension.wasm",
    "memoryLimitMB": 64,
    "permissions": {}
  },
  "contributions": {}
}"#;

#[test]
fn ac_fixture_parses() {
    let d: DescribeJson = serde_json::from_str(AC_FIXTURE).unwrap();
    assert_eq!(d.metadata.id, "greentic.adaptive-cards");
    assert_eq!(d.identity_key(), "greentic.adaptive-cards@1.6.0");
    assert_eq!(d.capabilities.offered.len(), 1);
    assert_eq!(d.runtime.memory_limit_mb, 64);
}

#[test]
fn round_trips_without_data_loss() {
    let d: DescribeJson = serde_json::from_str(AC_FIXTURE).unwrap();
    let serialized = serde_json::to_string(&d).unwrap();
    let parsed_back: DescribeJson = serde_json::from_str(&serialized).unwrap();
    assert_eq!(parsed_back.metadata.id, d.metadata.id);
}
