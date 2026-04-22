use std::collections::HashMap;
use std::sync::Arc;

use arc_swap::ArcSwap;
use tokio::sync::broadcast;
use wasmtime::Engine;

use crate::capability::{CapabilityRegistry, OfferedBinding};
use crate::discovery::DiscoveryPaths;
use crate::error::RuntimeError;
use crate::loaded::{ExtensionId, LoadedExtension, LoadedExtensionRef};

#[derive(Clone, Debug)]
pub struct RuntimeConfig {
    pub paths: DiscoveryPaths,
}

impl RuntimeConfig {
    #[must_use]
    pub fn from_paths(paths: DiscoveryPaths) -> Self {
        Self { paths }
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
        if std::env::var("GREENTIC_EXT_ALLOW_UNSIGNED").is_ok() {
            tracing::warn!(
                extension_dir = %dir.display(),
                "GREENTIC_EXT_ALLOW_UNSIGNED is set — signature verification skipped"
            );
            return Ok(());
        }
        let path = dir.join("describe.json");
        let raw = std::fs::read_to_string(&path)?;
        let describe: greentic_ext_contract::DescribeJson = serde_json::from_str(&raw)?;
        greentic_ext_contract::verify_describe(&describe).map_err(|e| {
            RuntimeError::SignatureInvalid {
                extension_id: describe.metadata.id.clone(),
                reason: e.to_string(),
            }
        })?;
        let pub_prefix = describe.signature.as_ref().map_or_else(
            || "?".to_string(),
            |s| s.public_key.chars().take(16).collect::<String>(),
        );
        tracing::info!(
            extension_id = %describe.metadata.id,
            key_prefix = %pub_prefix,
            "extension signature verified"
        );
        Ok(())
    }

    /// Spawns a watcher background thread. Events trigger reload of the
    /// affected extension's directory. Returns a stop sender — dropping or
    /// sending on it signals the watcher thread to exit. Also returns the
    /// thread `JoinHandle` for callers that want to wait for clean shutdown.
    pub fn start_watcher(self: Arc<Self>) -> Result<WatcherGuard, RuntimeError> {
        let paths: Vec<std::path::PathBuf> = self.config.paths.all().into_iter().cloned().collect();
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
    /// `greentic:extension-design/tools@0.1.0::invoke-tool`, and returns the
    /// JSON result string.
    pub fn invoke_tool(
        &self,
        ext_id: &str,
        tool_name: &str,
        args_json: &str,
    ) -> Result<String, RuntimeError> {
        use crate::host_bindings::greentic::extension_base::types::ExtensionError;

        let loaded = self
            .loaded
            .load()
            .get(&crate::loaded::ExtensionId(ext_id.to_string()))
            .cloned()
            .ok_or_else(|| RuntimeError::NotFound(ext_id.to_string()))?;

        let (mut store, instance) = loaded
            .build_store_and_instance(&self.engine)
            .map_err(RuntimeError::Wasmtime)?;

        // Resolve the nested export: first the interface instance, then the function.
        // This is the wasmtime 43 pattern: get_export_index(store, parent, name).
        let iface_name = "greentic:extension-design/tools@0.1.0";
        let iface_idx = instance
            .get_export_index(&mut store, None, iface_name)
            .ok_or_else(|| {
                RuntimeError::Wasmtime(anyhow::anyhow!(
                    "extension does not export interface '{iface_name}'"
                ))
            })?;
        let func_idx = instance
            .get_export_index(&mut store, Some(&iface_idx), "invoke-tool")
            .ok_or_else(|| {
                RuntimeError::Wasmtime(anyhow::anyhow!(
                    "interface '{iface_name}' does not export 'invoke-tool'"
                ))
            })?;

        let func = instance
            .get_typed_func::<(String, String), (Result<String, ExtensionError>,)>(
                &mut store, &func_idx,
            )
            .map_err(|e| RuntimeError::Wasmtime(e.into()))?;

        let (result,) = func
            .call(&mut store, (tool_name.to_string(), args_json.to_string()))
            .map_err(|e| RuntimeError::Wasmtime(e.into()))?;
        // post_return is deprecated/no-op in wasmtime 43 — not called.

        result.map_err(|e| {
            RuntimeError::Wasmtime(anyhow::anyhow!(
                "extension returned error for tool '{tool_name}': {e:?}"
            ))
        })
    }
}

impl ExtensionRuntime {
    /// Validate extension-specific content against the extension's schema.
    ///
    /// Calls `greentic:extension-design/validation@0.1.0::validate-content`.
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
        use crate::host_bindings::exports::greentic::extension_design::validation::{
            Diagnostic as WitDiagnostic, ValidateResult as WitValidateResult,
        };
        use crate::host_bindings::greentic::extension_base::types::Severity as WitSeverity;

