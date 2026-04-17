use std::fs;
use std::sync::Arc;
use std::time::Duration;

use greentic_ext_contract::ExtensionKind;
use greentic_ext_runtime::{DiscoveryPaths, ExtensionRuntime, RuntimeConfig};
use greentic_ext_testing::ExtensionFixtureBuilder;
use tempfile::TempDir;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn hot_reload_picks_up_new_extension() {
    let tmp = TempDir::new().unwrap();
    let user_root = tmp.path().join("user");
    let design_dir = user_root.join("design");
    fs::create_dir_all(&design_dir).unwrap();

    let config = RuntimeConfig::from_paths(DiscoveryPaths::new(user_root));
    let rt = Arc::new(ExtensionRuntime::new(config).unwrap());
    let _guard = rt.clone().start_watcher().unwrap();

    // Give the watcher time to settle before writing files.
    tokio::time::sleep(Duration::from_millis(400)).await;

    let fixture = ExtensionFixtureBuilder::new(ExtensionKind::Design, "greentic.hot", "0.1.0")
        .offer("greentic:hot/ping", "1.0.0")
        .with_wasm(wat::parse_str("(component)").unwrap())
        .build()
        .unwrap();

    let target = design_dir.join("greentic.hot-0.1.0");
    fs::create_dir_all(&target).unwrap();
    for e in fs::read_dir(fixture.root()).unwrap() {
        let e = e.unwrap();
        fs::copy(e.path(), target.join(e.file_name())).unwrap();
    }

    // Wait for the debouncer (500ms) + processing time.
    tokio::time::sleep(Duration::from_millis(2000)).await;

    let loaded = rt.loaded();
    // _guard drops here, cleanly stopping the watcher thread.
    drop(_guard);

    assert!(
        loaded.values().any(|e| e.id.as_str() == "greentic.hot"),
        "extension should be loaded; got: {:?}",
        loaded.keys().collect::<Vec<_>>()
    );
}
