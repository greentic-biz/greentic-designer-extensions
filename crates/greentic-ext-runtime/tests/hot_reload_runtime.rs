use std::fs;
use std::sync::Arc;
use std::time::Duration;

use greentic_ext_runtime::{DiscoveryPaths, ExtensionRuntime, RuntimeConfig};
use greentic_extension_sdk_contract::ExtensionKind;
use greentic_extension_sdk_testing::ExtensionFixtureBuilder;
use tempfile::TempDir;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn hot_reload_picks_up_new_extension() {
    let tmp = TempDir::new().unwrap();
    let user_root = tmp.path().join("user");
    let design_dir = user_root.join("design");
    fs::create_dir_all(&design_dir).unwrap();

    let config = RuntimeConfig::from_paths(DiscoveryPaths::new(user_root));
    let rt = Arc::new(ExtensionRuntime::new(config).unwrap());
    let guard = rt.clone().start_watcher().unwrap();

    // Give the watcher time to settle before writing files.
    tokio::time::sleep(Duration::from_millis(400)).await;

    let fixture = ExtensionFixtureBuilder::new(ExtensionKind::Design, "greentic.hot", "0.1.0")
        .offer("greentic:hot/ping", "1.0.0")
        .with_wasm(wat::parse_str("(component)").unwrap())
        .build()
        .unwrap();

    // Patch gtpack so the runtime can resolve the wasm path (sdk-testing
    // 1.2.0-research leaves gtpack=None on every component).
    {
        use greentic_extension_sdk_contract::describe::provider::RuntimeGtpack;
        let path = fixture.root().join("describe.json");
        let raw = fs::read_to_string(&path).unwrap();
        let mut describe: greentic_extension_sdk_contract::DescribeJson =
            serde_json::from_str(&raw).unwrap();
        for component in describe.runtime.components.values_mut() {
            if component.gtpack.is_none() {
                component.gtpack = Some(RuntimeGtpack {
                    file: "extension.wasm".to_string(),
                    sha256: "0".repeat(64),
                    pack_id: describe.metadata.id.clone(),
                    component_version: describe.metadata.version.clone(),
                });
            }
        }
        fs::write(&path, serde_json::to_string_pretty(&describe).unwrap()).unwrap();
    }

    let target = design_dir.join("greentic.hot-0.1.0");
    fs::create_dir_all(&target).unwrap();
    for e in fs::read_dir(fixture.root()).unwrap() {
        let e = e.unwrap();
        fs::copy(e.path(), target.join(e.file_name())).unwrap();
    }

    // Wait for the debouncer (500ms) + processing time.
    tokio::time::sleep(Duration::from_secs(2)).await;

    let loaded = rt.loaded();
    // guard drops here, cleanly stopping the watcher thread.
    drop(guard);

    assert!(
        loaded.values().any(|e| e.id.as_str() == "greentic.hot"),
        "extension should be loaded; got: {:?}",
        loaded.keys().collect::<Vec<_>>()
    );
}