        let loaded = self
            .loaded
            .load()
            .get(&crate::loaded::ExtensionId(ext_id.to_string()))
            .cloned()
            .ok_or_else(|| RuntimeError::NotFound(ext_id.to_string()))?;

        let (mut store, instance) = loaded
            .build_store_and_instance(&self.engine)
            .map_err(RuntimeError::Wasmtime)?;

        let iface_name = "greentic:extension-design/validation@0.1.0";
        let iface_idx = instance
            .get_export_index(&mut store, None, iface_name)
            .ok_or_else(|| {
                RuntimeError::Wasmtime(anyhow::anyhow!(
                    "extension does not export interface '{iface_name}'"
                ))
            })?;
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
    /// Calls `greentic:extension-design/tools@0.1.0::list-tools`.
    pub fn list_tools(
        &self,
        ext_id: &str,
    ) -> Result<Vec<crate::types::ToolDefinition>, RuntimeError> {
        use crate::host_bindings::exports::greentic::extension_design::tools::ToolDefinition as WitToolDef;

        let loaded = self
            .loaded
            .load()
            .get(&crate::loaded::ExtensionId(ext_id.to_string()))
            .cloned()
            .ok_or_else(|| RuntimeError::NotFound(ext_id.to_string()))?;

        let (mut store, instance) = loaded
            .build_store_and_instance(&self.engine)
            .map_err(RuntimeError::Wasmtime)?;

        let iface_name = "greentic:extension-design/tools@0.1.0";
        let iface_idx = instance
            .get_export_index(&mut store, None, iface_name)
            .ok_or_else(|| {
                RuntimeError::Wasmtime(anyhow::anyhow!(
                    "extension does not export interface '{iface_name}'"
                ))
            })?;
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
            })
            .collect())
    }
}

