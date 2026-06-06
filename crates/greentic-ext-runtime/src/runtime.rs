use std::collections::HashMap;
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
}

impl RuntimeConfig {
    /// Construct a config from discovery paths, using production-safe
    /// [`HostOverrides::default()`] (no HTTP client, empty secrets/i18n).
    #[must_use]
    pub fn from_paths(paths: DiscoveryPaths) -> Self {
        Self {
            paths,
            host_overrides: HostOverrides::default(),
        }
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
        Self::verify_dir_signature(dir)?;
        let loaded = LoadedExtension::load_from_dir(&self.engine, dir)?;
        let id = loaded.id.clone();

        // Build new registry: clone existing offerings, add new extension's offerings.
        let mut new_registry = CapabilityRegistry::new();
        for existing in self.capability_registry.load().offerings() {
            new_registry.add_offering(existing.clone());
        }
        for cap in &loaded.describe.capabilities.offered {
            let version: semver::Version = cap.version.parse().map_err(|e: semver::Error| {
                RuntimeError::Wasmtime(anyhow::anyhow!("bad offered version: {e}"))
            })?;
            new_registry.add_offering(OfferedBinding {
                extension_id: id.as_str().to_string(),
                cap_id: cap.id.clone(),
                version,
                kind: loaded.kind,
                export_path: String::new(),
            });
        }

        // Atomically swap in new loaded map and registry.
        let mut new_map = (**self.loaded.load()).clone();
        new_map.insert(id.clone(), Arc::new(loaded));
        self.loaded.store(Arc::new(new_map));
        self.capability_registry.store(Arc::new(new_registry));

        let _ = self.events.send(RuntimeEvent::ExtensionInstalled(id));
        Ok(())
    }

