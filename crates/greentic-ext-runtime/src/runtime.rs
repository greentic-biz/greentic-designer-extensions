//! Runtime core: configuration, the loaded/registry stores, and the shared
//! plumbing every dispatch module builds on.
//!
//! The per-interface dispatch lives in sibling modules — [`crate::runtime_design`],
//! [`crate::runtime_deploy`], [`crate::runtime_bundle`], [`crate::runtime_roles`],
//! [`crate::runtime_dw_composer`] — and the load gate in
//! [`crate::runtime_verify`] / [`crate::runtime_registry`]. Each carries its own
//! `impl ExtensionRuntime` block so no single file outgrows the 500-line cap.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use arc_swap::ArcSwap;
use tokio::sync::broadcast;
use wasmtime::Engine;

use crate::capability::CapabilityRegistry;
use crate::discovery::DiscoveryPaths;
use crate::error::RuntimeError;
use crate::host_state::HostState;
use crate::loaded::{ExtensionId, HostOverrides, LoadedExtensionRef};

/// Capacity of the [`RuntimeEvent`] broadcast channel. Subscribers that fall
/// this far behind lose the oldest events (`broadcast` semantics) rather than
/// stalling the watcher thread.
const EVENT_CHANNEL_CAPACITY: usize = 64;

/// Configuration passed to [`ExtensionRuntime::new`].
///
/// Carries both the filesystem discovery paths and the [`HostOverrides`]
/// bundle that every dispatch call injects into the wasmtime `HostState`.
/// Callers that only need defaults (tests, simple CLI tools) can use
/// [`RuntimeConfig::from_paths`]; production callers that need real
/// i18n/secrets/HTTP backends chain [`RuntimeConfig::with_host_overrides`]
/// before handing the config to the runtime.
#[derive(Clone, Debug)]
pub struct RuntimeConfig {
    pub paths: DiscoveryPaths,
    /// Host-function overrides threaded into every WASM dispatch.
    /// Defaults to [`HostOverrides::default()`] (key-translator, empty
    /// secrets, no HTTP client, empty allow-list, no broker weak ref).
    /// Production callers replace this via [`RuntimeConfig::with_host_overrides`]
    /// or the ergonomic [`ExtensionRuntime::with_host_overrides`] builder.
    pub host_overrides: HostOverrides,
    /// Root of the TOFU publisher-key store (`<root>/trust/publishers.json`).
    /// `None` resolves as [`RuntimeConfig::resolve_trust_root`] describes —
    /// `$GREENTIC_HOME`, else `~/.greentic`. Tests point this at a temp dir.
    pub trust_root: Option<PathBuf>,
}

impl RuntimeConfig {
    /// Construct a config from discovery paths, using production-safe
    /// [`HostOverrides::default()`] (no HTTP client, empty secrets/i18n).
    #[must_use]
    pub fn from_paths(paths: DiscoveryPaths) -> Self {
        Self {
            paths,
            host_overrides: HostOverrides::default(),
            trust_root: None,
        }
    }

    /// Override the TOFU trust-store root. Returns `self` for builder-style
    /// chaining. Mainly for tests — production should leave this `None` so the
    /// root resolves to the same store `gtdx` writes.
    #[must_use]
    pub fn with_trust_root(mut self, root: PathBuf) -> Self {
        self.trust_root = Some(root);
        self
    }

