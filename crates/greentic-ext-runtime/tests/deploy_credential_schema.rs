//! End-to-end test for deploy host bindings against a real deploy extension.
//!
//! Gated on `GTDX_TEST_DEPLOY_GTXPACK` env var pointing to a signed
//! `.gtxpack` of a deploy extension (e.g. deploy-aws). Skips locally when
//! unset; CI opts in by building and pointing at a fixture.

use std::path::PathBuf;

use greentic_ext_runtime::{DiscoveryPaths, ExtensionRuntime, RuntimeConfig};

fn load_rt_from_pack() -> Option<(tempfile::TempDir, ExtensionRuntime, String)> {
    let raw = std::env::var("GTDX_TEST_DEPLOY_GTXPACK").ok()?;
    let pack = PathBuf::from(&raw);
    if !pack.exists() {
        eprintln!(
            "skipping: GTDX_TEST_DEPLOY_GTXPACK points to non-existent file: {}",
            pack.display()
        );
        return None;
    }

    let tmp = tempfile::TempDir::new().unwrap();
    let ext_dir = tmp.path().join("ext");
    greentic_ext_testing::unpack_to_dir(&pack, &ext_dir).unwrap();

    let config = RuntimeConfig::from_paths(DiscoveryPaths::new(tmp.path().to_path_buf()));
    let mut rt = ExtensionRuntime::new(config).unwrap();
    rt.register_loaded_from_dir(&ext_dir).unwrap();

    // Find the registered extension id.
    let id = rt
        .loaded()
        .keys()
        .next()
        .map(|k| k.as_str().to_string())
        .expect("at least one extension loaded");

    Some((tmp, rt, id))
}

#[test]
fn list_targets_returns_non_empty_for_deploy_extension() {
    let Some((_tmp, rt, id)) = load_rt_from_pack() else {
        eprintln!("skipping: GTDX_TEST_DEPLOY_GTXPACK not set.");
        return;
    };
    let targets = rt.list_targets(&id).expect("list_targets should succeed");
    assert!(!targets.is_empty(), "expected at least one target");
    let first = &targets[0];
    assert!(!first.id.is_empty());
    assert!(!first.display_name.is_empty());
}

#[test]
fn credential_schema_returns_valid_json_schema() {
    let Some((_tmp, rt, id)) = load_rt_from_pack() else {
        eprintln!("skipping: GTDX_TEST_DEPLOY_GTXPACK not set.");
        return;
    };
    let targets = rt.list_targets(&id).unwrap();
    let target_id = &targets[0].id;
    let schema = rt
        .credential_schema(&id, target_id)
        .expect("credential_schema should succeed");
    let parsed: serde_json::Value =
        serde_json::from_str(&schema).expect("schema should be valid JSON");
    assert_eq!(
        parsed.get("type").and_then(|v| v.as_str()),
        Some("object"),
        "credential schema top-level type should be 'object', got: {schema}"
    );
}

#[test]
fn validate_credentials_returns_diagnostics_slice() {
    let Some((_tmp, rt, id)) = load_rt_from_pack() else {
        eprintln!("skipping: GTDX_TEST_DEPLOY_GTXPACK not set.");
        return;
    };
    let targets = rt.list_targets(&id).unwrap();
    let target_id = &targets[0].id;
    // Empty JSON object is a valid shape; diagnostics may or may not be empty
    // depending on the target's schema — just assert the call returns Ok.
    let result = rt
        .validate_credentials(&id, target_id, r"{}")
        .expect("validate_credentials should succeed");
    // Sanity check: if diagnostics are present, each one has non-empty message.
    for d in &result {
        assert!(!d.message.is_empty(), "diagnostic message should be non-empty");
    }
}
