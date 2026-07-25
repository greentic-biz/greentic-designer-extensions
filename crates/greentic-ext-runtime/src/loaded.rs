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

/// Deserialize a validated describe `Value` into a typed [`DescribeJson`],
/// migrating a `greentic.ai/v1` describe to the current (v2) shape first.
///
/// The bundled fallback extensions — and any extension authored before the
/// contract 1.1 bump — are v1: `contributions.knowledge` is an array of path
/// strings, `engine` stands in for `compat`, etc. Those do not deserialize
/// into the typed 1.1 structs, so a straight `from_value` fails with a raw
/// "expected struct Knowledge". The contract crate already knows how to lift a
/// v0.4/v1 describe to v2 (`migrate_v0_4_x_value`); run it on read so old
/// describes load instead of being rejected. A v2 describe passes through
/// untouched.
/// Lift a `greentic.ai/v1` describe `Value` to the current (v2) shape; a v2
/// value passes through untouched.
///
/// This is a **`Value`-level** step on purpose: it must run *before*
/// `schema::validate_describe_json`, which only accepts v2 and rejects v1 with
/// "expected greentic.ai/v2; run the v1->v2 migration first". Callers that
/// validate (e.g. `load_from_dir`) migrate first, then validate, then
/// deserialize.
pub(crate) fn migrate_value_if_v1(value: serde_json::Value) -> anyhow::Result<serde_json::Value> {
    const V1_API_VERSION: &str = "greentic.ai/v1";
    let is_v1 = value.get("apiVersion").and_then(serde_json::Value::as_str) == Some(V1_API_VERSION);
    if is_v1 {
        let (migrated, _report) = greentic_extension_sdk_contract::migrate_v0_4_x_value(&value)
            .map_err(|e| anyhow::anyhow!("migrate v1 describe.json to v2: {e}"))?;
        Ok(migrated)
    } else {
        Ok(value)
    }
}

/// Deserialize a describe `Value` into a typed [`DescribeJson`], migrating a
/// `greentic.ai/v1` describe to the current (v2) shape first.
///
/// The bundled fallback extensions — and any extension authored before the
/// contract 1.1 bump — are v1: `contributions.knowledge` is an array of path
/// strings, `engine` stands in for `compat`, etc. Those do not deserialize into
/// the typed 1.1 structs, so a straight `from_value` fails with a raw "expected
/// struct Knowledge". Used where no schema validation is needed (e.g.
/// `verify_dir_signature`).
pub(crate) fn describe_from_value(value: serde_json::Value) -> anyhow::Result<DescribeJson> {
    Ok(serde_json::from_value(migrate_value_if_v1(value)?)?)
}