    /// Resolve the root under which the TOFU publisher-key store lives.
    ///
    /// Resolution order, mirroring `gtdx` exactly:
    /// 1. an explicit [`RuntimeConfig::with_trust_root`] override,
    /// 2. `$GREENTIC_HOME` (gtdx's `--home` flag reads the same var),
    /// 3. `~/.greentic`.
    ///
    /// This deliberately does **not** derive from [`DiscoveryPaths`]. The
    /// trust store keys publisher keys by extension *id*; it has no
    /// relationship to where an extension directory happens to live.
    /// `DiscoveryPaths::home()` equals `~/.greentic` only by coincidence in
    /// the default layout and diverges under either `$GREENTIC_HOME` or the
    /// runner's `GREENTIC_EXTENSIONS_DIR` override — in which case a TOFU
    /// check would silently re-pin into a *different* store instead of
    /// matching the one gtdx populated.
    ///
    /// # Errors
    ///
    /// Returns [`RuntimeError::Io`] when no override or `$GREENTIC_HOME` is
    /// set and the platform reports no home directory. Failing closed is
    /// deliberate: any invented fallback root would pin somewhere gtdx never
    /// reads, which is the silent-mismatch failure this resolution exists to
    /// prevent.
    pub fn resolve_trust_root(&self) -> Result<PathBuf, RuntimeError> {
        if let Some(root) = &self.trust_root {
            return Ok(root.clone());
        }
        if let Some(home) = std::env::var_os("GREENTIC_HOME").filter(|v| !v.is_empty()) {
            return Ok(PathBuf::from(home));
        }
        directories::BaseDirs::new()
            .map(|d| d.home_dir().join(".greentic"))
            .ok_or_else(|| {
                RuntimeError::Io(std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    "cannot resolve the extension trust root: no home directory on this platform \
                     and GREENTIC_HOME is unset",
                ))
            })
    }

    /// Replace the [`HostOverrides`] bundle. Returns `self` for builder-style
    /// chaining:
    ///
    /// ```ignore
    /// let config = RuntimeConfig::from_paths(paths)
    ///     .with_host_overrides(production_overrides);
    /// ```
    #[must_use]
    pub fn with_host_overrides(mut self, overrides: HostOverrides) -> Self {
        self.host_overrides = overrides;
        self
    }
}

pub struct ExtensionRuntime {
    engine: Engine,
    config: RuntimeConfig,
    loaded: ArcSwap<HashMap<ExtensionId, LoadedExtensionRef>>,
    capability_registry: ArcSwap<CapabilityRegistry>,
    events: broadcast::Sender<RuntimeEvent>,
}

#[derive(Debug, Clone)]
pub enum RuntimeEvent {
    ExtensionInstalled(ExtensionId),
    ExtensionUpdated {
        id: ExtensionId,
        prev_version: String,
    },
    ExtensionRemoved(ExtensionId),
    CapabilityRegistryRebuilt,
    /// `~/.greentic/extensions-state.json` was created or modified. Subscribers
    /// should reload extension state and re-apply their enable/disable filter.
    StateFileChanged,
}

/// Returned by [`ExtensionRuntime::start_watcher`]. Dropping this stops the
/// watcher thread cleanly (within ~200 ms — the thread's stop-signal poll
/// interval).
pub struct WatcherGuard {
    stop_tx: Option<std::sync::mpsc::Sender<()>>,
    join: Option<std::thread::JoinHandle<()>>,
}

impl WatcherGuard {
    pub(crate) const fn new(
        stop_tx: std::sync::mpsc::Sender<()>,
        join: std::thread::JoinHandle<()>,
    ) -> Self {
        Self {
            stop_tx: Some(stop_tx),
            join: Some(join),
        }
    }
}

impl Drop for WatcherGuard {
    fn drop(&mut self) {
        // Drop stop_tx first to signal the thread, then join.
        drop(self.stop_tx.take());
        if let Some(handle) = self.join.take()
            && handle.join().is_err()
        {
            // The watcher thread panicked. Nothing to recover here — we are
            // already unwinding a guard — but a silent join failure would hide
            // that hot reload has been dead since that panic.
            tracing::error!("extension watcher thread panicked; hot reload was not running");
        }
    }
}