impl ExtensionRuntime {
    /// Retrieve system prompt fragments from a loaded design extension.
    ///
    /// Calls `greentic:extension-design/prompting@0.1.0::system-prompt-fragments`.
    pub fn prompt_fragments(
        &self,
        ext_id: &str,
    ) -> Result<Vec<crate::types::PromptFragment>, RuntimeError> {
        use crate::host_bindings::exports::greentic::extension_design::prompting::PromptFragment as WitFrag;

        let loaded = self
            .loaded
            .load()
            .get(&crate::loaded::ExtensionId(ext_id.to_string()))
            .cloned()
            .ok_or_else(|| RuntimeError::NotFound(ext_id.to_string()))?;

        let (mut store, instance) = loaded
            .build_store_and_instance(&self.engine)
            .map_err(RuntimeError::Wasmtime)?;

        let iface_name = "greentic:extension-design/prompting@0.1.0";
        let iface_idx = instance
            .get_export_index(&mut store, None, iface_name)
            .ok_or_else(|| {
                RuntimeError::Wasmtime(anyhow::anyhow!(
                    "extension does not export interface '{iface_name}'"
                ))
            })?;
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
    /// Calls `greentic:extension-design/knowledge@0.1.0::list-entries`.
    pub fn knowledge_list(
        &self,
        ext_id: &str,
        category_filter: Option<&str>,
    ) -> Result<Vec<crate::types::KnowledgeEntrySummary>, RuntimeError> {
        use crate::host_bindings::exports::greentic::extension_design::knowledge::EntrySummary as WitSummary;

        let loaded = self
            .loaded
            .load()
            .get(&crate::loaded::ExtensionId(ext_id.to_string()))
            .cloned()
            .ok_or_else(|| RuntimeError::NotFound(ext_id.to_string()))?;

        let (mut store, instance) = loaded
            .build_store_and_instance(&self.engine)
            .map_err(RuntimeError::Wasmtime)?;

        let iface_name = "greentic:extension-design/knowledge@0.1.0";
        let iface_idx = instance
            .get_export_index(&mut store, None, iface_name)
            .ok_or_else(|| {
                RuntimeError::Wasmtime(anyhow::anyhow!(
                    "extension does not export interface '{iface_name}'"
                ))
            })?;
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
    /// Calls `greentic:extension-design/knowledge@0.1.0::get-entry`.
    pub fn knowledge_get(
        &self,
        ext_id: &str,
        entry_id: &str,
    ) -> Result<crate::types::KnowledgeEntry, RuntimeError> {
        use crate::host_bindings::exports::greentic::extension_design::knowledge::Entry as WitEntry;
        use crate::host_bindings::exports::greentic::extension_design::knowledge::ExtensionError;

        let loaded = self
            .loaded
            .load()
            .get(&crate::loaded::ExtensionId(ext_id.to_string()))
            .cloned()
            .ok_or_else(|| RuntimeError::NotFound(ext_id.to_string()))?;

        let (mut store, instance) = loaded
            .build_store_and_instance(&self.engine)
            .map_err(RuntimeError::Wasmtime)?;

        let iface_name = "greentic:extension-design/knowledge@0.1.0";
        let iface_idx = instance
            .get_export_index(&mut store, None, iface_name)
            .ok_or_else(|| {
                RuntimeError::Wasmtime(anyhow::anyhow!(
                    "extension does not export interface '{iface_name}'"
                ))
            })?;
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
    /// Calls `greentic:extension-design/knowledge@0.1.0::suggest-entries`.
    pub fn knowledge_suggest(
        &self,
        ext_id: &str,
        query: &str,
        limit: u32,
    ) -> Result<Vec<crate::types::KnowledgeEntrySummary>, RuntimeError> {
        use crate::host_bindings::exports::greentic::extension_design::knowledge::EntrySummary as WitSummary;

        let loaded = self
            .loaded
            .load()
            .get(&crate::loaded::ExtensionId(ext_id.to_string()))
            .cloned()
            .ok_or_else(|| RuntimeError::NotFound(ext_id.to_string()))?;

        let (mut store, instance) = loaded
            .build_store_and_instance(&self.engine)
            .map_err(RuntimeError::Wasmtime)?;

        let iface_name = "greentic:extension-design/knowledge@0.1.0";
        let iface_idx = instance
            .get_export_index(&mut store, None, iface_name)
            .ok_or_else(|| {
                RuntimeError::Wasmtime(anyhow::anyhow!(
                    "extension does not export interface '{iface_name}'"
                ))
            })?;
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
    s: crate::host_bindings::exports::greentic::extension_design::knowledge::EntrySummary,
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
        use crate::host_bindings::deploy::exports::greentic::extension_deploy::targets::Diagnostic as WitDiagnostic;
        use crate::host_bindings::deploy::greentic::extension_base::types::Severity as WitSeverity;

        let loaded = self
            .loaded
            .load()
            .get(&crate::loaded::ExtensionId(ext_id.to_string()))
            .cloned()
            .ok_or_else(|| RuntimeError::NotFound(ext_id.to_string()))?;

        let (mut store, instance) = loaded
            .build_store_and_instance(&self.engine)
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
    pub fn credential_schema(
        &self,
        ext_id: &str,
        target_id: &str,
    ) -> Result<String, RuntimeError> {
        use crate::host_bindings::deploy::greentic::extension_base::types::ExtensionError;

        let loaded = self
            .loaded
            .load()
            .get(&crate::loaded::ExtensionId(ext_id.to_string()))
            .cloned()
            .ok_or_else(|| RuntimeError::NotFound(ext_id.to_string()))?;

        let (mut store, instance) = loaded
            .build_store_and_instance(&self.engine)
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
        use crate::host_bindings::deploy::exports::greentic::extension_deploy::targets::TargetSummary as WitTargetSummary;

        let loaded = self
            .loaded
            .load()
            .get(&crate::loaded::ExtensionId(ext_id.to_string()))
            .cloned()
            .ok_or_else(|| RuntimeError::NotFound(ext_id.to_string()))?;

        let (mut store, instance) = loaded
            .build_store_and_instance(&self.engine)
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

fn find_extension_dir(p: &std::path::Path) -> Option<std::path::PathBuf> {
    let mut cur = p;
    loop {
        if cur.join("describe.json").exists() {
            return Some(cur.to_path_buf());
        }
        cur = cur.parent()?;
    }
}

#[cfg(test)]
mod deploy_tests {
    use super::*;

    #[test]
    fn list_targets_returns_error_for_unknown_extension() {
        let tmp = tempfile::TempDir::new().unwrap();
        let config = RuntimeConfig::from_paths(crate::DiscoveryPaths::new(tmp.path().to_path_buf()));
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
        let config = RuntimeConfig::from_paths(crate::DiscoveryPaths::new(tmp.path().to_path_buf()));
        let rt = ExtensionRuntime::new(config).unwrap();
        let err = rt.credential_schema("does-not-exist", "some-target").unwrap_err();
        assert!(matches!(err, RuntimeError::NotFound(_)));
    }

    #[test]
    fn validate_credentials_returns_error_for_unknown_extension() {
        let tmp = tempfile::TempDir::new().unwrap();
        let config = RuntimeConfig::from_paths(crate::DiscoveryPaths::new(tmp.path().to_path_buf()));
        let rt = ExtensionRuntime::new(config).unwrap();
        let err = rt
            .validate_credentials("does-not-exist", "target", r#"{}"#)
            .unwrap_err();
        assert!(matches!(err, RuntimeError::NotFound(_)));
    }
}
