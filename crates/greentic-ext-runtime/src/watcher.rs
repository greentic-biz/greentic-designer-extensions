use std::path::PathBuf;
use std::sync::mpsc;
use std::time::Duration;

use notify::RecursiveMode;
use notify_debouncer_full::{
    new_debouncer, DebounceEventResult, Debouncer, RecommendedCache,
};
use notify::RecommendedWatcher;

use crate::error::RuntimeError;

#[derive(Debug, Clone)]
pub enum FsEvent {
    Added(PathBuf),
    Modified(PathBuf),
    Removed(PathBuf),
}

/// RAII handle that keeps the debouncer alive. Drop this to stop watching.
pub struct WatchHandle {
    _debouncer: Debouncer<RecommendedWatcher, RecommendedCache>,
}

/// Start watching `paths` recursively. Returns a channel receiver emitting
/// coalesced FS events and a `WatchHandle` that owns the debouncer — drop
/// the handle to stop watching and close the channel.
pub fn watch(paths: &[PathBuf]) -> Result<(mpsc::Receiver<FsEvent>, WatchHandle), RuntimeError> {
    let (tx, rx) = mpsc::channel();
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
                        let _ = tx.send(out);
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
    Ok((rx, WatchHandle { _debouncer: debouncer }))
}