impl ExtensionRuntime {
    pub fn new(config: RuntimeConfig) -> Result<Self, RuntimeError> {
        let mut ec = wasmtime::Config::new();
        ec.wasm_component_model(true);

        // Persist compiled component artifacts to an on-disk cache. Without
        // this, every `Component::from_file` recompiles the WASM via Cranelift
        // on each boot — the dominant designer startup cost (~28 extensions,
        // several seconds). The default cache keys on the module bytes plus the
        // compiler settings, so a warm cache turns subsequent boots into a
        // deserialize instead of a recompile. Failing to initialise the cache
        // is non-fatal: we log and fall back to the no-cache (recompile) path.
        match wasmtime::Cache::from_file(None) {
            Ok(cache) => {
                ec.cache(Some(cache));
            }
            Err(e) => {
                tracing::warn!("wasmtime compilation cache disabled: {e}");
            }
        }

        let engine = Engine::new(&ec).map_err(|e| RuntimeError::Wasmtime(e.into()))?;
        let (tx, _) = broadcast::channel(EVENT_CHANNEL_CAPACITY);
        Ok(Self {
            engine,
            config,
            loaded: ArcSwap::from_pointee(HashMap::new()),
            capability_registry: ArcSwap::from_pointee(CapabilityRegistry::default()),
            events: tx,
        })
    }

    /// Construct a runtime with **no extensions loaded**, for downstream
    /// unit tests that need an `ExtensionRuntime` instance but do not
    /// exercise real WASM dispatch.
    ///
    /// Uses production-safe [`HostOverrides::default()`] and a throwaway
    /// discovery path that is never read (no extension is ever loaded).
    /// `list_tools` returns empty; `invoke_tool` returns a not-found error.
    ///
    /// This exists so crates like `greentic-aw-runtime` can build an
    /// `Arc<ExtensionRuntime>` in `--features test-mock` unit tests
    /// without a live extension directory. Do NOT use in production.
    ///
    /// # Errors
    ///
    /// Returns [`RuntimeError::Wasmtime`] only if the wasmtime `Engine` itself
    /// refuses to build — which needs no filesystem and no extension, so in
    /// practice this cannot fail. It still returns `Result` rather than
    /// panicking, so a host embedding it never takes the process down.
    pub fn for_test() -> Result<Self, RuntimeError> {
        let paths = DiscoveryPaths::new(PathBuf::from("/nonexistent/aw-ext-runtime-test"));
        Self::new(RuntimeConfig::from_paths(paths))
    }

    /// Replace the [`HostOverrides`] bundle used for every dispatch. Call
    /// once at startup with adapters that wrap real backends (i18n
    /// catalogue, secrets store, allow-listed HTTP client). Without this,
    /// host fns resolve through [`HostOverrides::default`] — fine for unit
    /// tests, not for production: i18n returns the key, secrets are empty,
    /// http allow-list is empty.
    ///
    /// This is a thin ergonomic wrapper over
    /// [`RuntimeConfig::with_host_overrides`] for callers that already hold
    /// an `ExtensionRuntime` instance:
    ///
    /// ```ignore
    /// let runtime = ExtensionRuntime::new(config)?
    ///     .with_host_overrides(production_overrides);
    /// ```
    #[must_use]
    pub fn with_host_overrides(mut self, host_overrides: HostOverrides) -> Self {
        self.config.host_overrides = host_overrides;
        self
    }

    #[must_use]
    pub fn engine(&self) -> &Engine {
        &self.engine
    }

    #[must_use]
    pub fn subscribe(&self) -> broadcast::Receiver<RuntimeEvent> {
        self.events.subscribe()
    }

    #[must_use]
    pub fn config(&self) -> &RuntimeConfig {
        &self.config
    }

    #[must_use]
    pub fn loaded(&self) -> Arc<HashMap<ExtensionId, LoadedExtensionRef>> {
        self.loaded.load_full()
    }

    #[must_use]
    pub fn capability_registry(&self) -> Arc<CapabilityRegistry> {
        self.capability_registry.load_full()
    }

    /// Snapshot of the loaded map, cloned for mutation by the registration
    /// paths in [`crate::runtime_registry`].
    pub(crate) fn loaded_map(&self) -> HashMap<ExtensionId, LoadedExtensionRef> {
        (**self.loaded.load()).clone()
    }

