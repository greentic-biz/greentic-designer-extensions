//! Registration, capability-registry derivation, and the hot-reload watcher.
//!
//! Split out of [`crate::runtime`] so the mutation paths for `loaded` live
//! together: every one of them must rebuild the capability registry wholesale
//! (see [`ExtensionRuntime::rebuild_registry`]), and keeping them in one file
//! is what makes a new mutation path that forgets to obvious on review.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::capability::{CapabilityRegistry, OfferedBinding};
use crate::error::RuntimeError;
use crate::loaded::{ExtensionId, LoadedExtension, LoadedExtensionRef};
use crate::runtime::{ExtensionRuntime, RuntimeEvent, WatcherGuard};

/// Filename of the persistent enable/disable state document, located at
/// `<home>/extensions-state.json`. Kept in sync with the constant of the
/// same name in `greentic-ext-state` (single source of truth lives there;
/// this duplicate exists only because the runtime intentionally does not
/// depend on `greentic-ext-state` to avoid a circular crate dependency).
const STATE_FILENAME: &str = "extensions-state.json";

/// How long the watcher thread waits on the event channel before re-checking
/// its stop signal. Bounds shutdown latency; `WatcherGuard::drop` documents it.
const WATCH_POLL_INTERVAL: std::time::Duration = std::time::Duration::from_millis(200);

impl ExtensionRuntime {
    /// Verify, load, and register the extension in `dir`.
    ///
    /// The signature gate runs first and unconditionally — see
    /// [`crate::runtime_verify`].
    pub fn register_loaded_from_dir(&mut self, dir: &Path) -> Result<(), RuntimeError> {
        self.verify_dir_signature(dir)?;
        let loaded = LoadedExtension::load_from_dir(self.engine(), dir)?;
        let id = loaded.id.clone();

        let mut new_map = self.loaded_map();
        new_map.insert(id.clone(), Arc::new(loaded));
        let new_registry = Self::rebuild_registry(&new_map)?;

        // Atomically swap in the new loaded map and its registry. Both stores
        // happen only after the rebuild succeeds, so a bad offered version
        // leaves the runtime exactly as it was.
        self.store_loaded(new_map, new_registry);

        self.emit(RuntimeEvent::ExtensionInstalled(id));
        Ok(())
    }

    /// Derive the capability registry from the loaded set.
    ///
    /// The registry holds nothing that is not already derivable from the
    /// loaded describes, so it is rebuilt wholesale rather than patched
    /// incrementally at each call site. That is what makes eviction correct by
    /// construction: a capability dropped from a describe, an extension that
    /// was removed, and a re-registered dir all fall out of the new map
    /// automatically instead of each needing its own fix. The previous
    /// clone-forward-then-append approach got all three wrong.
    pub(crate) fn rebuild_registry(
        loaded: &HashMap<ExtensionId, LoadedExtensionRef>,
    ) -> Result<CapabilityRegistry, RuntimeError> {
        let mut registry = CapabilityRegistry::new();
        for (id, ext) in loaded {
            for cap in &ext.describe.capabilities.offered {
                let version: semver::Version =
                    cap.version.parse().map_err(|e: semver::Error| {
                        RuntimeError::Wasmtime(anyhow::anyhow!("bad offered version: {e}"))
                    })?;
                registry.add_offering(OfferedBinding {
                    extension_id: id.as_str().to_string(),
                    cap_id: cap.id.clone(),
                    version,
                    kind: ext.kind,
                    // Source-dir registration has no export path; preserved
                    // from the original behaviour.
                    export_path: String::new(),
                });
            }
        }
        Ok(registry)
    }

    /// Spawns a watcher background thread. Events trigger reload of the
    /// affected extension's directory. The returned guard stops the thread
    /// when dropped.
    pub fn start_watcher(self: Arc<Self>) -> Result<WatcherGuard, RuntimeError> {
        let mut paths: Vec<PathBuf> = self.config().paths.all().into_iter().cloned().collect();
        // Also watch the parent of the extensions root so we receive events
        // for `<home>/extensions-state.json`. Best-effort: if the home dir
        // doesn't exist or has no parent we skip it — the kind dirs are still
        // watched, so hot reload keeps working; only the state-file signal is
        // lost, and `crate::watcher::watch` logs the paths it could not take.
        if let Some(home) = self.config().paths.home()
            && home.exists()
            && !paths.iter().any(|p| p == home)
        {
            paths.push(home.to_path_buf());
        }
        let (rx, watch_handle) = crate::watcher::watch(&paths)?;
        let (stop_tx, stop_rx) = std::sync::mpsc::channel::<()>();
        let this = self.clone();
        let join = std::thread::spawn(move || {
            // Own the watch_handle here — dropping it closes the fs watcher
            // and the tx side of the FsEvent channel when this thread exits.
            let _watch_handle = watch_handle;
            loop {
                // Check stop signal (Ok = message received, Disconnected = sender dropped).
                match stop_rx.try_recv() {
                    Ok(()) | Err(std::sync::mpsc::TryRecvError::Disconnected) => break,
                    Err(std::sync::mpsc::TryRecvError::Empty) => {}
                }
                match rx.recv_timeout(WATCH_POLL_INTERVAL) {
                    Ok(event) => {
                        if let Err(e) = this.handle_fs_event(&event) {
                            tracing::warn!(error = %e, "hot reload failed");
                        }
                    }
                    Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
                    Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
                }
            }
        });
        Ok(WatcherGuard::new(stop_tx, join))
    }

