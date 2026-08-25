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
        let describe = self.verify_dir_signature(dir)?;
        let loaded = LoadedExtension::from_verified(self.engine(), dir, describe)?;
        let id = loaded.id.clone();

        self.mutate_loaded(|map| map.insert(id.clone(), Arc::new(loaded)))?;

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

    /// Drive one filesystem event through the classifier.
    ///
    /// `#[doc(hidden)] pub` for the same reason as [`Self::handle_removal`]:
    /// the classification itself — which events evict and which reload — is
    /// what the watcher tests need to pin, and driving a real filesystem
    /// watcher to reach it would be slow and racy. Not part of the supported
    /// API.
    #[doc(hidden)]
    pub fn handle_fs_event_for_test(
        &self,
        event: &crate::watcher::FsEvent,
    ) -> Result<(), RuntimeError> {
        self.handle_fs_event(event)
    }

    fn handle_fs_event(&self, event: &crate::watcher::FsEvent) -> Result<(), RuntimeError> {
        use crate::watcher::FsEvent;
        let path = match event {
            FsEvent::Added(p) | FsEvent::Modified(p) | FsEvent::Removed(p) => p.clone(),
        };

        // Classify state file events first. The state file lives at exactly
        // `<home>/extensions-state.json`. Match the whole path, not the
        // basename: the home dir is watched recursively, so a basename match
        // also fired for any pack shipping a file of that name — which both
        // emitted spurious `StateFileChanged` and, worse, returned early and
        // skipped re-verifying the pack that file belonged to.
        let is_state_file = self
            .config()
            .paths
            .home()
            .is_some_and(|home| path == home.join(STATE_FILENAME));
        if is_state_file {
            self.emit(RuntimeEvent::StateFileChanged);
            return Ok(());
        }

        match event {
            // Removal cannot go through `find_extension_dir`: that resolves a
            // path by looking for a `describe.json` beside it, and after an
            // uninstall there is no describe.json to find — so the event was
            // dropped and the extension stayed loaded and dispatchable until
            // the process restarted. Match against what is loaded instead, and
            // let the filesystem confirm which of those are actually gone.
            FsEvent::Removed(_) => self.evict_vanished_extensions(),
            FsEvent::Added(_) | FsEvent::Modified(_) => {
                let roots = self.config().paths.all();
                if let Some(dir) = find_extension_dir(&roots, &path) {
                    self.handle_added_or_modified(&dir)?;
                }
            }
        }
        Ok(())
    }

    /// Drop every loaded extension whose source directory no longer exists.
    ///
    /// Driven by the removal event rather than by the removed path: an
    /// uninstall can arrive as one event for the directory, or as a burst of
    /// per-file events in whatever order the debouncer coalesced them, and
    /// asking the filesystem which extensions are still there answers all of
    /// those the same way. Deleting a single asset out of a pack leaves its
    /// directory in place and so does not evict — the compiled component is
    /// already in memory, and a re-verify would only reject the pack without
    /// unloading it.
    fn evict_vanished_extensions(&self) {
        let vanished: Vec<ExtensionId> = self
            .loaded()
            .iter()
            .filter(|(_, ext)| !ext.source_dir.exists())
            .map(|(id, _)| id.clone())
            .collect();

        for id in vanished {
            self.evict(&id);
        }
    }

    /// Hot-reload entry point for a removed extension directory.
    ///
    /// `#[doc(hidden)] pub` rather than private so the watcher-path tests can
    /// exercise it directly — driving a real filesystem watcher from a test
    /// would be slow and racy. Not part of the supported API.
    #[doc(hidden)]
    pub fn handle_removal(&self, dir: &Path) {
        // The lookup happens inside the edit so it sees the same map the
        // removal is applied to; resolving the id outside would reintroduce the
        // race the lock exists to close.
        let found = self.mutate_loaded(|map| {
            let id = map
                .iter()
                .find(|(_, v)| v.source_dir == dir)
                .map(|(id, _)| id.clone())?;
            map.remove(&id);
            Some(id)
        });
        self.report_eviction(&dir.display().to_string(), found);
    }

    /// Drop one extension by id.
    fn evict(&self, id: &ExtensionId) {
        let found = self.mutate_loaded(|map| map.remove(id).map(|_| id.clone()));
        self.report_eviction(id.as_str(), found);
    }

    /// Announce an eviction, or record why one did not happen.
    ///
    /// Rebuilding drops the evicted extension's offerings. Leaving them
    /// advertised is the false positive that lets a preflight check pass a
    /// policy the runtime then fails closed on.
    fn report_eviction(&self, subject: &str, found: Result<Option<ExtensionId>, RuntimeError>) {
        match found {
            Ok(Some(id)) => self.emit(RuntimeEvent::ExtensionRemoved(id)),
            Ok(None) => {}
            // Unreachable in practice: an extension whose offered version does
            // not parse never enters `loaded` (every insert rebuilds before
            // storing and bails on error), so a rebuild over a subset of
            // `loaded` cannot fail. Eviction returns no error, so rather than
            // strand the runtime in a half-applied state we keep both the map
            // and the registry as they were and make the anomaly auditable.
            Err(e) => tracing::error!(
                subject,
                error = %e,
                "capability registry rebuild failed on eviction; extension left loaded"
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
        let describe = self.verify_dir_signature(dir)?;
        let loaded = LoadedExtension::from_verified(self.engine(), dir, describe)?;
        let id = loaded.id.clone();
        let prev_version = self.mutate_loaded(|map| {
            let prev = map.get(&id).map(|e| e.describe.metadata.version.clone());
            map.insert(id.clone(), Arc::new(loaded));
            prev
        })?;
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

/// Resolve a changed path to the extension directory that owns it, if any.
///
/// An extension directory is `<root>/<kind>/<name>` — exactly what
/// [`crate::discovery::scan_kind_dir`] enumerates. This walks up from `p` only
/// as far as that shape allows, and requires the result to sit directly under
/// a kind directory of one of the configured `roots`.
///
/// The bound is the point. Walking up to the filesystem root and taking the
/// first `describe.json` found meant any nested or dot-prefixed directory
/// anywhere under the watched tree — places discovery would never enumerate —
/// got a load attempt, and on success a first-use publisher pin under an
/// extension id of the writer's choosing. That pin is a write into the store
/// shared with `gtdx`, so it permanently blocks the genuine publisher for that
/// id. Making the two enumerators agree on what an extension directory is
/// closes that door.
fn find_extension_dir(roots: &[&PathBuf], p: &Path) -> Option<PathBuf> {
    let mut cur = p;
    loop {
        if cur
            .join(greentic_extension_sdk_contract::DESCRIBE_ENTRY_NAME)
            .exists()
            && is_extension_dir(roots, cur)
        {
            return Some(cur.to_path_buf());
        }
        cur = cur.parent()?;
    }
}

/// Is `dir` a `<root>/<kind>/<name>` directory for one of `roots`?
fn is_extension_dir(roots: &[&PathBuf], dir: &Path) -> bool {
    dir.parent()
        .and_then(Path::parent)
        .is_some_and(|root| roots.iter().any(|r| r.as_path() == root))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `<root>/<kind>/<name>/describe.json`, the shape `scan_kind_dir` finds.
    fn extension_at(root: &Path, kind: &str, name: &str) -> PathBuf {
        let dir = root.join(kind).join(name);
        std::fs::create_dir_all(dir.join("assets")).unwrap();
        std::fs::write(dir.join("describe.json"), b"{}").unwrap();
        dir
    }

    #[test]
    fn find_extension_dir_walks_up_to_the_owning_extension() {
        let tmp = tempfile::TempDir::new().unwrap();
        let root = tmp.path().to_path_buf();
        let ext = extension_at(&root, "design", "greentic.demo");

        let touched = ext.join("assets").join("icon.svg");
        assert_eq!(find_extension_dir(&[&root], &touched), Some(ext));
    }

    #[test]
    fn find_extension_dir_gives_up_outside_an_extension() {
        let tmp = tempfile::TempDir::new().unwrap();
        let root = tmp.path().to_path_buf();
        let stray = root.join("not-an-extension.txt");
        std::fs::write(&stray, b"x").unwrap();
        assert_eq!(find_extension_dir(&[&root], &stray), None);
    }

    #[test]
    fn a_describe_nested_below_the_expected_depth_is_not_an_extension() {
        // A `describe.json` smuggled one level deeper than `<root>/<kind>/<name>`
        // is somewhere `scan_kind_dir` would never look. Loading it would hand
        // whoever wrote it a first-use publisher pin under an id of their
        // choosing, in the trust store gtdx shares.
        let tmp = tempfile::TempDir::new().unwrap();
        let root = tmp.path().to_path_buf();
        let nested = root.join("design").join("greentic.demo").join("smuggled");
        std::fs::create_dir_all(&nested).unwrap();
        std::fs::write(nested.join("describe.json"), b"{}").unwrap();

        // It resolves to the real extension above it, never to the nested dir.
        assert_eq!(
            find_extension_dir(&[&root], &nested.join("describe.json")),
            None,
            "the enclosing directory has no describe.json of its own here"
        );
    }

    #[test]
    fn a_describe_outside_every_configured_root_is_ignored() {
        let tmp = tempfile::TempDir::new().unwrap();
        let root = tmp.path().join("configured");
        std::fs::create_dir_all(&root).unwrap();
        let elsewhere = extension_at(&tmp.path().join("elsewhere"), "design", "greentic.rogue");

        assert_eq!(
            find_extension_dir(&[&root], &elsewhere.join("describe.json")),
            None
        );
    }
}