    /// Publish a new loaded map together with the registry derived from it.
    ///
    /// The pair is stored together on purpose: a registry that does not come
    /// from the map beside it is the stale-offering bug the wholesale rebuild
    /// exists to prevent. Callers must pass a registry produced by
    /// [`Self::rebuild_registry`] over exactly this map.
    pub(crate) fn store_loaded(
        &self,
        map: HashMap<ExtensionId, LoadedExtensionRef>,
        registry: CapabilityRegistry,
    ) {
        self.loaded.store(Arc::new(map));
        self.capability_registry.store(Arc::new(registry));
    }

    /// Broadcast a lifecycle event.
    ///
    /// A send error means only that nobody is subscribed, which is the normal
    /// state for a host that does not watch lifecycle events — so it is
    /// deliberately not surfaced to the caller.
    pub(crate) fn emit(&self, event: RuntimeEvent) {
        let _ = self.events.send(event);
    }

    /// Resolve a loaded extension into a fresh wasmtime store + instance.
    ///
    /// Every dispatch method starts here, so the "look the id up, build a
    /// store, map the failure" preamble exists once rather than once per
    /// exported function.
    pub(crate) fn dispatch_instance(
        &self,
        ext_id: &str,
    ) -> Result<(wasmtime::Store<HostState>, wasmtime::component::Instance), RuntimeError> {
        self.dispatch_instance_ctx(ext_id, &crate::host_ports::HostCallContext::default())
    }

    /// Like [`Self::dispatch_instance`] but threads a per-call
    /// [`crate::host_ports::HostCallContext`] into the host ports.
    pub(crate) fn dispatch_instance_ctx(
        &self,
        ext_id: &str,
        ctx: &crate::host_ports::HostCallContext,
    ) -> Result<(wasmtime::Store<HostState>, wasmtime::component::Instance), RuntimeError> {
        let loaded = self.lookup(ext_id)?;
        loaded
            .build_store_and_instance(&self.engine, self.config.host_overrides.clone(), ctx)
            .map_err(RuntimeError::Wasmtime)
    }

    /// Look a loaded extension up by id, or fail with
    /// [`RuntimeError::NotFound`].
    pub(crate) fn lookup(&self, ext_id: &str) -> Result<LoadedExtensionRef, RuntimeError> {
        self.loaded
            .load()
            .get(&ExtensionId(ext_id.to_string()))
            .cloned()
            .ok_or_else(|| RuntimeError::NotFound(ext_id.to_string()))
    }
}

/// Tracks extension ids already warned about a legacy WIT contract, so the
/// deprecation notice fires once per extension per process instead of on
/// every dispatch. Bounded by the number of distinct loaded extensions.
static LEGACY_CONTRACT_WARNED: std::sync::OnceLock<
    std::sync::Mutex<std::collections::HashSet<String>>,
> = std::sync::OnceLock::new();

/// Emit a one-shot deprecation warning if `version` is not the newest entry
/// in `newest`. No-op when the extension is already on the current contract
/// or has been warned before.
pub(crate) fn warn_if_legacy_contract(ext_id: &str, version: &str, newest: &str) {
    if version == newest {
        return;
    }
    let set = LEGACY_CONTRACT_WARNED
        .get_or_init(|| std::sync::Mutex::new(std::collections::HashSet::new()));
    // A poisoned lock here only means some other thread panicked mid-insert;
    // the set is a plain dedup cache, so recovering the inner value is correct
    // and strictly better than panicking on a logging path.
    let mut guard = match set.lock() {
        Ok(g) => g,
        Err(poisoned) => poisoned.into_inner(),
    };
    if guard.insert(ext_id.to_string()) {
        tracing::warn!(
            extension = %ext_id,
            contract = version,
            newest = newest,
            "extension uses a deprecated WIT contract version; rebuild against the current contract (unified 6-variant extension-error)"
        );
    }
}