    fn handle_fs_event(&self, event: &crate::watcher::FsEvent) -> Result<(), RuntimeError> {
        use crate::watcher::FsEvent;
        let path = match event {
            FsEvent::Added(p) | FsEvent::Modified(p) | FsEvent::Removed(p) => p.clone(),
        };

        // Classify state file events first. The state file lives at
        // `<home>/extensions-state.json`, which is outside the per-kind
        // extension dirs, so `find_extension_dir` would return None — but
        // matching by filename is cheaper and unambiguous.
        if path.file_name().is_some_and(|n| n == STATE_FILENAME) {
            self.emit(RuntimeEvent::StateFileChanged);
            return Ok(());
        }

        let ext_dir = find_extension_dir(&path);
        match event {
            FsEvent::Removed(_) => {
                if let Some(dir) = ext_dir {
                    self.handle_removal(&dir);
                }
            }
            FsEvent::Added(_) | FsEvent::Modified(_) => {
                if let Some(dir) = ext_dir {
                    self.handle_added_or_modified(&dir)?;
                }
            }
        }
        Ok(())
    }

    /// Hot-reload entry point for a removed extension directory.
    ///
    /// `#[doc(hidden)] pub` rather than private so the watcher-path tests can
    /// exercise it directly — driving a real filesystem watcher from a test
    /// would be slow and racy. Not part of the supported API.
    #[doc(hidden)]
    pub fn handle_removal(&self, dir: &Path) {
        let current = self.loaded();
        let Some((id, _)) = current.iter().find(|(_, v)| v.source_dir == dir) else {
            return;
        };
        let id = id.clone();
        let mut new_map = (*current).clone();
        new_map.remove(&id);
        // Rebuilding drops the removed extension's offerings. Leaving them
        // advertised is the false positive that lets a preflight check pass a
        // policy the runtime then fails closed on.
        match Self::rebuild_registry(&new_map) {
            Ok(new_registry) => {
                self.store_loaded(new_map, new_registry);
                self.emit(RuntimeEvent::ExtensionRemoved(id));
            }
            // Unreachable in practice: an extension whose offered version does
            // not parse never enters `loaded` (both insert paths rebuild before
            // storing and bail on error), so a rebuild over a subset of
            // `loaded` cannot fail. Removal returns no error, so rather than
            // strand the runtime in a half-applied state we keep both the map
            // and the registry as they were and make the anomaly auditable.
            Err(e) => tracing::error!(
                extension_id = %id.as_str(),
                error = %e,
                "capability registry rebuild failed on removal; extension left loaded"
            ),
        }
    }

    /// Hot-reload entry point for an added or modified extension directory.
    ///
    /// `#[doc(hidden)] pub` rather than private so the watcher-path tests can
    /// exercise the signature gate directly — driving a real filesystem
    /// watcher from a test would be slow and racy. Not part of the supported
    /// API: callers should use [`ExtensionRuntime::register_loaded_from_dir`].
    #[doc(hidden)]
    pub fn handle_added_or_modified(&self, dir: &Path) -> Result<(), RuntimeError> {
        // The same gate as `register_loaded_from_dir`, no exceptions. Without
        // this, anyone able to write to a watched extension directory got code
        // execution with no signature check at all — and did not even need to
        // re-sign, since this path previously verified nothing.
        self.verify_dir_signature(dir)?;
        let loaded = LoadedExtension::load_from_dir(self.engine(), dir)?;
        let id = loaded.id.clone();
        let mut new_map = self.loaded_map();
        let prev_version = new_map
            .get(&id)
            .map(|e| e.describe.metadata.version.clone());
        new_map.insert(id.clone(), Arc::new(loaded));
        let new_registry = Self::rebuild_registry(&new_map)?;
        self.store_loaded(new_map, new_registry);
        let event = match prev_version {
            Some(prev) => RuntimeEvent::ExtensionUpdated {
                id,
                prev_version: prev,
            },
            None => RuntimeEvent::ExtensionInstalled(id),
        };
        self.emit(event);
        Ok(())
    }
}

/// Walk up from `p` until a directory containing `describe.json` is found.
///
/// Returns `None` at the filesystem root, so an event outside any extension
/// directory classifies as "not an extension" rather than looping.
fn find_extension_dir(p: &Path) -> Option<PathBuf> {
    let mut cur = p;
    loop {
        if cur
            .join(greentic_extension_sdk_contract::DESCRIBE_ENTRY_NAME)
            .exists()
        {
            return Some(cur.to_path_buf());
        }
        cur = cur.parent()?;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn find_extension_dir_walks_up_to_the_describe() {
        let tmp = tempfile::TempDir::new().unwrap();
        let ext = tmp.path().join("design").join("greentic.demo");
        std::fs::create_dir_all(ext.join("assets")).unwrap();
        std::fs::write(ext.join("describe.json"), b"{}").unwrap();

        let touched = ext.join("assets").join("icon.svg");
        assert_eq!(find_extension_dir(&touched), Some(ext));
    }

    #[test]
    fn find_extension_dir_gives_up_outside_an_extension() {
        let tmp = tempfile::TempDir::new().unwrap();
        let stray = tmp.path().join("not-an-extension.txt");
        std::fs::write(&stray, b"x").unwrap();
        // Walks to the filesystem root and stops there rather than looping.
        assert_eq!(find_extension_dir(&stray), None);
    }
}
