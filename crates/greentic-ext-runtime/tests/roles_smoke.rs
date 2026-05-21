use greentic_ext_runtime::{DiscoveryPaths, ExtensionRuntime, RuntimeConfig, RuntimeError};
use tempfile::tempdir;

#[test]
fn list_roles_unknown_extension_returns_not_found() {
    let dir = tempdir().expect("tempdir");
    let cfg = RuntimeConfig::from_paths(DiscoveryPaths::new(dir.path().to_path_buf()));
    let rt = ExtensionRuntime::new(cfg).expect("new runtime");
    let err = rt.list_roles("does.not.exist").unwrap_err();
    assert!(
        matches!(err, RuntimeError::NotFound(_)),
        "expected NotFound, got {err:?}"
    );
}

#[test]
fn validate_role_unknown_extension_returns_not_found() {
    let dir = tempdir().expect("tempdir");
    let cfg = RuntimeConfig::from_paths(DiscoveryPaths::new(dir.path().to_path_buf()));
    let rt = ExtensionRuntime::new(cfg).expect("new runtime");
    let err = rt
        .validate_role("does.not.exist", "topic_picker", "{}")
        .unwrap_err();
    assert!(
        matches!(err, RuntimeError::NotFound(_)),
        "expected NotFound, got {err:?}"
    );
}

#[test]
fn compile_role_unknown_extension_returns_unknown_role() {
    use greentic_ext_runtime::{RoleError, TargetKind};

    let dir = tempdir().expect("tempdir");
    let cfg = RuntimeConfig::from_paths(DiscoveryPaths::new(dir.path().to_path_buf()));
    let rt = ExtensionRuntime::new(cfg).expect("new runtime");
    let err = rt
        .compile_role(
            "does.not.exist",
            "topic_picker",
            TargetKind::AdaptiveCard,
            "{}",
            None,
        )
        .unwrap_err();
    assert!(
        matches!(err, RoleError::UnknownRole(_)),
        "expected RoleError::UnknownRole, got {err:?}"
    );
}
