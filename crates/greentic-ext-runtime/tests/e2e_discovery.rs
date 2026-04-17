use std::fs;

use greentic_ext_contract::ExtensionKind;
use greentic_ext_runtime::{DiscoveryPaths, ExtensionRuntime, RuntimeConfig};
use greentic_ext_testing::ExtensionFixtureBuilder;
use tempfile::TempDir;

fn copy_fixture(src: &std::path::Path, dst: &std::path::Path) {
    fs::create_dir_all(dst).unwrap();
    for e in fs::read_dir(src).unwrap() {
        let e = e.unwrap();
        fs::copy(e.path(), dst.join(e.file_name())).unwrap();
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn end_to_end_discovery_and_capability_resolution() {
    let tmp = TempDir::new().unwrap();
    let user_root = tmp.path().join("user");
    fs::create_dir_all(user_root.join("design")).unwrap();
    fs::create_dir_all(user_root.join("bundle")).unwrap();

    let offerer = ExtensionFixtureBuilder::new(ExtensionKind::Design, "greentic.offerer", "1.2.0")
        .offer("greentic:x/service", "1.2.0")
        .with_wasm(wat::parse_str("(component)").unwrap())
        .build()
        .unwrap();

    let consumer =
        ExtensionFixtureBuilder::new(ExtensionKind::Bundle, "greentic.consumer", "0.1.0")
            .require("greentic:x/service", "^1.0")
            .with_wasm(wat::parse_str("(component)").unwrap())
            .build()
            .unwrap();

    copy_fixture(
        offerer.root(),
        &user_root.join("design/greentic.offerer-1.2.0"),
    );
    copy_fixture(
        consumer.root(),
        &user_root.join("bundle/greentic.consumer-0.1.0"),
    );

    let mut rt = ExtensionRuntime::new(RuntimeConfig::from_paths(DiscoveryPaths::new(
        user_root.clone(),
    )))
    .unwrap();

    for kind in ["design", "bundle"] {
        for path in greentic_ext_runtime::discovery::scan_kind_dir(&user_root.join(kind)).unwrap() {
            rt.register_loaded_from_dir(&path).unwrap();
        }
    }

    let registry = rt.capability_registry();
    let plan = registry.resolve(
        "greentic.consumer",
        &[greentic_ext_contract::CapabilityRef {
            id: "greentic:x/service".parse().unwrap(),
            version: "^1.0".into(),
        }],
    );
    assert!(
        plan.unresolved.is_empty(),
        "unresolved: {:?}",
        plan.unresolved
    );
    assert_eq!(plan.resolved.len(), 1);
}