    fn verify_dir_signature(dir: &std::path::Path) -> Result<(), RuntimeError> {
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
        // Integrity: the describe is unmodified since signing. This is NOT
        // authenticity — it proves nothing about *who* signed (an attacker can
        // re-sign with their own key). Anchored authenticity
        // (`verify_describe_with_key` against a trust-store / RootVerifier key)
        // is the audit C1 follow-up; it needs a runtime trust store and the
        // org-provisioned prod root key (both currently blocked).
        greentic_extension_sdk_contract::verify_describe_self_consistent(&describe).map_err(
            |e| RuntimeError::SignatureInvalid {
                extension_id: describe.metadata.id.clone(),
                reason: e.to_string(),
            },
        )?;
        let pub_prefix = describe.signature.as_ref().map_or_else(
            || "?".to_string(),
            |s| s.public_key.chars().take(16).collect::<String>(),
        );
        tracing::info!(
            extension_id = %describe.metadata.id,
            key_prefix = %pub_prefix,
            "extension signature verified"
        );
        Self::verify_dir_manifest(dir, &describe)?;
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

    fn handle_removal(&self, dir: &std::path::Path) {
        let current = self.loaded.load();
        let Some((id, _)) = current.iter().find(|(_, v)| v.source_dir == dir) else {
            return;
        };
        let id = id.clone();
        let mut new_map = (**current).clone();
        new_map.remove(&id);
        self.loaded.store(Arc::new(new_map));
        let _ = self.events.send(RuntimeEvent::ExtensionRemoved(id));
    }

    fn handle_added_or_modified(&self, dir: &std::path::Path) -> Result<(), RuntimeError> {
        let loaded = crate::loaded::LoadedExtension::load_from_dir(&self.engine, dir)?;
        let id = loaded.id.clone();
        let mut new_map = (**self.loaded.load()).clone();
        let prev_version = new_map
            .get(&id)
            .map(|e| e.describe.metadata.version.clone());
        new_map.insert(id.clone(), Arc::new(loaded));
        self.loaded.store(Arc::new(new_map));
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

impl ExtensionRuntime {
    /// List all tools exposed by a loaded design extension.
    ///
    /// Calls `greentic:extension-design/tools::list-tools` (resolved
    /// against `@0.2.0` first, then `@0.1.0`) for v1-contract
    /// extensions. **v2 contract** (`apiVersion == "greentic.ai/v2"`)
    /// reads the tools from `describe.contributions.tools[]` — the
    /// runtime WIT no longer exports `list-tools` in that contract.
    /// The declarative v2 entries only carry `name` + `export`, so
    /// `description` / `input_schema_json` come back empty; callers
    /// that need full schemas must introspect the named WIT export.
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
                .map(|t| crate::types::ToolDefinition {
                    name: t.name.clone(),
                    description: String::new(),
                    input_schema_json: String::new(),
                    output_schema_json: None,
                    capabilities: None,
                    agentic_worker_metadata: None,
                })
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
    /// Calls `greentic:extension-design/knowledge::get-entry`
    /// (resolved against `@0.2.0` first, then `@0.1.0`).
    pub fn knowledge_get(
        &self,
        ext_id: &str,
        entry_id: &str,
    ) -> Result<crate::types::KnowledgeEntry, RuntimeError> {
        use crate::host_bindings::exports::greentic::extension_design0_2_0::knowledge::Entry as WitEntry;
        use crate::host_bindings::exports::greentic::extension_design0_2_0::knowledge::ExtensionError;

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
            .get_export_index(&mut store, Some(&iface_idx), "get-entry")
            .ok_or_else(|| {
                RuntimeError::Wasmtime(anyhow::anyhow!(
                    "interface '{iface_name}' does not export 'get-entry'"
                ))
            })?;

        let func = instance
            .get_typed_func::<(String,), (Result<WitEntry, ExtensionError>,)>(&mut store, &func_idx)
            .map_err(|e| RuntimeError::Wasmtime(e.into()))?;

        let (result,) = func
            .call(&mut store, (entry_id.to_string(),))
            .map_err(|e| RuntimeError::Wasmtime(e.into()))?;

        result
            .map(|e| crate::types::KnowledgeEntry {
                id: e.id,
                title: e.title,
                category: e.category,
                tags: e.tags,
                content_json: e.content_json,
            })
            .map_err(|e| {
                RuntimeError::Wasmtime(anyhow::anyhow!(
                    "extension returned error for get-entry '{entry_id}': {e:?}"
                ))
            })
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

        let iface_name = "greentic:extension-deploy/targets@0.1.0";
        let iface_idx = instance
            .get_export_index(&mut store, None, iface_name)
            .ok_or_else(|| {
                RuntimeError::Wasmtime(anyhow::anyhow!(
                    "extension does not export interface '{iface_name}'"
                ))
            })?;
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
        use crate::host_bindings::deploy::greentic::extension_base0_1_0::types::ExtensionError;

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

        let iface_name = "greentic:extension-deploy/targets@0.1.0";
        let iface_idx = instance
            .get_export_index(&mut store, None, iface_name)
            .ok_or_else(|| {
                RuntimeError::Wasmtime(anyhow::anyhow!(
                    "extension does not export interface '{iface_name}'"
                ))
            })?;
        let func_idx = instance
            .get_export_index(&mut store, Some(&iface_idx), "credential-schema")
            .ok_or_else(|| {
                RuntimeError::Wasmtime(anyhow::anyhow!(
                    "interface '{iface_name}' does not export 'credential-schema'"
                ))
            })?;

        let func = instance
            .get_typed_func::<(String,), (Result<String, ExtensionError>,)>(&mut store, &func_idx)
            .map_err(|e| RuntimeError::Wasmtime(e.into()))?;

        let (result,) = func
            .call(&mut store, (target_id.to_string(),))
            .map_err(|e| RuntimeError::Wasmtime(e.into()))?;

        result.map_err(|e| {
            RuntimeError::Wasmtime(anyhow::anyhow!(
                "extension returned error for credential-schema target '{target_id}': {e:?}"
            ))
        })
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

        let iface_name = "greentic:extension-deploy/targets@0.1.0";
        let iface_idx = instance
            .get_export_index(&mut store, None, iface_name)
            .ok_or_else(|| {
                RuntimeError::Wasmtime(anyhow::anyhow!(
                    "extension does not export interface '{iface_name}'"
                ))
            })?;
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
/// Resolve `base@<ver>` against the instance's exports, trying `versions`
/// in order (newest first). Returns the export index, the full resolved
/// interface name, and the bare version string that matched — dispatch
/// code branches on the version to pick the matching typed signature.
fn resolve_iface_versions(
    store: &mut wasmtime::Store<crate::host_state::HostState>,
    instance: &wasmtime::component::Instance,
    base: &str,
    versions: &[&'static str],
) -> Result<(wasmtime::component::ComponentExportIndex, String, &'static str), RuntimeError> {
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
const DEPLOY_VERSIONS: &[&str] = &["0.2.0", "0.1.0"];
const BUNDLE_VERSIONS: &[&str] = &["0.2.0", "0.1.0"];

fn resolve_design_iface(
    store: &mut wasmtime::Store<crate::host_state::HostState>,
    instance: &wasmtime::component::Instance,
    base: &str,
) -> Result<(wasmtime::component::ComponentExportIndex, String), RuntimeError> {
    resolve_iface_versions(store, instance, base, DESIGN_VERSIONS)
        .map(|(idx, name, _)| (idx, name))
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
    /// at `ext_id`, and surfaces every other failure as
    /// `RuntimeError::Wasmtime` with the WIT-level error attached.
    pub fn render_bundle(
        &self,
        ext_id: &str,
        recipe_id: &str,
        config_json: &str,
        session: crate::types::BundleSession,
    ) -> Result<crate::types::BundleArtifact, RuntimeError> {
        use crate::host_bindings::bundle::exports::greentic::extension_bundle0_1_0::bundling::{
            BundleArtifact as WitBundleArtifact, DesignerSession as WitDesignerSession,
        };
        use crate::host_bindings::bundle::greentic::extension_base0_1_0::types::ExtensionError;

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

        let iface_name = "greentic:extension-bundle/bundling@0.1.0";
        let iface_idx = instance
            .get_export_index(&mut store, None, iface_name)
            .ok_or_else(|| {
                RuntimeError::Wasmtime(anyhow::anyhow!(
                    "extension does not export interface '{iface_name}'"
                ))
            })?;
        let func_idx = instance
            .get_export_index(&mut store, Some(&iface_idx), "render")
            .ok_or_else(|| {
                RuntimeError::Wasmtime(anyhow::anyhow!(
                    "interface '{iface_name}' does not export 'render'"
                ))
            })?;

        let func = instance
            .get_typed_func::<
                (String, String, WitDesignerSession),
                (Result<WitBundleArtifact, ExtensionError>,),
            >(&mut store, &func_idx)
            .map_err(|e| RuntimeError::Wasmtime(e.into()))?;

        let wit_session = WitDesignerSession {
            flows_json: session.flows_json,
            contents_json: session.contents_json,
            assets: session.assets,
            capabilities_used: session.capabilities_used,
        };

        let (result,) = func
            .call(
                &mut store,
                (recipe_id.to_string(), config_json.to_string(), wit_session),
            )
            .map_err(|e| RuntimeError::Wasmtime(e.into()))?;

        let artifact = result.map_err(|e| {
            RuntimeError::Wasmtime(anyhow::anyhow!(
                "extension '{ext_id}' returned error rendering recipe '{recipe_id}': {e:?}"
            ))
        })?;

        Ok(crate::types::BundleArtifact {
            filename: artifact.filename,
            bytes: artifact.bytes,
            sha256: artifact.sha256,
        })
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
