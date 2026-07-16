#[path = "support/mod.rs"]
mod support;

use std::fs;
use std::sync::Arc;
use std::time::Duration;

use greentic_ext_runtime::{DiscoveryPaths, ExtensionRuntime, RuntimeConfig};
use greentic_extension_sdk_contract::ExtensionKind;
use tempfile::TempDir;

use support::signed_fixture;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn hot_reload_picks_up_new_extension() {
    let tmp = TempDir::new().unwrap();
    let user_root = tmp.path().join("user");
    let design_dir = user_root.join("design");
    fs::create_dir_all(&design_dir).unwrap();
    // Throwaway trust root: the first load pins the fixture's key, and the
    // default root would be the developer's real ~/.greentic.
    let trust = TempDir::new().unwrap();

    let config = RuntimeConfig::from_paths(DiscoveryPaths::new(user_root))
        .with_trust_root(trust.path().to_path_buf());
    let rt = Arc::new(ExtensionRuntime::new(config).unwrap());
    let guard = rt.clone().start_watcher().unwrap();

    // Give the watcher time to settle before writing files.
    tokio::time::sleep(Duration::from_millis(400)).await;

    // The fixture must be signed. The watcher path now enforces the same
    // signature gate as explicit registration; this test previously used an
    // unsigned, manifest-less fixture and passed only because that path
    // verified nothing at all. `signed_fixture` also patches gtpack so the
    // loader can resolve the wasm path.
    let (fixture, _sk) = signed_fixture(ExtensionKind::Design, "greentic.hot", "0.1.0");

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
