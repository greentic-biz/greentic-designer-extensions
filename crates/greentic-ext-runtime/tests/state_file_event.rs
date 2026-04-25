//! Verifies the `StateFileChanged` event variant exists, and that the
//! watcher emits it when `~/.greentic/extensions-state.json` is created or
//! modified.

use std::sync::Arc;
use std::time::Duration;

use greentic_ext_runtime::{DiscoveryPaths, ExtensionRuntime, RuntimeConfig, RuntimeEvent};
use tempfile::TempDir;

#[test]
fn runtime_event_has_state_file_changed_variant() {
    let event = RuntimeEvent::StateFileChanged;
    // Smoke test: enum variant exists, is Debug + Clone.
    let _cloned = event.clone();
    let _ = format!("{event:?}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn watcher_emits_state_file_changed_on_state_file_create_or_modify() {
    let tmp = TempDir::new().unwrap();
    // The runtime convention is: `user` is the dir containing kind subdirs
    // (e.g. `<user>/design/`, `<user>/bundle/`). The state file lives in
    // the parent of `user` (the `home` dir).
    let home = tmp.path().to_path_buf();
    let user_root = home.join("extensions");
    for kind in ["design", "bundle", "deploy", "provider"] {
        std::fs::create_dir_all(user_root.join(kind)).unwrap();
    }

    let config = RuntimeConfig::from_paths(DiscoveryPaths::new(user_root.clone()));
    let runtime = Arc::new(ExtensionRuntime::new(config).unwrap());
    let mut rx = runtime.subscribe();

    let _guard = runtime.clone().start_watcher().unwrap();

    // Give the watcher time to settle before writing files.
    tokio::time::sleep(Duration::from_millis(400)).await;

    // Trigger state file write at `<home>/extensions-state.json`.
    std::fs::write(home.join("extensions-state.json"), r#"{"schema":"1.0"}"#).unwrap();

    // Drain events until we see StateFileChanged or timeout.
    let deadline = tokio::time::Instant::now() + Duration::from_secs(3);
    let mut got_state_file = false;
    while tokio::time::Instant::now() < deadline {
        let event = tokio::time::timeout(Duration::from_millis(200), rx.recv()).await;
        if let Ok(Ok(RuntimeEvent::StateFileChanged)) = event {
            got_state_file = true;
            break;
        }
    }

    assert!(got_state_file, "did not receive StateFileChanged within 3s");
}
