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

    pub fn register_loaded_from_dir(
        &mut self,
        dir: &std::path::Path,
    ) -> Result<(), RuntimeError> {
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
}
