use std::collections::HashMap;
use std::sync::Arc;

use arc_swap::ArcSwap;
use tokio::sync::broadcast;
use wasmtime::Engine;

use crate::capability::CapabilityRegistry;
use crate::discovery::DiscoveryPaths;
use crate::error::RuntimeError;
use crate::loaded::{ExtensionId, LoadedExtensionRef};

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
}
