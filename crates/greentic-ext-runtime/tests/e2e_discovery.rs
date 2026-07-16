use std::fs;

use ed25519_dalek::SigningKey;
use greentic_ext_runtime::{DiscoveryPaths, ExtensionRuntime, RuntimeConfig};
use greentic_extension_sdk_contract::ExtensionKind;
use greentic_extension_sdk_testing::ExtensionFixtureBuilder;
use rand::rngs::OsRng;
use tempfile::TempDir;

#[path = "support/mod.rs"]
mod support;

fn copy_fixture(src: &std::path::Path, dst: &std::path::Path) {
    fs::create_dir_all(dst).unwrap();
    for e in fs::read_dir(src).unwrap() {
        let e = e.unwrap();
        fs::copy(e.path(), dst.join(e.file_name())).unwrap();
    }
}

/// Patch `runtime.components[*].gtpack` for source-dir loads, then build the
/// whole-archive manifest, bind it, and sign — so the dir passes the runtime's
/// hardened verify chain (audit P5). Operates on the post-copy discovery dir.
fn sign_fixture_dir(dir: &std::path::Path) {
    let path = dir.join("describe.json");
    let raw = fs::read_to_string(&path).unwrap();
    let mut describe: greentic_extension_sdk_contract::DescribeJson =
        serde_json::from_str(&raw).unwrap();
    support::populate_gtpack_for_local_load(&mut describe);
    let sk = SigningKey::generate(&mut OsRng);
    support::finalize_signed_with_manifest(dir, &mut describe, &sk);
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

    let offerer_dst = user_root.join("design/greentic.offerer-1.2.0");
    let consumer_dst = user_root.join("bundle/greentic.consumer-0.1.0");

    copy_fixture(offerer.root(), &offerer_dst);
    copy_fixture(consumer.root(), &consumer_dst);

    // Sign both after copy so the runtime's verify gate accepts them.
    sign_fixture_dir(&offerer_dst);
    sign_fixture_dir(&consumer_dst);

    // Trust root inside the existing tempdir: the default root is the
    // developer's real ~/.greentic, which tests must never pin into.
    let mut rt = ExtensionRuntime::new(
        RuntimeConfig::from_paths(DiscoveryPaths::new(user_root.clone()))
            .with_trust_root(tmp.path().join("trust-root")),
    )
    .unwrap();

    for kind in ["design", "bundle"] {
        for path in greentic_ext_runtime::discovery::scan_kind_dir(&user_root.join(kind)).unwrap() {
            rt.register_loaded_from_dir(&path).unwrap();
        }
    }

    let registry = rt.capability_registry();
    let plan = registry.resolve(
        "greentic.consumer",
        &[greentic_extension_sdk_contract::CapabilityRef {
            id: "greentic:x/service".parse().unwrap(),
            version: "^1.0".into(),
            deprecated: None,
        }],
    );
    assert!(
        plan.unresolved.is_empty(),
        "unresolved: {:?}",
        plan.unresolved
    );
    assert_eq!(plan.resolved.len(), 1);
}
