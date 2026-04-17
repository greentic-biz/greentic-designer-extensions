use std::path::PathBuf;
use std::sync::mpsc;
use std::time::Duration;

use notify::RecursiveMode;
use notify_debouncer_full::{new_debouncer, DebounceEventResult};

use crate::error::RuntimeError;

#[derive(Debug, Clone)]
pub enum FsEvent {
    Added(PathBuf),
    Modified(PathBuf),
    Removed(PathBuf),
}

/// Start watching `paths` recursively. Returns a channel receiver emitting
/// coalesced FS events. The watcher runs in a background thread owned by
/// `notify-debouncer-full` — we intentionally leak it so the channel stays
/// open for the lifetime of the caller.
pub fn watch(paths: &[PathBuf]) -> Result<mpsc::Receiver<FsEvent>, RuntimeError> {
    let (tx, rx) = mpsc::channel();
    let tx_clone = tx.clone();
    let mut debouncer = new_debouncer(
        Duration::from_millis(500),
        None,
        move |res: DebounceEventResult| {
            if let Ok(events) = res {
                for ev in events {
                    for p in &ev.event.paths {
                        let out = match ev.event.kind {
                            notify::EventKind::Create(_) => FsEvent::Added(p.clone()),
                            notify::EventKind::Modify(_) => FsEvent::Modified(p.clone()),
                            notify::EventKind::Remove(_) => FsEvent::Removed(p.clone()),
                            _ => continue,
                        };
                        let _ = tx_clone.send(out);
                    }
                }
            }
        },
    )
    .map_err(|e| RuntimeError::Watcher(e.to_string()))?;

    for p in paths {
        if p.exists() {
            debouncer
                .watch(p, RecursiveMode::Recursive)
                .map_err(|e| RuntimeError::Watcher(e.to_string()))?;
        }
    }
    // Keep the debouncer alive — drop(tx) would close the channel prematurely.
    std::mem::forget(debouncer);
    drop(tx);
    Ok(rx)
}
