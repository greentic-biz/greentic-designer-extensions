use greentic_ext_state::ExtensionState;
use tempfile::TempDir;

#[test]
fn load_returns_default_when_file_missing() {
    let tmp = TempDir::new().unwrap();
    let state = ExtensionState::load(tmp.path()).unwrap();
    // missing file = empty default = everything enabled
    assert!(state.is_enabled("anything", "1.0.0"));
}

#[test]
fn load_parses_existing_state_file() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("extensions-state.json");
    std::fs::write(
        &path,
        r#"{
            "schema": "1.0",
            "default": { "enabled": { "ext.a@1.0.0": false, "ext.b@2.0.0": true } },
            "tenants": {}
        }"#,
    )
    .unwrap();
    let state = ExtensionState::load(tmp.path()).unwrap();
    assert!(!state.is_enabled("ext.a", "1.0.0"));
    assert!(state.is_enabled("ext.b", "2.0.0"));
    assert!(state.is_enabled("ext.c", "1.0.0")); // default true when absent
}

#[test]
fn set_enabled_then_query() {
    let mut state = ExtensionState::default();
    state.set_enabled("ext.x", "0.1.0", false);
    assert!(!state.is_enabled("ext.x", "0.1.0"));
    state.set_enabled("ext.x", "0.1.0", true);
    assert!(state.is_enabled("ext.x", "0.1.0"));
}
