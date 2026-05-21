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
        let wasm_path = source_dir.join(single_gtpack_file(&describe)?);
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

/// Resolve the single gtpack.file path for an extension's runtime component.
///
/// v2's `runtime.components` is a map keyed by component id. ext-runtime
/// today loads a single wasm component per extension, so we require exactly
/// one entry. Multi-component dispatch (driven by `runtime_ref` on
/// nodeTypes/tools) is a follow-up — when it lands, callers will pick the
/// component by id and this helper goes away.
fn single_gtpack_file(describe: &DescribeJson) -> anyhow::Result<&str> {
    let mut iter = describe.runtime.components.iter();
    let Some((id, component)) = iter.next() else {
        anyhow::bail!("describe.runtime.components must declare at least one entry");
    };
    if iter.next().is_some() {
        anyhow::bail!(
            "describe.runtime.components has more than one entry; multi-component dispatch is not yet implemented"
        );
    }
    let gtpack = component.gtpack.as_ref().ok_or_else(|| {
        anyhow::anyhow!(
            "describe.runtime.components[{id:?}].gtpack must be set for source-dir loads (OCI-only deploy is not yet supported)",
        )
    })?;
    Ok(gtpack.file.as_str())
}

pub type LoadedExtensionRef = Arc<LoadedExtension>;

/// Bundle of overrides every dispatch caller must supply when building a
/// `HostState`. Production code (designer) constructs adapters around
/// `greentic-i18n` + `greentic-secrets`; tests use [`HostOverrides::defaults_for_tests`].
///
/// `http_client` is `Option` because `reqwest::blocking::Client` spawns an
/// internal tokio runtime, and dropping that runtime from inside an
/// outer async context panics with "Cannot drop a runtime in a context
/// where blocking is not allowed". Tests instantiate `ExtensionRuntime`
/// inside `#[tokio::test]` bodies but never call `http::fetch`, so
/// they leave the client `None` — `host_state` will surface a clean
/// "http client not configured" error if a test ever does invoke fetch.
/// Production callers pass `Some(client)` once at startup.
#[derive(Clone)]
pub struct HostOverrides {
    pub translator: std::sync::Arc<dyn crate::host_ports::Translator>,
    pub secrets_backend: std::sync::Arc<dyn crate::host_ports::SecretsBackend>,
    pub http_client: Option<reqwest::blocking::Client>,
    pub url_matcher: crate::url_matcher::UrlMatcher,
    pub runtime_weak: std::sync::Weak<crate::runtime::ExtensionRuntime>,
    pub call_depth_start: u32,
}

impl std::fmt::Debug for HostOverrides {
    /// Opaque debug representation: trait-object fields cannot provide
    /// structural debug output, and `reqwest::blocking::Client` does not
    /// implement `Debug`. We show field presence rather than field values.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HostOverrides")
            .field("translator", &"<dyn Translator>")
            .field("secrets_backend", &"<dyn SecretsBackend>")
            .field(
                "http_client",
                &self.http_client.as_ref().map(|_| "<Client>"),
            )
            .field("url_matcher", &self.url_matcher)
            .field(
                "runtime_weak",
                &self
                    .runtime_weak
                    .upgrade()
                    .map(|_| "<Arc<ExtensionRuntime>>"),
            )
            .field("call_depth_start", &self.call_depth_start)
            .finish()
    }
}

impl HostOverrides {
    /// Fakes-everywhere helper. `http_client` is `None` so dropping the
    /// runtime inside an outer async context never panics; the test never
    /// hits the path that uses it. Runtime weak is left unset (`Weak::new`),
    /// so broker dispatch returns "no runtime context available" until
    /// the cross-extension dispatch cascade lands.
    #[must_use]
    pub fn defaults_for_tests() -> Self {
        Self::default()
    }
}

impl Default for HostOverrides {
    /// Production-safe defaults: key-pass-through translator (i18n key
    /// returned verbatim), empty in-memory secrets, no HTTP client (callers
    /// that need HTTP must supply `Some(client)` via
    /// `RuntimeConfig::with_host_overrides` or
    /// `ExtensionRuntime::with_host_overrides`), empty URL allow-list, and
    /// no broker-runtime weak reference (cross-extension dispatch returns
    /// "no runtime context available" until the cascade cascade lands).
    ///
    /// `http_client` is intentionally `None` rather than eagerly constructed
    /// because `reqwest::blocking::Client` spawns its own internal tokio
    /// runtime; dropping that runtime from inside an outer `#[tokio::test]`
    /// body panics with "Cannot drop a runtime in a context where blocking is
    /// not allowed". Tests leave it `None`; production callers pass
    /// `Some(client)` once at startup.
    fn default() -> Self {
        Self {
            translator: std::sync::Arc::new(crate::host_ports::KeyTranslator),
            secrets_backend: std::sync::Arc::new(crate::host_ports::InMemorySecrets::new()),
            http_client: None,
            url_matcher: crate::url_matcher::UrlMatcher::default(),
            runtime_weak: std::sync::Weak::new(),
            call_depth_start: 0,
        }
    }
}
