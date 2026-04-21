//! End-to-end test for `ExtensionRuntime::validate_content` against the
//! real adaptive-cards extension.
//!
//! Mirrors the gating of `ac_invoke.rs`: skips when `GTDX_TEST_GTXPACK` is
//! unset or points to a missing artifact, so the test stays optional
//! locally while CI can opt-in by building the extension first.

use std::path::PathBuf;

use greentic_ext_runtime::{DiscoveryPaths, ExtensionRuntime, RuntimeConfig, Severity};

#[test]
fn validate_content_on_ac_extension_accepts_minimal_card() {
    let Ok(raw) = std::env::var("GTDX_TEST_GTXPACK") else {
        eprintln!("skipping: GTDX_TEST_GTXPACK not set (see ac_invoke.rs for setup).");
        return;
    };
    let pack = PathBuf::from(raw);
    if !pack.exists() {
        eprintln!(
            "skipping: GTDX_TEST_GTXPACK points to non-existent file: {}",
            pack.display()
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
        .validate_content(
            "greentic.adaptive-cards",
            "AdaptiveCard",
            r#"{"type":"AdaptiveCard","version":"1.6"}"#,
        )
        .expect("validate_content should succeed on a valid card");

    assert!(result.valid, "expected valid=true, got: {result:#?}");
    let errors = result
        .diagnostics
        .iter()
        .filter(|d| d.severity == Severity::Error)
        .count();
    assert_eq!(
        errors, 0,
        "expected zero error diagnostics, got: {result:#?}"
    );
}