impl LoadedExtension {
    pub fn load_from_dir(engine: &wasmtime::Engine, source_dir: &Path) -> anyhow::Result<Self> {
        let describe_path = source_dir.join("describe.json");
        let describe_bytes = std::fs::read(&describe_path)?;
        let describe_value: serde_json::Value = serde_json::from_slice(&describe_bytes)?;
        // Migrate a v1 describe to v2 BEFORE schema validation — the schema is
        // v2-only and rejects v1 outright, so validating first would fail the
        // very extensions this migration exists to load.
        let describe_value = migrate_value_if_v1(describe_value)?;
        greentic_extension_sdk_contract::schema::validate_describe_json(&describe_value)
            .map_err(|e| anyhow::anyhow!("invalid describe.json: {e}"))?;
        let describe: DescribeJson = serde_json::from_value(describe_value)?;
        let id = ExtensionId::from_describe(&describe);
        let wasm_path = wasm_component_path(&describe, source_dir)?;
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
        ctx: &crate::host_ports::HostCallContext,
    ) -> anyhow::Result<(Store<HostState>, Instance)> {
        use crate::host_bindings::greentic::extension_host::{
            broker, http, i18n, llm, logging, secrets,
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
        llm::add_to_linker::<HostState, HasSelf<HostState>>(&mut linker, |s| s)?;
        crate::host_bindings::design_v04::greentic::oauth_broker::broker_v1::add_to_linker::<
            HostState,
            HasSelf<HostState>,
        >(&mut linker, |s| s)?;

        // Per-extension network allow-list: when the extension declares
        // `runtime.permissions.network` patterns, those patterns become the
        // authoritative allow-list for this extension (replace semantics —
        // the host-level override is NOT added). When no patterns are
        // declared the host-level override is used unchanged (deny-all by
        // default). See `effective_url_matcher` for the loopback-http rule.
        let url_matcher = effective_url_matcher(
            &self.describe.runtime.permissions.network,
            host_overrides.url_matcher,
        );

        let state = HostState::builder(
            self.id.as_str().to_string(),
            self.describe.runtime.permissions.clone(),
        )
        .translator(host_overrides.translator)
        .secrets_backend(host_overrides.secrets_backend)
        .http_client(host_overrides.http_client)
        .llm_port(host_overrides.llm_port)
        .call_ctx(ctx.clone())
        .url_matcher(url_matcher)
        .runtime_weak(host_overrides.runtime_weak)
        .call_depth_start(host_overrides.call_depth_start)
        .oauth_config(host_overrides.oauth_config.clone())
        .build();

        let mut store = Store::new(engine, state);
        let instance = linker.instantiate(&mut store, &self.component)?;
        Ok((store, instance))
    }
}

/// Resolve the WASM component path for an extension's runtime component.
///
/// Extensions that use the dual-component layout ship:
/// - Root `extension.wasm` — design-side WebAssembly with metadata (channel
///   name, icon, i18n, schemas). This is what the designer loads.
/// - A runtime gtpack (e.g. `runtime/provider.gtpack`) — either a placeholder
///   text file or a real .gtpack ZIP. The real runner-host WASM lives downstream
///   and is fetched lazily there; the designer must never try to parse it.
///
/// Multiple extension kinds follow this dual-component layout:
/// - `ProviderExtension` (e.g. `greentic.provider.telegram-1.3.1-research`)
/// - `DesignExtension` (e.g. `greentic.llm-openai-1.3.1-research`) — has a
///   real 80–900 KB `extension.wasm`; `describe.json` points at
///   `runtime/component-llm-openai.gtpack` (a 929 KB .gtpack ZIP that wasmtime
///   cannot parse as a raw component).
/// - `BundleExtension` (e.g. `greentic.bundle-standard-1.3.0-research`) — has
///   a 938 KB `extension.wasm`; `describe.json` points at a `.gtxpack` that
///   may not even exist in the installed directory.
///
/// Strategy: if `<source_dir>/extension.wasm` exists, prefer it unconditionally
/// regardless of kind. Only the runner-host — which has its own separate loader
/// path — needs the runtime gtpack declared in `describe.runtime.components`.
/// Designer's boot loader only consumes design-side metadata and UI assets.
///
/// Older single-component extensions that ship no `extension.wasm` at root fall
/// back to `describe.runtime.components[X].gtpack.file` resolved relative to
/// `source_dir`, exactly as before.
///
/// v2's `runtime.components` is a map keyed by component id. ext-runtime today
/// loads a single WASM component per extension, so we require exactly one entry.
/// Multi-component dispatch (driven by `runtime_ref` on nodeTypes/tools) is a
/// follow-up — when it lands, callers will pick the component by id and this
/// helper goes away.
fn wasm_component_path(describe: &DescribeJson, source_dir: &Path) -> anyhow::Result<PathBuf> {
    // Dual-component layout: extensions that ship a design-side `extension.wasm`
    // at the source-dir root use it for designer-side loading regardless of kind.
    // The runtime gtpack declared in `describe.runtime.components` stays meaningful
    // for runner-host (flow-execution time), which has its own separate loader path.
    //
    // Provider, llm-openai (DesignExtension), and bundle-standard (BundleExtension)
    // all follow this layout. Older single-component extensions that don't ship
    // `extension.wasm` fall back to the describe.json declared path below.
    let design_wasm = source_dir.join("extension.wasm");
    if design_wasm.exists() {
        return Ok(design_wasm);
    }

    // Fallback for older single-component extensions: read
    // `describe.runtime.components[X].gtpack.file` and resolve it relative to
    // `source_dir`. These kinds already point at real WASM at that path.
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
    Ok(source_dir.join(gtpack.file.as_str()))
}

/// Select the URL matcher for a single extension instantiation.
///
/// **Replace semantics:** when the extension's `describe.json` declares one
/// or more patterns under `runtime.permissions.network`, those patterns are
/// the authoritative allow-list for that extension and a fresh
/// [`UrlMatcher`] is built from them (with the loopback-http rule applied —
/// see below). The host-level `override_matcher` is **ignored** in this
/// path — it is the host-wide default that applies only to extensions that
/// make no network declaration.
///
/// When the declaration is empty the host-level override is returned
/// unchanged, which is the deny-all default in most deployments. This
/// preserves existing behavior for extensions that do not need outbound HTTP.
///
/// # Loopback-http rule
///
/// [`UrlMatcher`] rejects non-`https` URLs by default (scheme-downgrade
/// defence) and only honours plain `http` when `with_allow_http(true)` is
/// set. That toggle is **matcher-wide** — it cannot be scoped to a single
/// pattern. To let an extension talk to a local dev service over
/// `http://127.0.0.1` / `http://localhost` WITHOUT also opening plain http
/// to public hosts, we:
///
/// 1. drop any declared `http://` pattern whose host is NOT loopback (it
///    could never be safely honoured — a public-host plain-http downgrade
///    is exactly the attack the matcher defends against), and
/// 2. enable `with_allow_http(true)` only when at least one *loopback*
///    `http://` pattern survives.
///
/// Because the matcher matches scheme exactly per declared pattern, a
/// co-declared `https://host/*` pattern still requires `https` even when
/// the toggle is on — the toggle only decides whether `http` patterns are
/// consulted at all, and after step 1 the only surviving `http` patterns
/// are loopback.
///
/// # Arguments
///
/// * `declared_patterns` — the `runtime.permissions.network` slice from
///   the extension's parsed `describe.json`.
/// * `override_matcher` — the host-level matcher supplied via
///   [`HostOverrides`]. Used only when `declared_patterns` is empty.
///
/// # Returns
///
/// A [`UrlMatcher`] that enforces the correct allow-list for this extension.
pub(crate) fn effective_url_matcher(
    declared_patterns: &[String],
    override_matcher: crate::url_matcher::UrlMatcher,
) -> crate::url_matcher::UrlMatcher {
    if declared_patterns.is_empty() {
        return override_matcher;
    }

    // Replace path: build the effective matcher exclusively from the
    // extension's declared patterns (the host override does NOT apply).
    let mut patterns: Vec<String> = declared_patterns.to_vec();

    // Loopback-http handling: keep loopback http patterns, drop public-host
    // http patterns (they can never be honoured safely), and record whether
    // any loopback http pattern remains so we can flip the matcher-wide
    // allow_http toggle.
    let mut allow_loopback_http = false;
    patterns.retain(|p| {
        if let Some(host) = http_pattern_host(p) {
            if is_loopback_host(host) {
                allow_loopback_http = true;
                true
            } else {
                tracing::warn!(
                    pattern = %p,
                    "dropping non-loopback http url pattern; plain http is only honoured for loopback hosts"
                );
                false
            }
        } else {
            // https (or any non-http) pattern — kept verbatim; UrlMatcher
            // validates it on construction.
            true
        }
    });

    crate::url_matcher::UrlMatcher::from_patterns(patterns).with_allow_http(allow_loopback_http)
}

/// Return the host portion of a `http://` pattern, or `None` when the
/// pattern is not plain http. The leading `*.` wildcard label (e.g.
/// `http://*.example.com/*`) is stripped so the remaining host can be
/// classified; a bare wildcard host is treated as non-loopback.
///
/// Bracketed IPv6 literals (e.g. `[::1]` in `http://[::1]:8787/*`) are
/// returned with their brackets intact so that `is_loopback_host` can strip
/// them: splitting on the first `:` would otherwise yield the bare `"["`
/// opener and misclassify `[::1]` as non-loopback.
fn http_pattern_host(pattern: &str) -> Option<&str> {
    let rest = pattern.strip_prefix("http://")?;
    let host_and_port = rest.split('/').next().unwrap_or(rest);
    // Strip the userinfo (`user@host`) if present.
    let host_and_port = host_and_port.rsplit('@').next().unwrap_or(host_and_port);
    // Bracketed IPv6 literal: `[::1]` or `[::1]:8787`.
    // Return the bracketed token (including the `]`) so is_loopback_host can
    // strip the brackets and compare against `::1`.
    let host = if let Some(bracket_end) = host_and_port.find(']') {
        &host_and_port[..=bracket_end]
    } else {
        // Plain hostname or IPv4: split on first `:` to drop optional port.
        host_and_port.split(':').next().unwrap_or(host_and_port)
    };
    Some(host.trim_start_matches("*."))
}

/// Loopback hosts for which plain http is acceptable: `localhost`,
/// `127.0.0.1` (any IPv4 loopback in `127.0.0.0/8` would also qualify, but
/// the only spellings extensions declare in practice are these two and
/// `[::1]`), and the IPv6 loopback.
fn is_loopback_host(host: &str) -> bool {
    let host = host.trim_start_matches('[').trim_end_matches(']');
    host.eq_ignore_ascii_case("localhost") || host == "127.0.0.1" || host == "::1"
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
    pub llm_port: Option<std::sync::Arc<dyn crate::host_ports::LlmPort>>,
    pub url_matcher: crate::url_matcher::UrlMatcher,
    pub runtime_weak: std::sync::Weak<crate::runtime::ExtensionRuntime>,
    pub call_depth_start: u32,
    pub oauth_config: Option<crate::oauth::OAuthBrokerConfig>,
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
            .field("llm_port", &self.llm_port.as_ref().map(|_| "<dyn LlmPort>"))
            .field("url_matcher", &self.url_matcher)
            .field(
                "runtime_weak",
                &self
                    .runtime_weak
                    .upgrade()
                    .map(|_| "<Arc<ExtensionRuntime>>"),
            )
            .field("call_depth_start", &self.call_depth_start)
            .field(
                "oauth_config",
                &self.oauth_config.as_ref().map(|_| "<OAuthBrokerConfig>"),
            )
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
            llm_port: None,
            url_matcher: crate::url_matcher::UrlMatcher::default(),
            runtime_weak: std::sync::Weak::new(),
            call_depth_start: 0,
            oauth_config: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::url_matcher::UrlMatcher;

    /// The real bundled Adaptive Cards describe — `apiVersion: greentic.ai/v1`,
    /// with `contributions.knowledge` as an array of path strings — which does
    /// not deserialize into the contract 1.1 typed structs. Before this fix a
    /// straight `from_value` failed with "expected struct Knowledge", so every
    /// bundled fallback extension (all v1) was unloadable under the 1.1 runtime.
    /// It must now migrate on read and load.
    ///
    /// Uses the actual bundled describe rather than a hand-built minimal one so
    /// the test can't drift from the real v1 shape the runtime must accept.
    const AC_V1_DESCRIBE: &str = include_str!("testdata/ac_v1_describe.json");

    /// Reproduces the `load_from_dir` sequence exactly: migrate the v1 Value,
    /// THEN run the v2-only schema validation, THEN deserialize. The previous
    /// order (validate first) rejected v1 with "expected greentic.ai/v2" before
    /// migration ever ran — a bug the direct-`describe_from_value` test missed
    /// because it skips validation.
    #[test]
    fn v1_describe_survives_validate_after_migration() {
        let value: serde_json::Value =
            serde_json::from_str(AC_V1_DESCRIBE).expect("fixture is valid JSON");

        let migrated = migrate_value_if_v1(value).expect("v1 migrates");
        greentic_extension_sdk_contract::schema::validate_describe_json(&migrated)
            .expect("migrated describe passes the v2 schema");
        let describe: DescribeJson =
            serde_json::from_value(migrated).expect("migrated describe deserializes");
        assert_eq!(describe.metadata.id, "greentic.adaptive-cards");
    }

    #[test]
    fn describe_from_value_migrates_the_bundled_v1_describe() {
        let value: serde_json::Value =
            serde_json::from_str(AC_V1_DESCRIBE).expect("fixture is valid JSON");
        assert_eq!(
            value.get("apiVersion").and_then(|v| v.as_str()),
            Some("greentic.ai/v1"),
            "fixture must be a v1 describe for this test to mean anything"
        );

        let describe = describe_from_value(value).expect("bundled v1 describe must migrate + load");
        assert_eq!(describe.metadata.id, "greentic.adaptive-cards");
    }

    fn empty_override() -> UrlMatcher {
        UrlMatcher::default()
    }

    fn override_with_pattern(pattern: &str) -> UrlMatcher {
        UrlMatcher::from_patterns(vec![pattern.to_string()])
    }

    /// Extensions that declare network patterns must have exactly those
    /// patterns enforced — the host-level override must NOT apply.
    #[test]
    fn declared_patterns_allow_declared_host_and_deny_undeclared() {
        let declared = vec!["https://api.github.com/*".to_string()];
        let matcher = effective_url_matcher(&declared, empty_override());

        assert!(
            matcher.is_allowed("https://api.github.com/repos/org/repo"),
            "declared host must be allowed"
        );
        assert!(
            !matcher.is_allowed("https://evil.com/"),
            "undeclared host must be denied even though host override is empty"
        );
    }

    /// When no network patterns are declared the host-level override is
    /// returned verbatim — behavior is unchanged for legacy extensions.
    #[test]
    fn empty_declaration_falls_back_to_host_override() {
        let override_matcher = override_with_pattern("https://allowed.com/*");
        let matcher = effective_url_matcher(&[], override_matcher);

        assert!(
            matcher.is_allowed("https://allowed.com/path"),
            "host-override host must be reachable when declare is empty"
        );
        assert!(
            !matcher.is_allowed("https://other.com/path"),
            "host-override deny must still apply"
        );
    }

    /// Non-empty declaration REPLACES (not unions) the host-level
    /// override. A broader operator override must not bleed through to
    /// an extension that declared its own narrower allow-list.
    #[test]
    fn declared_patterns_replace_not_union_host_override() {
        let declared = vec!["https://api.github.com/*".to_string()];
        let override_matcher = override_with_pattern("https://operator-allowed.com/*");
        let matcher = effective_url_matcher(&declared, override_matcher);

        assert!(
            matcher.is_allowed("https://api.github.com/repos/org/repo"),
            "declared host must be allowed"
        );
        assert!(
            !matcher.is_allowed("https://operator-allowed.com/anything"),
            "operator override must NOT bleed through when declaration is non-empty"
        );
    }

    /// Empty declaration + empty host override must deny every URL —
    /// this is the default deny-all posture for extensions that never
    /// call the network.
    #[test]
    fn empty_declaration_and_empty_override_denies_everything() {
        let matcher = effective_url_matcher(&[], empty_override());

        assert!(
            !matcher.is_allowed("https://api.github.com/anything"),
            "empty declaration + empty override must produce deny-all matcher"
        );
    }

    /// A declared loopback `http://127.0.0.1` pattern must be reachable
    /// over plain http. The matcher rejects non-https by default, so the
    /// effective matcher has to opt http in — but ONLY because the
    /// declared pattern is loopback.
    #[test]
    fn declared_http_loopback_127_allows_plain_http() {
        let declared = vec!["http://127.0.0.1:8787/*".to_string()];
        let matcher = effective_url_matcher(&declared, empty_override());

        assert!(
            matcher.is_allowed("http://127.0.0.1:8787/execute"),
            "declared http loopback pattern must permit plain http to that loopback"
        );
    }

    /// `http://localhost` is the other loopback spelling and must behave
    /// the same as `127.0.0.1`.
    #[test]
    fn declared_http_loopback_localhost_allows_plain_http() {
        let declared = vec!["http://localhost:8787/*".to_string()];
        let matcher = effective_url_matcher(&declared, empty_override());

        assert!(
            matcher.is_allowed("http://localhost:8787/execute"),
            "declared http localhost pattern must permit plain http to localhost"
        );
    }

    /// The loopback-http opt-in must NOT leak to non-loopback http: a
    /// declared `http://evil.com` pattern must stay denied (no plain-http
    /// downgrade for a public host) even though the pattern technically
    /// targets http.
    #[test]
    fn declared_http_non_loopback_stays_denied() {
        let declared = vec!["http://evil.com/*".to_string()];
        let matcher = effective_url_matcher(&declared, empty_override());

        assert!(
            !matcher.is_allowed("http://evil.com/anything"),
            "plain http must stay denied for a non-loopback declared host"
        );
    }

    /// A mixed declaration (loopback http + a normal https host) must keep
    /// https reachable AND the loopback http reachable, while still
    /// refusing plain http to the https host (the global `allow_http` toggle
    /// must not downgrade the https-only host because no http pattern for
    /// it exists, and `is_allowed` matches scheme exactly per pattern).
    #[test]
    fn mixed_loopback_http_and_https_host() {
        let declared = vec![
            "http://127.0.0.1:8787/*".to_string(),
            "https://api.example.com/*".to_string(),
        ];
        let matcher = effective_url_matcher(&declared, empty_override());

        assert!(
            matcher.is_allowed("http://127.0.0.1:8787/execute"),
            "loopback http must be allowed in a mixed declaration"
        );
        assert!(
            matcher.is_allowed("https://api.example.com/v1/foo"),
            "declared https host must stay reachable"
        );
        assert!(
            !matcher.is_allowed("http://api.example.com/v1/foo"),
            "plain http to the https-only host must stay denied even with loopback http enabled"
        );
    }

    /// A bracketed IPv6 loopback `http://[::1]:8787/*` must survive the
    /// loopback filter and allow plain http to `http://[::1]:8787/x`.
    ///
    /// The url crate's `host_str()` returns the bracketed form `"[::1]"` for
    /// both the pattern and the request URL, so the Exact host rule matches.
    /// The bug this test guards against: `http_pattern_host` previously split
    /// on the first `:`, yielding `"["` as the host, which was classified as
    /// non-loopback and dropped.
    #[test]
    fn declared_http_ipv6_loopback_allows_plain_http() {
        let declared = vec!["http://[::1]:8787/*".to_string()];
        let matcher = effective_url_matcher(&declared, empty_override());

        assert!(
            matcher.is_allowed("http://[::1]:8787/x"),
            "declared http IPv6 loopback pattern must permit plain http to [::1]"
        );
        // Must not bleed to arbitrary non-loopback hosts.
        assert!(
            !matcher.is_allowed("http://evil.com/x"),
            "IPv6 loopback opt-in must not permit plain http to non-loopback hosts"
        );
    }

    /// An adversarial pattern `http://[::1].evil.com/*` that tries to smuggle
    /// a non-loopback host inside brackets must be rejected. The url crate
    /// refuses to parse this (it is not a valid bracketed IPv6 literal), so
    /// the pattern is either unparseable (dropped by `UrlMatcher`) or the
    /// resulting host does not match `[::1]` in `is_loopback_host`.
    ///
    /// Either way the request to `http://[::1].evil.com/x` must be denied.
    #[test]
    fn adversarial_fake_ipv6_bracket_host_is_denied() {
        let declared = vec!["http://[::1].evil.com/*".to_string()];
        let matcher = effective_url_matcher(&declared, empty_override());

        // The pattern is malformed: url::Url::parse rejects `[::1].evil.com`
        // as a host, so the pattern is silently dropped and the matcher
        // remains deny-all for this declaration.
        assert!(
            !matcher.is_allowed("http://[::1].evil.com/x"),
            "malformed bracketed host must not be allowed"
        );
        // Real IPv6 loopback must also NOT be granted by a bad pattern.
        assert!(
            !matcher.is_allowed("http://[::1]/x"),
            "bad pattern must not accidentally allow real IPv6 loopback"
        );
    }
}
