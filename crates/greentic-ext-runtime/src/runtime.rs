use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use arc_swap::ArcSwap;
use tokio::sync::broadcast;
use wasmtime::Engine;

use crate::capability::{CapabilityRegistry, OfferedBinding};
use crate::discovery::DiscoveryPaths;
use crate::error::RuntimeError;
use crate::loaded::{ExtensionId, HostOverrides, LoadedExtension, LoadedExtensionRef};

/// Filename of the persistent enable/disable state document, located at
/// `<home>/extensions-state.json`. Kept in sync with the constant of the
/// same name in `greentic-ext-state` (single source of truth lives there;
/// this duplicate exists only because the runtime intentionally does not
/// depend on `greentic-ext-state` to avoid a circular crate dependency).
const STATE_FILENAME: &str = "extensions-state.json";

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
    /// Resolution order, mirroring `gtdx` exactly
    /// (`greentic-extension-sdk-cli/src/main.rs:116-124`):
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
/// watcher thread cleanly (within ~200 ms).
pub struct WatcherGuard {
    stop_tx: Option<std::sync::mpsc::Sender<()>>,
    join: Option<std::thread::JoinHandle<()>>,
}

impl Drop for WatcherGuard {
    fn drop(&mut self) {
        // Drop stop_tx first to signal the thread, then join.
        drop(self.stop_tx.take());
        if let Some(handle) = self.join.take() {
            let _ = handle.join();
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
        let (tx, _) = broadcast::channel(64);
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
    #[must_use]
    pub fn for_test() -> Self {
        let paths =
            DiscoveryPaths::new(std::path::PathBuf::from("/nonexistent/aw-ext-runtime-test"));
        Self::new(RuntimeConfig::from_paths(paths))
            .expect("for_test ExtensionRuntime construction is infallible")
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

    /// Sister modules (`runtime_roles`) reach for the active overrides via
    /// this accessor instead of touching `config` directly.
    #[must_use]
    pub(crate) fn host_overrides(&self) -> &HostOverrides {
        &self.config.host_overrides
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

    pub fn register_loaded_from_dir(&mut self, dir: &std::path::Path) -> Result<(), RuntimeError> {
        self.verify_dir_signature(dir)?;
        let loaded = LoadedExtension::load_from_dir(&self.engine, dir)?;
        let id = loaded.id.clone();

        let mut new_map = (**self.loaded.load()).clone();
        new_map.insert(id.clone(), Arc::new(loaded));
        let new_registry = Self::rebuild_registry(&new_map)?;

        // Atomically swap in the new loaded map and its registry. Both stores
        // happen only after the rebuild succeeds, so a bad offered version
        // leaves the runtime exactly as it was.
        self.loaded.store(Arc::new(new_map));
        self.capability_registry.store(Arc::new(new_registry));

        let _ = self.events.send(RuntimeEvent::ExtensionInstalled(id));
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
    fn rebuild_registry(
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

    fn verify_dir_signature(&self, dir: &std::path::Path) -> Result<(), RuntimeError> {
        #[cfg(feature = "dev-allow-unsigned")]
        if std::env::var("GREENTIC_EXT_ALLOW_UNSIGNED").is_ok() {
            tracing::warn!(
                extension_dir = %dir.display(),
                "GREENTIC_EXT_ALLOW_UNSIGNED is set — signature verification skipped"
            );
            return Ok(());
        }
        let path = dir.join("describe.json");
        let raw = std::fs::read_to_string(&path)?;
        let describe: greentic_extension_sdk_contract::DescribeJson = serde_json::from_str(&raw)?;
        // Step 1 — integrity: the describe is unmodified since signing. This is
        // NOT authenticity: it proves nothing about *who* signed, because an
        // attacker can re-sign their own describe with their own key and pass
        // this check trivially. Steps 2 and 3 below supply authenticity.
        greentic_extension_sdk_contract::verify_describe_self_consistent(&describe).map_err(
            |e| RuntimeError::SignatureInvalid {
                extension_id: describe.metadata.id.clone(),
                reason: e.to_string(),
            },
        )?;

        let invalid = |reason: String| RuntimeError::SignatureInvalid {
            extension_id: describe.metadata.id.clone(),
            reason,
        };
        // `verify_describe_self_consistent` already rejects an unsigned
        // describe, so this is belt-and-braces rather than a live path — but
        // failing closed here keeps the invariant local and obvious.
        let key_b64 = describe
            .signature
            .as_ref()
            .map(|s| s.public_key.clone())
            .ok_or_else(|| invalid("unsigned describe cannot be anchored".to_string()))?;

        // Step 2 — integrity of the artifact itself, not just of the describe.
        // This must run BEFORE the anchor below, because pinning is a *write*
        // into a store shared with `gtdx`: a pin left behind by a load that
        // then fails would permanently block the genuine publisher for this id
        // in both tools, recoverable only by hand-editing publishers.json.
        //
        // `gtdx` orders it the same way, one level up — see
        // `sdk-registry/src/lifecycle.rs`, `verify_integrity` then
        // `verify_authenticity`. The ordering rule inside
        // `sdk-registry/src/verify.rs` covers only signature-then-anchor
        // because integrity is already done by the time it is called; reading
        // that rule without its caller is what put the pin ahead of the ledger
        // here.
        Self::verify_dir_manifest(dir, &describe)?;

        // Step 3 — anchor (TOFU): the key that signed this describe must be the
        // one pinned for this extension id on first load. Step 1 proved the
        // signature verifies against `key_b64`; pinning `key_b64` is therefore
        // what turns integrity into authenticity. There is no separate
        // `verify_describe_with_key` step: passing it a key read out of the
        // describe compares that key against itself, which is a tautology the
        // SDK's own doc warns against ("the key must come from a trust anchor
        // ... never from the artifact alone"). The anchor IS the trust anchor.
        let trust_root = self.config.resolve_trust_root()?;
        greentic_extension_sdk_registry::trust_store::TrustStore::new(&trust_root)
            .pin_or_verify(&describe.metadata.id, &key_b64)
            .map_err(|e| invalid(e.to_string()))?;

        let pub_prefix = key_b64.chars().take(16).collect::<String>();
        tracing::info!(
            extension_id = %describe.metadata.id,
            key_prefix = %pub_prefix,
            "extension signature verified and anchored to the pinned publisher key"
        );
        Ok(())
    }

    /// Verify the unpacked extension dir against its `manifest.json`
    /// (whole-archive integrity ledger).
    ///
    /// Audit P5 hardening — fail **closed**:
    /// - A missing `manifest.json` is now a hard error. Pre-ledger packs only
    ///   load under the `dev-allow-unsigned` escape (checked upstream in
    ///   [`verify_dir_signature`]); production refuses an unverifiable pack.
    /// - The describe's manifest binding (`manifestSha256`) must match the
    ///   on-disk `manifest.json`, so the (signed) describe transitively commits
    ///   to the ledger — an attacker cannot swap the manifest without breaking
    ///   the describe signature ([`verify_manifest_binding`]).
    /// - Every file the manifest lists must then hash to the recorded sha256.
    ///
    /// Closes audit P0 #2 (wasm + sibling archive entries unsigned) and the C2
    /// binding gap on the consumer side.
    fn verify_dir_manifest(
        dir: &std::path::Path,
        describe: &greentic_extension_sdk_contract::DescribeJson,
    ) -> Result<(), RuntimeError> {
        use sha2::{Digest, Sha256};
        let extension_id = describe.metadata.id.as_str();
        let manifest_path = dir.join(greentic_extension_sdk_contract::MANIFEST_ENTRY_NAME);
        if !manifest_path.exists() {
            return Err(RuntimeError::SignatureInvalid {
                extension_id: extension_id.to_string(),
                reason: "manifest.json absent — refusing to load an extension without a \
                         whole-archive integrity ledger (set GREENTIC_EXT_ALLOW_UNSIGNED \
                         with the dev-allow-unsigned build for local dev)"
                    .to_string(),
            });
        }
        let raw = std::fs::read(&manifest_path)?;
        // Binding: the signed describe commits to exactly this manifest, so the
        // signature transitively covers the ledger (audit C2). Rejects both a
        // swapped manifest and an unbound (legacy) describe carrying a manifest.
        greentic_extension_sdk_contract::verify_manifest_binding(describe, &raw).map_err(|e| {
            RuntimeError::SignatureInvalid {
                extension_id: extension_id.to_string(),
                reason: format!("manifest binding: {e}"),
            }
        })?;
        let manifest: greentic_extension_sdk_contract::Manifest = serde_json::from_slice(&raw)
            .map_err(|e| RuntimeError::SignatureInvalid {
                extension_id: extension_id.to_string(),
                reason: format!("manifest.json parse: {e}"),
            })?;
        if manifest.schema != greentic_extension_sdk_contract::MANIFEST_SCHEMA_V1 {
            return Err(RuntimeError::SignatureInvalid {
                extension_id: extension_id.to_string(),
                reason: format!("manifest schema unsupported: {}", manifest.schema),
            });
        }
        for entry in &manifest.entries {
            let path = dir.join(&entry.path);
            if !path.exists() {
                return Err(RuntimeError::SignatureInvalid {
                    extension_id: extension_id.to_string(),
                    reason: format!("manifest lists missing file: {}", entry.path),
                });
            }
            let bytes = std::fs::read(&path)?;
            let computed = format!("{:x}", Sha256::digest(&bytes));
            if computed != entry.sha256 {
                return Err(RuntimeError::SignatureInvalid {
                    extension_id: extension_id.to_string(),
                    reason: format!(
                        "manifest sha256 mismatch for {}: expected {} got {}",
                        entry.path, entry.sha256, computed
                    ),
                });
            }
        }
        tracing::info!(
            extension_id = %extension_id,
            entries = manifest.entries.len(),
            "whole-archive manifest verified"
        );
        Ok(())
    }

    /// Spawns a watcher background thread. Events trigger reload of the
    /// affected extension's directory. Returns a stop sender — dropping or
    /// sending on it signals the watcher thread to exit. Also returns the
    /// thread `JoinHandle` for callers that want to wait for clean shutdown.
    pub fn start_watcher(self: Arc<Self>) -> Result<WatcherGuard, RuntimeError> {
        let mut paths: Vec<std::path::PathBuf> =
            self.config.paths.all().into_iter().cloned().collect();
        // Also watch the parent of the extensions root so we receive events
        // for `<home>/extensions-state.json`. Best-effort: if the home dir
        // doesn't exist or has no parent we silently skip — the kind dirs
        // are still watched.
        if let Some(home) = self.config.paths.home()
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
                match rx.recv_timeout(std::time::Duration::from_millis(200)) {
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
        Ok(WatcherGuard {
            stop_tx: Some(stop_tx),
            join: Some(join),
        })
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
            let _ = self.events.send(RuntimeEvent::StateFileChanged);
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
    pub fn handle_removal(&self, dir: &std::path::Path) {
        let current = self.loaded.load();
        let Some((id, _)) = current.iter().find(|(_, v)| v.source_dir == dir) else {
            return;
        };
        let id = id.clone();
        let mut new_map = (**current).clone();
        new_map.remove(&id);
        // Rebuilding drops the removed extension's offerings. Leaving them
        // advertised is the false positive that lets a preflight check pass a
        // policy the runtime then fails closed on.
        match Self::rebuild_registry(&new_map) {
            Ok(new_registry) => {
                self.loaded.store(Arc::new(new_map));
                self.capability_registry.store(Arc::new(new_registry));
                let _ = self.events.send(RuntimeEvent::ExtensionRemoved(id));
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
    pub fn handle_added_or_modified(&self, dir: &std::path::Path) -> Result<(), RuntimeError> {
        // The same gate as `register_loaded_from_dir`, no exceptions. Without
        // this, anyone able to write to a watched extension directory got code
        // execution with no signature check at all — and did not even need to
        // re-sign, since this path previously verified nothing.
        self.verify_dir_signature(dir)?;
        let loaded = crate::loaded::LoadedExtension::load_from_dir(&self.engine, dir)?;
        let id = loaded.id.clone();
        let mut new_map = (**self.loaded.load()).clone();
        let prev_version = new_map
            .get(&id)
            .map(|e| e.describe.metadata.version.clone());
        new_map.insert(id.clone(), Arc::new(loaded));
        let new_registry = Self::rebuild_registry(&new_map)?;
        self.loaded.store(Arc::new(new_map));
        self.capability_registry.store(Arc::new(new_registry));
        let event = match prev_version {
            Some(prev) => RuntimeEvent::ExtensionUpdated {
                id,
                prev_version: prev,
            },
            None => RuntimeEvent::ExtensionInstalled(id),
        };
        let _ = self.events.send(event);
        Ok(())
    }
}

impl ExtensionRuntime {
    /// Invoke a named tool on a loaded extension.
    ///
    /// Builds a fresh wasmtime Store + Instance, calls
    /// `greentic:extension-design/tools::invoke-tool` (resolved
    /// newest-first across 0.3.0/0.2.0/0.1.0; WIT errors surface as
    /// `RuntimeError::Extension`), and returns the JSON result string.
    pub fn invoke_tool(
        &self,
        ext_id: &str,
        tool_name: &str,
        args_json: &str,
    ) -> Result<String, RuntimeError> {
        self.invoke_tool_ctx(
            ext_id,
            tool_name,
            args_json,
            &crate::host_ports::HostCallContext::default(),
        )
    }

    /// Like [`Self::invoke_tool`] but threads a per-call
    /// [`crate::host_ports::HostCallContext`] (e.g. the caller's tenant slug)
    /// into the host ports for this dispatch. Multi-tenant hosts (the
    /// designer) use this so the LLM port can resolve roles per-tenant.
    pub fn invoke_tool_ctx(
        &self,
        ext_id: &str,
        tool_name: &str,
        args_json: &str,
        ctx: &crate::host_ports::HostCallContext,
    ) -> Result<String, RuntimeError> {
        let loaded = self
            .loaded
            .load()
            .get(&crate::loaded::ExtensionId(ext_id.to_string()))
            .cloned()
            .ok_or_else(|| RuntimeError::NotFound(ext_id.to_string()))?;

        let (mut store, instance) = loaded
            .build_store_and_instance(&self.engine, self.config.host_overrides.clone(), ctx)
            .map_err(RuntimeError::Wasmtime)?;

        // Resolve the nested export: first the interface instance, then the function.
        // This is the wasmtime 43 pattern: get_export_index(store, parent, name).
        // The interface is resolved newest-first across the design version table;
        // the matched version selects which `extension-error` ABI to deserialize
        // (6-variant base at 0.3.0, 4-variant base at 0.2.0/0.1.0). The
        // invoke-tool signature is identical across versions — only the error
        // variant set differs.
        let (iface_idx, iface_name, version) = resolve_iface_versions(
            &mut store,
            &instance,
            "greentic:extension-design/tools",
            DESIGN_VERSIONS,
        )?;
        warn_if_legacy_contract(ext_id, version, DESIGN_VERSIONS[0]);
        let func_idx = instance
            .get_export_index(&mut store, Some(&iface_idx), "invoke-tool")
            .ok_or_else(|| {
                RuntimeError::Wasmtime(anyhow::anyhow!(
                    "interface '{iface_name}' does not export 'invoke-tool'"
                ))
            })?;

        let call_args = (tool_name.to_string(), args_json.to_string());
        // post_return is deprecated/no-op in wasmtime 43 — not called.
        let mapped: Result<String, crate::types::HostExtensionError> = if version == "0.3.0" {
            use crate::host_bindings::design_v03::greentic::extension_base0_2_0::types::ExtensionError as E2;
            let func = instance
                .get_typed_func::<(String, String), (Result<String, E2>,)>(&mut store, &func_idx)
                .map_err(|e| RuntimeError::Wasmtime(e.into()))?;
            let (r,) = func
                .call(&mut store, call_args)
                .map_err(|e| RuntimeError::Wasmtime(e.into()))?;
            r.map_err(crate::ext_error::from_design_v03)
        } else {
            use crate::host_bindings::greentic::extension_base0_1_0::types::ExtensionError as E1;
            let func = instance
                .get_typed_func::<(String, String), (Result<String, E1>,)>(&mut store, &func_idx)
                .map_err(|e| RuntimeError::Wasmtime(e.into()))?;
            let (r,) = func
                .call(&mut store, call_args)
                .map_err(|e| RuntimeError::Wasmtime(e.into()))?;
            r.map_err(crate::ext_error::from_design_v01)
        };

        mapped.map_err(RuntimeError::Extension)
    }
}

impl ExtensionRuntime {
    /// Evaluate a guardrail extension against `input_json` and return the
    /// verdict as a JSON string.
    ///
    /// Loads the extension identified by `ext_id`, resolves the
    /// `greentic:extension-design/guardrail@0.3.0` interface, calls the
    /// `evaluate` export, and maps the returned `verdict` variant to
    /// [`crate::GuardrailVerdictWire`] serialised as JSON.
    ///
    /// `input_json` must be a JSON object with fields matching the WIT
    /// `guardrail-input` record:
    ///
    /// ```json
    /// {
    ///   "direction": "inbound",
    ///   "content": "…",
    ///   "agent_id": "…",
    ///   "session_id": "…",
    ///   "tenant_id": "…",
    ///   "env_id": "…",
    ///   "context": null
    /// }
    /// ```
    ///
    /// Returns the verdict as JSON, e.g. `{"kind":"accept"}` or
    /// `{"kind":"deny","code":"…","message":"…","details":null}`.
    ///
    /// # Errors
    ///
    /// - [`RuntimeError::NotFound`] when no extension is loaded at `ext_id`.
    /// - [`RuntimeError::Wasmtime`] when store/instance construction fails,
    ///   the interface is not exported, the typed-func call fails, or
    ///   `input_json` cannot be deserialised.
    pub fn evaluate_guardrail(
        &self,
        ext_id: &str,
        input_json: &str,
    ) -> Result<String, RuntimeError> {
        let loaded = self
            .loaded
            .load()
            .get(&crate::loaded::ExtensionId(ext_id.to_string()))
            .cloned()
            .ok_or_else(|| RuntimeError::NotFound(ext_id.to_string()))?;

        let (mut store, instance) = loaded
            .build_store_and_instance(
                &self.engine,
                self.config.host_overrides.clone(),
                &crate::host_ports::HostCallContext::default(),
            )
            .map_err(RuntimeError::Wasmtime)?;

        // Guardrail interface only exists at 0.3.0 — a single-version table.
        let (iface_idx, iface_name, _version) = resolve_iface_versions(
            &mut store,
            &instance,
            "greentic:extension-design/guardrail",
            GUARDRAIL_VERSIONS,
        )?;

        let func_idx = instance
            .get_export_index(&mut store, Some(&iface_idx), "evaluate")
            .ok_or_else(|| {
                RuntimeError::Wasmtime(anyhow::anyhow!(
                    "interface '{iface_name}' does not export 'evaluate'"
                ))
            })?;

        let wire =
            crate::guardrail_map::call_evaluate(&mut store, &instance, &func_idx, input_json)?;

        serde_json::to_string(&wire).map_err(|e| RuntimeError::Wasmtime(e.into()))
    }
}

impl ExtensionRuntime {
    /// Validate extension-specific content against the extension's schema.
    ///
    /// Calls `greentic:extension-design/validation::validate-content`
    /// (resolved against `@0.2.0` first, then `@0.1.0`).
    /// `content_type` is an extension-defined label (e.g. `"AdaptiveCard"`
    /// for the adaptive-cards extension); `content_json` is the content
    /// payload as a JSON string.
    ///
    /// Returns a [`types::ValidateResult`] with a `valid` flag and a list of
    /// diagnostics (error/warning/info/hint severities). Extensions that
    /// don't export this interface surface a `Wasmtime` error — callers
    /// that want graceful degradation should treat "interface not exported"
    /// as "no validation available" rather than a hard failure.
    pub fn validate_content(
        &self,
        ext_id: &str,
        content_type: &str,
        content_json: &str,
    ) -> Result<crate::types::ValidateResult, RuntimeError> {
        use crate::host_bindings::exports::greentic::extension_design0_2_0::validation::{
            Diagnostic as WitDiagnostic, ValidateResult as WitValidateResult,
        };
        use crate::host_bindings::greentic::extension_base0_1_0::types::Severity as WitSeverity;

        let loaded = self
            .loaded
            .load()
            .get(&crate::loaded::ExtensionId(ext_id.to_string()))
            .cloned()
            .ok_or_else(|| RuntimeError::NotFound(ext_id.to_string()))?;

        let (mut store, instance) = loaded
            .build_store_and_instance(
                &self.engine,
                self.config.host_overrides.clone(),
                &crate::host_ports::HostCallContext::default(),
            )
            .map_err(RuntimeError::Wasmtime)?;

        let (iface_idx, iface_name) = resolve_design_iface(
            &mut store,
            &instance,
            "greentic:extension-design/validation",
        )?;
        let func_idx = instance
            .get_export_index(&mut store, Some(&iface_idx), "validate-content")
            .ok_or_else(|| {
                RuntimeError::Wasmtime(anyhow::anyhow!(
                    "interface '{iface_name}' does not export 'validate-content'"
                ))
            })?;

        let func = instance
            .get_typed_func::<(String, String), (WitValidateResult,)>(&mut store, &func_idx)
            .map_err(|e| RuntimeError::Wasmtime(e.into()))?;

        let (result,) = func
            .call(
                &mut store,
                (content_type.to_string(), content_json.to_string()),
            )
            .map_err(|e| RuntimeError::Wasmtime(e.into()))?;

        let diagnostics = result
            .diagnostics
            .into_iter()
            .map(|d: WitDiagnostic| crate::types::Diagnostic {
                severity: match d.severity {
                    WitSeverity::Error => crate::types::Severity::Error,
                    WitSeverity::Warning => crate::types::Severity::Warning,
                    WitSeverity::Info => crate::types::Severity::Info,
                    WitSeverity::Hint => crate::types::Severity::Hint,
                },
                code: d.code,
                message: d.message,
                path: d.path,
            })
            .collect();

        Ok(crate::types::ValidateResult {
            valid: result.valid,
            diagnostics,
        })
    }
}

/// Map a v2 describe `Tool` contribution to a host-side [`crate::types::ToolDefinition`].
///
/// Every field comes from the declarative `describe.json` tool entry — for a
/// v2 extension this is the ONLY source, since [`ExtensionRuntime::list_tools`]
/// never calls the wasm's `list-tools` export for that contract. A field the
/// describe omits is therefore not "filled in from WIT later"; it is simply
/// absent for the tool's whole life.
///
/// Omissions are not errors, so that a partially-declared tool is still
/// offered rather than disappearing — but each one degrades the tool, and the
/// symptom is otherwise silence: an LLM that cannot infer arguments, or a
/// planner with no side-effect signal, and nothing anywhere saying why. This
/// mapper is a pure function and reports nothing; the omissions are reported
/// once per extension when the artifact is loaded, by
/// `tool_metadata_report::report_tool_metadata_gaps`. Reporting from here
/// instead would repeat the whole burst on every `list_tools` call, which is a
/// per-request path.
#[must_use]
pub fn contribution_tool_to_definition(
    t: &greentic_extension_sdk_contract::describe::contributions::Tool,
) -> crate::types::ToolDefinition {
    crate::types::ToolDefinition {
        name: t.name.clone(),
        description: t.description.clone().unwrap_or_default(),
        input_schema_json: t.input_schema.clone().unwrap_or_default(),
        output_schema_json: t.output_schema.clone(),
        capabilities: t.capabilities.clone(),
        agentic_worker_metadata: t.agentic_worker_metadata.clone(),
        secret_requirements: t.secret_requirements.clone(),
    }
}

impl ExtensionRuntime {
    /// List all tools exposed by a loaded design extension.
    ///
    /// Calls `greentic:extension-design/tools::list-tools` (resolved
    /// against `@0.2.0` first, then `@0.1.0`) for v1-contract
    /// extensions. **v2 contract** (`apiVersion == "greentic.ai/v2"`)
    /// reads the tools from `describe.contributions.tools[]` — the
    /// runtime WIT no longer exports `list-tools` in that contract.
    /// Everything a v2 tool exposes — description, schemas, capabilities,
    /// agentic-worker metadata — must therefore be declared in
    /// `describe.json`; the WIT export is never consulted on that path.
    pub fn list_tools(
        &self,
        ext_id: &str,
    ) -> Result<Vec<crate::types::ToolDefinition>, RuntimeError> {
        use crate::host_bindings::exports::greentic::extension_design0_2_0::tools::ToolDefinition as WitToolDef;

        let loaded = self
            .loaded
            .load()
            .get(&crate::loaded::ExtensionId(ext_id.to_string()))
            .cloned()
            .ok_or_else(|| RuntimeError::NotFound(ext_id.to_string()))?;

        // v2 declarative path: tools live in describe.json, not in WIT.
        if loaded.describe.api_version == "greentic.ai/v2" {
            return Ok(loaded
                .describe
                .contributions
                .tools
                .iter()
                .map(contribution_tool_to_definition)
                .collect());
        }

        // v1 WIT-call path.
        let (mut store, instance) = loaded
            .build_store_and_instance(
                &self.engine,
                self.config.host_overrides.clone(),
                &crate::host_ports::HostCallContext::default(),
            )
            .map_err(RuntimeError::Wasmtime)?;

        let (iface_idx, iface_name) =
            resolve_design_iface(&mut store, &instance, "greentic:extension-design/tools")?;
        let func_idx = instance
            .get_export_index(&mut store, Some(&iface_idx), "list-tools")
            .ok_or_else(|| {
                RuntimeError::Wasmtime(anyhow::anyhow!(
                    "interface '{iface_name}' does not export 'list-tools'"
                ))
            })?;

        let func = instance
            .get_typed_func::<(), (Vec<WitToolDef>,)>(&mut store, &func_idx)
            .map_err(|e| RuntimeError::Wasmtime(e.into()))?;

        let (defs,) = func
            .call(&mut store, ())
            .map_err(|e| RuntimeError::Wasmtime(e.into()))?;

        Ok(defs
            .into_iter()
            .map(|d| crate::types::ToolDefinition {
                name: d.name,
                description: d.description,
                input_schema_json: d.input_schema_json,
                output_schema_json: d.output_schema_json,
                capabilities: d.capabilities,
                agentic_worker_metadata: d.agentic_worker_metadata,
                secret_requirements: Vec::new(),
            })
            .collect())
    }
}

impl ExtensionRuntime {
    /// Retrieve system prompt fragments from a loaded design extension.
    ///
    /// Calls `greentic:extension-design/prompting::system-prompt-fragments`
    /// (resolved against `@0.2.0` first, then `@0.1.0`).
    pub fn prompt_fragments(
        &self,
        ext_id: &str,
    ) -> Result<Vec<crate::types::PromptFragment>, RuntimeError> {
        use crate::host_bindings::exports::greentic::extension_design0_2_0::prompting::PromptFragment as WitFrag;

        let loaded = self
            .loaded
            .load()
            .get(&crate::loaded::ExtensionId(ext_id.to_string()))
            .cloned()
            .ok_or_else(|| RuntimeError::NotFound(ext_id.to_string()))?;

        let (mut store, instance) = loaded
            .build_store_and_instance(
                &self.engine,
                self.config.host_overrides.clone(),
                &crate::host_ports::HostCallContext::default(),
            )
            .map_err(RuntimeError::Wasmtime)?;

        let (iface_idx, iface_name) =
            resolve_design_iface(&mut store, &instance, "greentic:extension-design/prompting")?;
        let func_idx = instance
            .get_export_index(&mut store, Some(&iface_idx), "system-prompt-fragments")
            .ok_or_else(|| {
                RuntimeError::Wasmtime(anyhow::anyhow!(
                    "interface '{iface_name}' does not export 'system-prompt-fragments'"
                ))
            })?;

        let func = instance
            .get_typed_func::<(), (Vec<WitFrag>,)>(&mut store, &func_idx)
            .map_err(|e| RuntimeError::Wasmtime(e.into()))?;

        let (frags,) = func
            .call(&mut store, ())
            .map_err(|e| RuntimeError::Wasmtime(e.into()))?;

        Ok(frags
            .into_iter()
            .map(|f| crate::types::PromptFragment {
                section: f.section,
                content_markdown: f.content_markdown,
                priority: f.priority,
            })
            .collect())
    }
}

impl ExtensionRuntime {
    /// List knowledge entries, optionally filtered by category.
    ///
    /// Calls `greentic:extension-design/knowledge::list-entries`
    /// (resolved against `@0.2.0` first, then `@0.1.0`).
    pub fn knowledge_list(
        &self,
        ext_id: &str,
        category_filter: Option<&str>,
    ) -> Result<Vec<crate::types::KnowledgeEntrySummary>, RuntimeError> {
        use crate::host_bindings::exports::greentic::extension_design0_2_0::knowledge::EntrySummary as WitSummary;

        let loaded = self
            .loaded
            .load()
            .get(&crate::loaded::ExtensionId(ext_id.to_string()))
            .cloned()
            .ok_or_else(|| RuntimeError::NotFound(ext_id.to_string()))?;

        let (mut store, instance) = loaded
            .build_store_and_instance(
                &self.engine,
                self.config.host_overrides.clone(),
                &crate::host_ports::HostCallContext::default(),
            )
            .map_err(RuntimeError::Wasmtime)?;

        let (iface_idx, iface_name) =
            resolve_design_iface(&mut store, &instance, "greentic:extension-design/knowledge")?;
        let func_idx = instance
            .get_export_index(&mut store, Some(&iface_idx), "list-entries")
            .ok_or_else(|| {
                RuntimeError::Wasmtime(anyhow::anyhow!(
                    "interface '{iface_name}' does not export 'list-entries'"
                ))
            })?;

        let func = instance
            .get_typed_func::<(Option<String>,), (Vec<WitSummary>,)>(&mut store, &func_idx)
            .map_err(|e| RuntimeError::Wasmtime(e.into()))?;

        let (entries,) = func
            .call(&mut store, (category_filter.map(String::from),))
            .map_err(|e| RuntimeError::Wasmtime(e.into()))?;

        Ok(entries.into_iter().map(wit_summary_to_host).collect())
    }

    /// Retrieve a single knowledge entry by ID.
    ///
    /// Calls `greentic:extension-design/knowledge::get-entry`, resolving the
    /// interface newest-first across `@0.3.0`/`@0.2.0`/`@0.1.0`. The matched
    /// version selects which `extension-error` ABI to deserialize (6-variant
    /// base at `@0.3.0`, 4-variant base at `@0.2.0`/`@0.1.0`); WIT errors
    /// surface as `RuntimeError::Extension`.
    pub fn knowledge_get(
        &self,
        ext_id: &str,
        entry_id: &str,
    ) -> Result<crate::types::KnowledgeEntry, RuntimeError> {
        let loaded = self
            .loaded
            .load()
            .get(&crate::loaded::ExtensionId(ext_id.to_string()))
            .cloned()
            .ok_or_else(|| RuntimeError::NotFound(ext_id.to_string()))?;

        let (mut store, instance) = loaded
            .build_store_and_instance(
                &self.engine,
                self.config.host_overrides.clone(),
                &crate::host_ports::HostCallContext::default(),
            )
            .map_err(RuntimeError::Wasmtime)?;

        let (iface_idx, iface_name, version) = resolve_iface_versions(
            &mut store,
            &instance,
            "greentic:extension-design/knowledge",
            DESIGN_VERSIONS,
        )?;
        let func_idx = instance
            .get_export_index(&mut store, Some(&iface_idx), "get-entry")
            .ok_or_else(|| {
                RuntimeError::Wasmtime(anyhow::anyhow!(
                    "interface '{iface_name}' does not export 'get-entry'"
                ))
            })?;

        let call_args = (entry_id.to_string(),);
        let mapped: Result<crate::types::KnowledgeEntry, crate::types::HostExtensionError> =
            if version == "0.3.0" {
                use crate::host_bindings::design_v03::exports::greentic::extension_design0_3_0::knowledge::{
                    Entry as WitEntry, ExtensionError as E2,
                };
                let func = instance
                    .get_typed_func::<(String,), (Result<WitEntry, E2>,)>(&mut store, &func_idx)
                    .map_err(|e| RuntimeError::Wasmtime(e.into()))?;
                let (r,) = func
                    .call(&mut store, call_args)
                    .map_err(|e| RuntimeError::Wasmtime(e.into()))?;
                r.map(|e| crate::types::KnowledgeEntry {
                    id: e.id,
                    title: e.title,
                    category: e.category,
                    tags: e.tags,
                    content_json: e.content_json,
                })
                .map_err(crate::ext_error::from_design_v03)
            } else {
                use crate::host_bindings::exports::greentic::extension_design0_2_0::knowledge::{
                    Entry as WitEntry, ExtensionError as E1,
                };
                let func = instance
                    .get_typed_func::<(String,), (Result<WitEntry, E1>,)>(&mut store, &func_idx)
                    .map_err(|e| RuntimeError::Wasmtime(e.into()))?;
                let (r,) = func
                    .call(&mut store, call_args)
                    .map_err(|e| RuntimeError::Wasmtime(e.into()))?;
                r.map(|e| crate::types::KnowledgeEntry {
                    id: e.id,
                    title: e.title,
                    category: e.category,
                    tags: e.tags,
                    content_json: e.content_json,
                })
                .map_err(crate::ext_error::from_design_v01)
            };

        mapped.map_err(RuntimeError::Extension)
    }

    /// Suggest knowledge entries matching a query.
    ///
    /// Calls `greentic:extension-design/knowledge::suggest-entries`
    /// (resolved against `@0.2.0` first, then `@0.1.0`).
    pub fn knowledge_suggest(
        &self,
        ext_id: &str,
        query: &str,
        limit: u32,
    ) -> Result<Vec<crate::types::KnowledgeEntrySummary>, RuntimeError> {
        use crate::host_bindings::exports::greentic::extension_design0_2_0::knowledge::EntrySummary as WitSummary;

        let loaded = self
            .loaded
            .load()
            .get(&crate::loaded::ExtensionId(ext_id.to_string()))
            .cloned()
            .ok_or_else(|| RuntimeError::NotFound(ext_id.to_string()))?;

        let (mut store, instance) = loaded
            .build_store_and_instance(
                &self.engine,
                self.config.host_overrides.clone(),
                &crate::host_ports::HostCallContext::default(),
            )
            .map_err(RuntimeError::Wasmtime)?;

        let (iface_idx, iface_name) =
            resolve_design_iface(&mut store, &instance, "greentic:extension-design/knowledge")?;
        let func_idx = instance
            .get_export_index(&mut store, Some(&iface_idx), "suggest-entries")
            .ok_or_else(|| {
                RuntimeError::Wasmtime(anyhow::anyhow!(
                    "interface '{iface_name}' does not export 'suggest-entries'"
                ))
            })?;

        let func = instance
            .get_typed_func::<(String, u32), (Vec<WitSummary>,)>(&mut store, &func_idx)
            .map_err(|e| RuntimeError::Wasmtime(e.into()))?;

        let (entries,) = func
            .call(&mut store, (query.to_string(), limit))
            .map_err(|e| RuntimeError::Wasmtime(e.into()))?;

        Ok(entries.into_iter().map(wit_summary_to_host).collect())
    }
}

/// Convert a bindgen `EntrySummary` to the host-side type.
fn wit_summary_to_host(
    s: crate::host_bindings::exports::greentic::extension_design0_2_0::knowledge::EntrySummary,
) -> crate::types::KnowledgeEntrySummary {
    crate::types::KnowledgeEntrySummary {
        id: s.id,
        title: s.title,
        category: s.category,
        tags: s.tags,
    }
}

impl ExtensionRuntime {
    /// Ask a deploy extension to validate a credentials JSON payload for the
    /// given target. Returns diagnostics; empty slice means valid.
    pub fn validate_credentials(
        &self,
        ext_id: &str,
        target_id: &str,
        credentials_json: &str,
    ) -> Result<Vec<crate::types::Diagnostic>, RuntimeError> {
        use crate::host_bindings::deploy::exports::greentic::extension_deploy0_1_0::targets::Diagnostic as WitDiagnostic;
        use crate::host_bindings::deploy::greentic::extension_base0_1_0::types::Severity as WitSeverity;

        let loaded = self
            .loaded
            .load()
            .get(&crate::loaded::ExtensionId(ext_id.to_string()))
            .cloned()
            .ok_or_else(|| RuntimeError::NotFound(ext_id.to_string()))?;

        let (mut store, instance) = loaded
            .build_store_and_instance(
                &self.engine,
                self.config.host_overrides.clone(),
                &crate::host_ports::HostCallContext::default(),
            )
            .map_err(RuntimeError::Wasmtime)?;

        let (iface_idx, iface_name, _version) = resolve_iface_versions(
            &mut store,
            &instance,
            "greentic:extension-deploy/targets",
            DEPLOY_VERSIONS,
        )?;
        let func_idx = instance
            .get_export_index(&mut store, Some(&iface_idx), "validate-credentials")
            .ok_or_else(|| {
                RuntimeError::Wasmtime(anyhow::anyhow!(
                    "interface '{iface_name}' does not export 'validate-credentials'"
                ))
            })?;

        let func = instance
            .get_typed_func::<(String, String), (Vec<WitDiagnostic>,)>(&mut store, &func_idx)
            .map_err(|e| RuntimeError::Wasmtime(e.into()))?;

        let (result,) = func
            .call(
                &mut store,
                (target_id.to_string(), credentials_json.to_string()),
            )
            .map_err(|e| RuntimeError::Wasmtime(e.into()))?;

        Ok(result
            .into_iter()
            .map(|d| crate::types::Diagnostic {
                severity: match d.severity {
                    WitSeverity::Error => crate::types::Severity::Error,
                    WitSeverity::Warning => crate::types::Severity::Warning,
                    WitSeverity::Info => crate::types::Severity::Info,
                    WitSeverity::Hint => crate::types::Severity::Hint,
                },
                code: d.code,
                message: d.message,
                path: d.path,
            })
            .collect())
    }
}

impl ExtensionRuntime {
    /// Return the JSON Schema (as a string) describing credentials required
    /// by the given deploy target.
    pub fn credential_schema(&self, ext_id: &str, target_id: &str) -> Result<String, RuntimeError> {
        let loaded = self
            .loaded
            .load()
            .get(&crate::loaded::ExtensionId(ext_id.to_string()))
            .cloned()
            .ok_or_else(|| RuntimeError::NotFound(ext_id.to_string()))?;

        let (mut store, instance) = loaded
            .build_store_and_instance(
                &self.engine,
                self.config.host_overrides.clone(),
                &crate::host_ports::HostCallContext::default(),
            )
            .map_err(RuntimeError::Wasmtime)?;

        let (iface_idx, iface_name, version) = resolve_iface_versions(
            &mut store,
            &instance,
            "greentic:extension-deploy/targets",
            DEPLOY_VERSIONS,
        )?;
        let func_idx = instance
            .get_export_index(&mut store, Some(&iface_idx), "credential-schema")
            .ok_or_else(|| {
                RuntimeError::Wasmtime(anyhow::anyhow!(
                    "interface '{iface_name}' does not export 'credential-schema'"
                ))
            })?;

        let call_args = (target_id.to_string(),);
        let mapped: Result<String, crate::types::HostExtensionError> = if version == "0.2.0" {
            use crate::host_bindings::deploy_v02::greentic::extension_base0_2_0::types::ExtensionError as E2;
            let func = instance
                .get_typed_func::<(String,), (Result<String, E2>,)>(&mut store, &func_idx)
                .map_err(|e| RuntimeError::Wasmtime(e.into()))?;
            let (r,) = func
                .call(&mut store, call_args)
                .map_err(|e| RuntimeError::Wasmtime(e.into()))?;
            r.map_err(crate::ext_error::from_deploy_v02)
        } else {
            use crate::host_bindings::deploy::greentic::extension_base0_1_0::types::ExtensionError as E1;
            let func = instance
                .get_typed_func::<(String,), (Result<String, E1>,)>(&mut store, &func_idx)
                .map_err(|e| RuntimeError::Wasmtime(e.into()))?;
            let (r,) = func
                .call(&mut store, call_args)
                .map_err(|e| RuntimeError::Wasmtime(e.into()))?;
            r.map_err(crate::ext_error::from_deploy_v01)
        };

        mapped.map_err(RuntimeError::Extension)
    }
}

impl ExtensionRuntime {
    /// Enumerate targets exported by a loaded deploy extension.
    ///
    /// Returns the `list-targets` output as host-side `TargetSummary` values.
    pub fn list_targets(
        &self,
        ext_id: &str,
    ) -> Result<Vec<crate::types::TargetSummary>, RuntimeError> {
        use crate::host_bindings::deploy::exports::greentic::extension_deploy0_1_0::targets::TargetSummary as WitTargetSummary;

        let loaded = self
            .loaded
            .load()
            .get(&crate::loaded::ExtensionId(ext_id.to_string()))
            .cloned()
            .ok_or_else(|| RuntimeError::NotFound(ext_id.to_string()))?;

        let (mut store, instance) = loaded
            .build_store_and_instance(
                &self.engine,
                self.config.host_overrides.clone(),
                &crate::host_ports::HostCallContext::default(),
            )
            .map_err(RuntimeError::Wasmtime)?;

        let (iface_idx, iface_name, _version) = resolve_iface_versions(
            &mut store,
            &instance,
            "greentic:extension-deploy/targets",
            DEPLOY_VERSIONS,
        )?;
        let func_idx = instance
            .get_export_index(&mut store, Some(&iface_idx), "list-targets")
            .ok_or_else(|| {
                RuntimeError::Wasmtime(anyhow::anyhow!(
                    "interface '{iface_name}' does not export 'list-targets'"
                ))
            })?;

        let func = instance
            .get_typed_func::<(), (Vec<WitTargetSummary>,)>(&mut store, &func_idx)
            .map_err(|e| RuntimeError::Wasmtime(e.into()))?;

        let (result,) = func
            .call(&mut store, ())
            .map_err(|e| RuntimeError::Wasmtime(e.into()))?;

        Ok(result
            .into_iter()
            .map(|t| crate::types::TargetSummary {
                id: t.id,
                display_name: t.display_name,
                description: t.description,
                icon_path: t.icon_path,
                supports_rollback: t.supports_rollback,
            })
            .collect())
    }
}

/// Resolve a `greentic:extension-design/<iface>` export by trying `@0.2.0`
/// first and falling back to `@0.1.0`.
///
/// The runtime bumped its WIT to `@0.2.0` in v1.2.x, but several extensions
/// in the wild (http, llm-generic, webhook, platform-bootstrap, ...) were
/// built against `@0.1.0` and have not yet been rebuilt. Without a
/// fallback, every dispatch into those extensions fails with
/// `extension does not export interface 'greentic:extension-design/
/// tools@0.2.0'`. Returning the resolved iface name (with version suffix)
/// lets the nested `func_idx` lookup error name the version that was
/// actually picked.
///
/// `roles@0.2.0` deliberately uses its own dedicated lookup (see
/// `runtime_roles.rs`); it never existed at `@0.1.0`, so no fallback is
/// appropriate there.
/// Tracks extension ids already warned about a legacy WIT contract, so the
/// deprecation notice fires once per extension per process instead of on
/// every dispatch. Bounded by the number of distinct loaded extensions.
static LEGACY_CONTRACT_WARNED: std::sync::OnceLock<
    std::sync::Mutex<std::collections::HashSet<String>>,
> = std::sync::OnceLock::new();

/// Emit a one-shot deprecation warning if `version` is not the newest entry
/// in `newest`. No-op when the extension is already on the current contract
/// or has been warned before.
fn warn_if_legacy_contract(ext_id: &str, version: &str, newest: &str) {
    if version == newest {
        return;
    }
    let set = LEGACY_CONTRACT_WARNED
        .get_or_init(|| std::sync::Mutex::new(std::collections::HashSet::new()));
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
pub(crate) fn resolve_iface_versions(
    store: &mut wasmtime::Store<crate::host_state::HostState>,
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

/// Version tables per package family — newest first.
const DESIGN_VERSIONS: &[&str] = &["0.3.0", "0.2.0", "0.1.0"];
pub(crate) const DEPLOY_VERSIONS: &[&str] = &["0.2.0", "0.1.0"];
const BUNDLE_VERSIONS: &[&str] = &["0.2.0", "0.1.0"];
/// Guardrail interface only exists at 0.3.0 — single-version table.
const GUARDRAIL_VERSIONS: &[&str] = &["0.3.0"];

fn resolve_design_iface(
    store: &mut wasmtime::Store<crate::host_state::HostState>,
    instance: &wasmtime::component::Instance,
    base: &str,
) -> Result<(wasmtime::component::ComponentExportIndex, String), RuntimeError> {
    resolve_iface_versions(store, instance, base, DESIGN_VERSIONS).map(|(idx, name, _)| (idx, name))
}

fn find_extension_dir(p: &std::path::Path) -> Option<std::path::PathBuf> {
    let mut cur = p;
    loop {
        if cur.join("describe.json").exists() {
            return Some(cur.to_path_buf());
        }
        cur = cur.parent()?;
    }
}

impl ExtensionRuntime {
    /// Render a bundle artefact by dispatching to a loaded bundle
    /// extension's `bundling.render` export.
    ///
    /// Mirrors the in-process call site that replaces the legacy
    /// `greentic-bundle ext render` subprocess pipeline. The host
    /// passes the designer session (flow JSON, content JSON, asset
    /// blobs, capability list) and a recipe-specific config string;
    /// the extension's WASM returns the rendered bytes (typically a
    /// `.gtpack` zip) plus the canonical filename and sha256 the
    /// extension wants written.
    ///
    /// Returns `RuntimeError::NotFound` when no extension is loaded
    /// at `ext_id`. The `bundling` interface is resolved newest-first
    /// across `@0.2.0`/`@0.1.0`; host-level failures surface as
    /// `RuntimeError::Wasmtime`, while the extension's WIT-level error
    /// surfaces as `RuntimeError::Extension` (6-variant base at `@0.2.0`,
    /// 4-variant base at `@0.1.0`).
    pub fn render_bundle(
        &self,
        ext_id: &str,
        recipe_id: &str,
        config_json: &str,
        session: crate::types::BundleSession,
    ) -> Result<crate::types::BundleArtifact, RuntimeError> {
        let loaded = self
            .loaded
            .load()
            .get(&crate::loaded::ExtensionId(ext_id.to_string()))
            .cloned()
            .ok_or_else(|| RuntimeError::NotFound(ext_id.to_string()))?;

        let (mut store, instance) = loaded
            .build_store_and_instance(
                &self.engine,
                self.config.host_overrides.clone(),
                &crate::host_ports::HostCallContext::default(),
            )
            .map_err(RuntimeError::Wasmtime)?;

        let (iface_idx, iface_name, version) = resolve_iface_versions(
            &mut store,
            &instance,
            "greentic:extension-bundle/bundling",
            BUNDLE_VERSIONS,
        )?;
        let func_idx = instance
            .get_export_index(&mut store, Some(&iface_idx), "render")
            .ok_or_else(|| {
                RuntimeError::Wasmtime(anyhow::anyhow!(
                    "interface '{iface_name}' does not export 'render'"
                ))
            })?;

        let mapped: Result<crate::types::BundleArtifact, crate::types::HostExtensionError> =
            if version == "0.2.0" {
                use crate::host_bindings::bundle_v02::exports::greentic::extension_bundle0_2_0::bundling::{
                    BundleArtifact as WitBundleArtifact, DesignerSession as WitDesignerSession,
                };
                use crate::host_bindings::bundle_v02::greentic::extension_base0_2_0::types::ExtensionError as E2;
                let func = instance
                    .get_typed_func::<
                        (String, String, WitDesignerSession),
                        (Result<WitBundleArtifact, E2>,),
                    >(&mut store, &func_idx)
                    .map_err(|e| RuntimeError::Wasmtime(e.into()))?;
                let wit_session = WitDesignerSession {
                    flows_json: session.flows_json,
                    contents_json: session.contents_json,
                    assets: session.assets,
                    capabilities_used: session.capabilities_used,
                };
                let (r,) = func
                    .call(
                        &mut store,
                        (recipe_id.to_string(), config_json.to_string(), wit_session),
                    )
                    .map_err(|e| RuntimeError::Wasmtime(e.into()))?;
                r.map(|a| crate::types::BundleArtifact {
                    filename: a.filename,
                    bytes: a.bytes,
                    sha256: a.sha256,
                })
                .map_err(crate::ext_error::from_bundle_v02)
            } else {
                use crate::host_bindings::bundle::exports::greentic::extension_bundle0_1_0::bundling::{
                    BundleArtifact as WitBundleArtifact, DesignerSession as WitDesignerSession,
                };
                use crate::host_bindings::bundle::greentic::extension_base0_1_0::types::ExtensionError as E1;
                let func = instance
                    .get_typed_func::<
                        (String, String, WitDesignerSession),
                        (Result<WitBundleArtifact, E1>,),
                    >(&mut store, &func_idx)
                    .map_err(|e| RuntimeError::Wasmtime(e.into()))?;
                let wit_session = WitDesignerSession {
                    flows_json: session.flows_json,
                    contents_json: session.contents_json,
                    assets: session.assets,
                    capabilities_used: session.capabilities_used,
                };
                let (r,) = func
                    .call(
                        &mut store,
                        (recipe_id.to_string(), config_json.to_string(), wit_session),
                    )
                    .map_err(|e| RuntimeError::Wasmtime(e.into()))?;
                r.map(|a| crate::types::BundleArtifact {
                    filename: a.filename,
                    bytes: a.bytes,
                    sha256: a.sha256,
                })
                .map_err(crate::ext_error::from_bundle_v01)
            };

        mapped.map_err(RuntimeError::Extension)
    }
}

#[cfg(test)]
mod deploy_tests {
    use super::*;

    #[test]
    fn list_targets_returns_error_for_unknown_extension() {
        let tmp = tempfile::TempDir::new().unwrap();
        let config =
            RuntimeConfig::from_paths(crate::DiscoveryPaths::new(tmp.path().to_path_buf()));
        let rt = ExtensionRuntime::new(config).unwrap();
        let err = rt.list_targets("does-not-exist").unwrap_err();
        match err {
            RuntimeError::NotFound(id) => assert_eq!(id, "does-not-exist"),
            other => panic!("expected NotFound, got {other:?}"),
        }
    }

    #[test]
    fn credential_schema_returns_error_for_unknown_extension() {
        let tmp = tempfile::TempDir::new().unwrap();
        let config =
            RuntimeConfig::from_paths(crate::DiscoveryPaths::new(tmp.path().to_path_buf()));
        let rt = ExtensionRuntime::new(config).unwrap();
        let err = rt
            .credential_schema("does-not-exist", "some-target")
            .unwrap_err();
        assert!(matches!(err, RuntimeError::NotFound(_)));
    }

    #[test]
    fn validate_credentials_returns_error_for_unknown_extension() {
        let tmp = tempfile::TempDir::new().unwrap();
        let config =
            RuntimeConfig::from_paths(crate::DiscoveryPaths::new(tmp.path().to_path_buf()));
        let rt = ExtensionRuntime::new(config).unwrap();
        let err = rt
            .validate_credentials("does-not-exist", "target", r"{}")
            .unwrap_err();
        assert!(matches!(err, RuntimeError::NotFound(_)));
    }

    #[test]
    fn render_bundle_returns_error_for_unknown_extension() {
        let tmp = tempfile::TempDir::new().unwrap();
        let config =
            RuntimeConfig::from_paths(crate::DiscoveryPaths::new(tmp.path().to_path_buf()));
        let rt = ExtensionRuntime::new(config).unwrap();
        let err = rt
            .render_bundle(
                "does-not-exist",
                "standard",
                "{}",
                crate::types::BundleSession::default(),
            )
            .unwrap_err();
        assert!(matches!(err, RuntimeError::NotFound(_)));
    }

    #[test]
    fn for_test_constructs_runtime_with_no_extensions() {
        let runtime = ExtensionRuntime::for_test();
        // No extensions are loaded.
        assert!(
            runtime.loaded().is_empty(),
            "for_test runtime must have zero loaded extensions"
        );
    }
}
