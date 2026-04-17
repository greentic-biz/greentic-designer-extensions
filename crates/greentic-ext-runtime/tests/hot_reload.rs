use std::fs;
use std::thread::sleep;
use std::time::Duration;

use greentic_ext_runtime::watcher::{watch, FsEvent};
use tempfile::TempDir;

#[test]
fn watcher_detects_new_file() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path().to_path_buf();
    let rx = watch(&[root.clone()]).unwrap();

    sleep(Duration::from_millis(300));
    let new_file = root.join("newfile.txt");
    fs::write(&new_file, "hello").unwrap();

    let mut saw_event = false;
    let deadline = std::time::Instant::now() + Duration::from_secs(3);
    while std::time::Instant::now() < deadline {
        if let Ok(ev) = rx.recv_timeout(Duration::from_millis(200)) {
            if matches!(ev, FsEvent::Added(_) | FsEvent::Modified(_)) {
                saw_event = true;
                break;
            }
        }
    }
    assert!(saw_event, "expected FsEvent::Added/Modified");
}
