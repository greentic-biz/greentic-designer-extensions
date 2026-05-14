use std::path::{Path, PathBuf};
use std::sync::Arc;

use greentic_extension_sdk_contract::{DescribeJson, ExtensionKind};
use wasmtime::Store;
use wasmtime::component::{Component, HasSelf, Instance, Linker};

use crate::health::ExtensionHealth;
use crate::host_state::HostState;
use crate::pool::InstancePool;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ExtensionId(pub String);

impl ExtensionId {
    #[must_use]
    pub fn from_describe(describe: &DescribeJson) -> Self {
        Self(describe.metadata.id.clone())
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for ExtensionId {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

impl From<String> for ExtensionId {
    fn from(s: String) -> Self {
        Self(s)
    }
}

pub struct LoadedExtension {
    pub id: ExtensionId,
    pub describe: Arc<DescribeJson>,
    pub kind: ExtensionKind,
    pub source_dir: PathBuf,
    pub component: Component,
    pub pool: InstancePool,
    pub health: ExtensionHealth,
}

impl LoadedExtension {
    pub fn load_from_dir(engine: &wasmtime::Engine, source_dir: &Path) -> anyhow::Result<Self> {
        let describe_path = source_dir.join("describe.json");
        let describe_bytes = std::fs::read(&describe_path)?;
        let describe_value: serde_json::Value = serde_json::from_slice(&describe_bytes)?;
        greentic_extension_sdk_contract::schema::validate_describe_json(&describe_value)
            .map_err(|e| anyhow::anyhow!("invalid describe.json: {e}"))?;
        let describe: DescribeJson = serde_json::from_value(describe_value)?;
        let id = ExtensionId::from_describe(&describe);
        let wasm_path = source_dir.join(&describe.runtime.component);
        let component = Component::from_file(engine, &wasm_path)?;
        let pool = InstancePool::new(2);
        let kind = describe.kind;
        Ok(Self {
            id,
            describe: Arc::new(describe),
            kind,
            source_dir: source_dir.to_path_buf(),
            component,
            pool,
            health: ExtensionHealth::Healthy,
        })
    }
}

impl LoadedExtension {
    /// Build a fresh wasmtime Store with [`HostState`] and instantiate the component.
    /// Each call creates a new instance (no pooling yet — pooling is future work).
    pub fn build_store_and_instance(
        &self,
        engine: &wasmtime::Engine,
        host_overrides: HostOverrides,
    ) -> anyhow::Result<(Store<HostState>, Instance)> {
        use crate::host_bindings::greentic::extension_host::{
            broker, http, i18n, logging, secrets,
        };

        let mut linker: Linker<HostState> = Linker::new(engine);

        // Wire WASI host functions. cargo-component always adds WASI imports to
        // its output even when the Rust source never calls them directly.
        wasmtime_wasi::p2::add_to_linker_sync(&mut linker)?;

        // HasSelf<T> wraps T and implements HasData — required for wasmtime 43 bindgen add_to_linker.
        logging::add_to_linker::<HostState, HasSelf<HostState>>(&mut linker, |s| s)?;
        i18n::add_to_linker::<HostState, HasSelf<HostState>>(&mut linker, |s| s)?;
        secrets::add_to_linker::<HostState, HasSelf<HostState>>(&mut linker, |s| s)?;
        broker::add_to_linker::<HostState, HasSelf<HostState>>(&mut linker, |s| s)?;
        http::add_to_linker::<HostState, HasSelf<HostState>>(&mut linker, |s| s)?;

        let state = HostState::builder(
            self.id.as_str().to_string(),
            self.describe.runtime.permissions.clone(),
        )
        .translator(host_overrides.translator)
        .secrets_backend(host_overrides.secrets_backend)
        .http_client(host_overrides.http_client)
        .url_matcher(host_overrides.url_matcher)
        .runtime_weak(host_overrides.runtime_weak)
        .call_depth_start(host_overrides.call_depth_start)
        .build();

        let mut store = Store::new(engine, state);
        let instance = linker.instantiate(&mut store, &self.component)?;
        Ok((store, instance))
    }
}

pub type LoadedExtensionRef = Arc<LoadedExtension>;

/// Bundle of overrides every dispatch caller must supply when building a
/// `HostState`. Production code (designer) constructs adapters around
/// `greentic-i18n` + `greentic-secrets`; tests use [`HostOverrides::defaults_for_tests`].
#[derive(Clone)]
pub struct HostOverrides {
    pub translator: std::sync::Arc<dyn crate::host_ports::Translator>,
    pub secrets_backend: std::sync::Arc<dyn crate::host_ports::SecretsBackend>,
    pub http_client: reqwest::blocking::Client,
    pub url_matcher: crate::url_matcher::UrlMatcher,
    pub runtime_weak: std::sync::Weak<crate::runtime::ExtensionRuntime>,
    pub call_depth_start: u32,
}

impl HostOverrides {
    /// Fakes-everywhere helper. Runtime weak is left unset (`Weak::new`), so
    /// broker dispatch will fail with "runtime gone" until B.6 wires the
    /// real `Arc<ExtensionRuntime>`.
    #[must_use]
    pub fn defaults_for_tests() -> Self {
        Self {
            translator: std::sync::Arc::new(crate::host_ports::KeyTranslator),
            secrets_backend: std::sync::Arc::new(crate::host_ports::InMemorySecrets::new()),
            http_client: reqwest::blocking::Client::new(),
            url_matcher: crate::url_matcher::UrlMatcher::default(),
            runtime_weak: std::sync::Weak::new(),
            call_depth_start: 0,
        }
    }
}