/// Resolve `base@<ver>` against the instance's exports, trying `versions`
/// in order (newest first). Returns the export index, the full resolved
/// interface name, and the bare version string that matched — dispatch
/// code branches on the version to pick the matching typed signature.
///
/// The runtime bumped its WIT past `@0.1.0`, but several extensions in the
/// wild (http, llm-generic, webhook, platform-bootstrap, ...) were built
/// against the older package and have not been rebuilt. Returning the resolved
/// name (with version suffix) lets the nested `func_idx` lookup error name the
/// version that was actually picked.
pub(crate) fn resolve_iface_versions(
    store: &mut wasmtime::Store<HostState>,
    instance: &wasmtime::component::Instance,
    base: &str,
    versions: &[&'static str],
) -> Result<
    (
        wasmtime::component::ComponentExportIndex,
        String,
        &'static str,
    ),
    RuntimeError,
> {
    for &v in versions {
        let name = format!("{base}@{v}");
        if let Some(idx) = instance.get_export_index(&mut *store, None, &name) {
            return Ok((idx, name, v));
        }
    }
    Err(RuntimeError::Wasmtime(anyhow::anyhow!(
        "extension does not export interface '{base}' at any supported version ({versions:?})"
    )))
}

/// Resolve a named function inside an already-resolved interface.
pub(crate) fn resolve_func(
    store: &mut wasmtime::Store<HostState>,
    instance: &wasmtime::component::Instance,
    iface_idx: &wasmtime::component::ComponentExportIndex,
    iface_name: &str,
    func: &str,
) -> Result<wasmtime::component::ComponentExportIndex, RuntimeError> {
    instance
        .get_export_index(&mut *store, Some(iface_idx), func)
        .ok_or_else(|| {
            RuntimeError::Wasmtime(anyhow::anyhow!(
                "interface '{iface_name}' does not export '{func}'"
            ))
        })
}

/// Version tables per package family — newest first.
pub(crate) const DESIGN_VERSIONS: &[&str] = &["0.3.0", "0.2.0", "0.1.0"];
pub(crate) const DEPLOY_VERSIONS: &[&str] = &["0.2.0", "0.1.0"];
pub(crate) const BUNDLE_VERSIONS: &[&str] = &["0.2.0", "0.1.0"];
/// Guardrail interface only exists at 0.3.0 — single-version table.
pub(crate) const GUARDRAIL_VERSIONS: &[&str] = &["0.3.0"];

/// Resolve a `greentic:extension-design/<iface>` export against the design
/// version table, discarding the matched version.
///
/// `roles@0.2.0` deliberately uses its own dedicated lookup (see
/// `runtime_roles.rs`); it never existed at `@0.1.0`, so no fallback is
/// appropriate there.
pub(crate) fn resolve_design_iface(
    store: &mut wasmtime::Store<HostState>,
    instance: &wasmtime::component::Instance,
    base: &str,
) -> Result<(wasmtime::component::ComponentExportIndex, String), RuntimeError> {
    resolve_iface_versions(store, instance, base, DESIGN_VERSIONS).map(|(idx, name, _)| (idx, name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn for_test_constructs_runtime_with_no_extensions() {
        let runtime = ExtensionRuntime::for_test().expect("engine construction");
        assert!(
            runtime.loaded().is_empty(),
            "for_test runtime must have zero loaded extensions"
        );
    }

    #[test]
    fn lookup_reports_the_missing_id() {
        let runtime = ExtensionRuntime::for_test().expect("engine construction");
        match runtime.lookup("greentic.absent").err() {
            Some(RuntimeError::NotFound(id)) => assert_eq!(id, "greentic.absent"),
            other => panic!("expected NotFound, got {other:?}"),
        }
    }

    #[test]
    fn an_explicit_trust_root_wins_over_the_environment() {
        let tmp = tempfile::TempDir::new().unwrap();
        let config = RuntimeConfig::from_paths(DiscoveryPaths::new(PathBuf::from("/dev/null")))
            .with_trust_root(tmp.path().to_path_buf());
        assert_eq!(config.resolve_trust_root().unwrap(), tmp.path());
    }
}
