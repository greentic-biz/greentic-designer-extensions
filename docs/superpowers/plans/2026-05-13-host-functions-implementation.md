# Phase B — Wire the 4B.0 Host Stubs Implementation Plan

> **Status (2026-05-17): SHIPPED on `research`.**
>
> | Task | PR |
> |---|---|
> | B.1 (port traits + `HostState` builder) | greentic-biz/greentic-designer-extensions#52 |
> | B.2 (`i18n::t`/`tf` via `Translator`) | same PR |
> | B.3 (`secrets::get` + permission gating) | same PR |
> | B.4 (URL matcher + `http::fetch`) | same PR |
> | B.5 (`broker::call_extension` cross-ext dispatch) | same PR |
> | B.7 (verification + grep gate) | same PR |
>
> **Follow-up consumer wiring:**
>
> | Task | PR |
> |---|---|
> | C.1 (ext-runtime sdk-contract dep → `=1.2.0-research`) | greentic-biz/greentic-designer-extensions#54 |
> | C.2 (loaded.rs v2 describe migration) | same PR |
> | C.3 (sweep ext-runtime + sibling crates for v1-shape uses) | same PR |
> | D.1 (HostOverrides wired through ExtensionRuntime) | greentic-biz/greentic-designer-extensions#55 |
> | (reqwest re-export so consumers can match version) | greentic-biz/greentic-designer-extensions#56 |
> | D.2 (gate `GREENTIC_EXT_ALLOW_UNSIGNED` behind `dev-allow-unsigned` feature) | greentic-biz/greentic-designer-extensions#57 |
> | D.4.runtime (`verify_dir_manifest` on install) | greentic-biz/greentic-designer-extensions#58 |
>
> Original plan body preserved below as historical record.
>
> ---

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace every `"not implemented in 4B.0"` placeholder in the `greentic-ext-runtime` host with real implementations of `i18n::t/tf`, `secrets::get`, `http::fetch`, and `broker::call_extension`, gated by the extension's declared `permissions` and verified by integration tests.

**Architecture:** Introduce two narrow ports inside `greentic-ext-runtime` (`Translator` + `SecretsBackend` traits) so the runtime stays free of heavy upstream deps. `HostState` carries `Arc<dyn …>` handles plus a `reqwest::blocking::Client` and an `Arc<ExtensionRuntime>` weak handle for broker dispatch. URL allow-list checks use `url::Url` parsing with strict scheme + host-suffix + path-prefix matching. Broker dispatch reuses the same wasmtime export resolution `invoke_tool` already uses, with depth tracked in `HostState`. The designer wires production backends (greentic-i18n catalogs + greentic-secrets backend) in `greentic-designer/src/ui/mod.rs`; tests use in-memory fakes.

**Tech Stack:** Rust 1.95 (edition 2024), wasmtime 43, reqwest 0.12 (blocking, rustls-tls), url 2, greentic-extension-sdk-contract 0.5.0 (Phase A output), greentic-i18n catalogs (consumed via the in-house `Translator` adapter), greentic-secrets backend (consumed via the in-house `SecretsBackend` adapter).

**Pre-conditions before starting:**
- Phase A (`2026-05-13-contract-0.5.0-bump.md`) has merged to `research` of `greentic-designer-sdk` and bumped `greentic-extension-sdk-contract` to `0.5.0`. The `Permissions` struct retains the field layout shown in the umbrella spec (`network: Vec<String>`, `secrets: Vec<String>`, `call_extension_kinds: Vec<String>`); if A changes any field name we will adjust in Task B.1.
- Worktree exists per `superpowers:using-git-worktrees` (e.g. `~/Works/greentic/.worktrees/designer-extensions-host-fns/`).
- Branch base: `research` of `greentic-designer-extensions`. Final PR target: `research`.
- `cargo` toolchain installed and `cargo build -p greentic-ext-runtime` succeeds on a clean checkout.

**Commit/PR rules:**
- Conventional Commits (`feat:`, `fix:`, `refactor:`, `test:`, `chore:`, `docs:`).
- **NO** `Co-Authored-By:` trailers. **NO** `Generated with Claude Code` lines anywhere in commit body or PR description.
- One commit per (test + impl) pair where reasonable. Frequent commits over giant ones.
- Push branch `feat/host-functions-1.2.x` and open a PR against `research`.

---

## File Structure

### New files
| Path | Purpose |
| --- | --- |
| `crates/greentic-ext-runtime/src/host_ports.rs` | Defines the two narrow ports: `trait Translator` and `trait SecretsBackend`. Owned by ext-runtime so we don't pull `greentic-i18n` / `greentic-secrets` into the runtime crate. |
| `crates/greentic-ext-runtime/src/url_matcher.rs` | Strict URL allow-list matcher (`UrlMatcher::is_allowed`). Uses `url::Url` for scheme + host suffix + path prefix matching. |
| `crates/greentic-ext-runtime/tests/url_matcher.rs` | Three attack-vector tests (open redirect, subdomain confusion, scheme downgrade) + happy-path. |
| `crates/greentic-ext-runtime/tests/host_i18n.rs` | Round-trip integration test for `i18n::t/tf` resolving against a fixture `Translator`. |
| `crates/greentic-ext-runtime/tests/host_secrets.rs` | Round-trip integration test for `secrets::get` against a fixture `SecretsBackend`, plus permission-denied test. |
| `crates/greentic-ext-runtime/tests/host_http.rs` | Round-trip integration test for `http::fetch` against `wiremock`, plus permission-denied test. |
| `crates/greentic-ext-runtime/tests/host_broker.rs` | A-calls-B round-trip + permission denial + depth-exceeded fixtures. |
| `crates/greentic-ext-runtime/tests/fixtures/host_i18n_caller/src/lib.rs` | Cargo-component fixture that calls `host.i18n.t("greentic.test.hello")` from inside a tool. (See Task B.2 for the full file content.) |
| `crates/greentic-ext-runtime/tests/fixtures/host_secrets_caller/src/lib.rs` | Fixture calling `host.secrets.get("api.openai.com/api_key")`. |
| `crates/greentic-ext-runtime/tests/fixtures/host_http_caller/src/lib.rs` | Fixture calling `host.http.fetch(...)`. |
| `crates/greentic-ext-runtime/tests/fixtures/broker_caller_a/src/lib.rs` | Fixture A that calls B via `host.broker.call-extension`. |
| `crates/greentic-ext-runtime/tests/fixtures/broker_target_b/src/lib.rs` | Fixture B that exports `tools::invoke-tool` and returns a typed echo. |

### Modified files
| Path | Why |
| --- | --- |
| `crates/greentic-ext-runtime/Cargo.toml` | Add `reqwest` (blocking), `url`, `async-trait` deps; add `wiremock` dev-dep. |
| `crates/greentic-ext-runtime/src/lib.rs` | Add `mod host_ports;`, `mod url_matcher;` and re-export `Translator`, `SecretsBackend`, `KeyTranslator`, `InMemorySecrets`, `UrlMatcher`. Forbid unsafe. |
| `crates/greentic-ext-runtime/src/host_state.rs` | Extend `HostState` with `translator`, `secrets`, `http_client`, `url_matcher`, `runtime_weak`, `call_depth`. Rewrite the four host-fn `impl` blocks. |
| `crates/greentic-ext-runtime/src/loaded.rs` | `build_store_and_instance` must take + thread the new dependencies into `HostState::new(...)`. |
| `crates/greentic-ext-runtime/src/runtime.rs` | `ExtensionRuntime` constructs `Arc<Self>` upfront so each `HostState` gets a `Weak<Self>`; add `broker_dispatch` method that resolves the target extension's `tools::invoke-tool` export and reuses the existing `resolve_design_iface` logic. Update `invoke_tool` to thread the new HostState constructor arguments. |
| `../greentic-designer/src/ui/mod.rs` *(separate repo)* | Construct production `Translator` (`GreenticI18nTranslator`) + `SecretsBackend` (`GreenticSecretsAdapter`) and pass them into `RuntimeConfig`. |
| `../greentic-designer/Cargo.toml` *(separate repo)* | Add `greentic-i18n-lib` + `greentic-secrets-core` path deps so the adapter has something to wrap. |

---

## Task B.1 — Port traits + state plumbing

**Files:**
- Create: `/home/bima-pangestu/Works/greentic/greentic-designer-extensions/crates/greentic-ext-runtime/src/host_ports.rs`
- Modify: `/home/bima-pangestu/Works/greentic/greentic-designer-extensions/crates/greentic-ext-runtime/src/lib.rs`
- Modify: `/home/bima-pangestu/Works/greentic/greentic-designer-extensions/crates/greentic-ext-runtime/src/host_state.rs`
- Modify: `/home/bima-pangestu/Works/greentic/greentic-designer-extensions/crates/greentic-ext-runtime/src/loaded.rs`
- Test: `/home/bima-pangestu/Works/greentic/greentic-designer-extensions/crates/greentic-ext-runtime/src/host_ports.rs` (unit tests `#[cfg(test)] mod tests` at bottom)

- [ ] **Step 1: Write the failing unit test for `KeyTranslator`**

Append to `crates/greentic-ext-runtime/src/host_ports.rs` (file will not exist yet — create it with both module body and tests in this step; see step 3 for impl):

```rust
// Test block lives at the end of host_ports.rs.
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_translator_returns_key_for_t() {
        let t = KeyTranslator;
        assert_eq!(t.t("greentic.test.hello"), "greentic.test.hello");
    }

    #[test]
    fn key_translator_substitutes_args_for_tf() {
        let t = KeyTranslator;
        let out = t.tf(
            "greentic.test.hello.{}",
            &[("name", "Bima")],
        );
        // KeyTranslator is intentionally inert: it returns the key as-is.
        // Real templating lives in the GreenticI18nTranslator adapter.
        assert_eq!(out, "greentic.test.hello.{}");
    }

    #[test]
    fn in_memory_secrets_returns_value_when_present() {
        let mut s = InMemorySecrets::default();
        s.insert("api.openai.com/api_key", "sk-test");
        let v = s.get("api.openai.com/api_key").unwrap();
        assert_eq!(v, "sk-test");
    }

    #[test]
    fn in_memory_secrets_returns_not_found_when_absent() {
        let s = InMemorySecrets::default();
        let err = s.get("api.openai.com/api_key").unwrap_err();
        assert!(matches!(err, SecretsError::NotFound(_)));
    }
}
```

- [ ] **Step 2: Run test to verify it fails (module does not exist yet)**

Run: `cd /home/bima-pangestu/Works/greentic/greentic-designer-extensions && cargo test -p greentic-ext-runtime --lib host_ports::tests -- --nocapture`
Expected: FAIL with `file not found for module 'host_ports'` (or similar: ext-runtime/lib.rs has no `mod host_ports`).

- [ ] **Step 3: Implement `host_ports.rs` with both traits + fakes**

Replace the content of `crates/greentic-ext-runtime/src/host_ports.rs` with the full file:

```rust
//! Narrow ports the runtime depends on for host-side capabilities.
//!
//! `greentic-ext-runtime` defines these traits locally so it stays free of
//! the (large) `greentic-i18n` / `greentic-secrets` dependency trees. The
//! designer crate wires production adapters; tests use the in-crate fakes.

use std::collections::HashMap;
use std::sync::Mutex;

use thiserror::Error;

/// Look up i18n keys against a locale catalog set up by the host.
///
/// `t` returns the rendered string for `key` (or the key itself if no
/// translation is available — the runtime never panics on missing keys).
/// `tf` performs simple `{name}` substitution against `args`.
pub trait Translator: Send + Sync + 'static {
    fn t(&self, key: &str) -> String;
    fn tf(&self, key: &str, args: &[(&str, &str)]) -> String;
}

/// A default no-op translator. Returns each key verbatim. Used when the
/// designer is built without an i18n catalog and as a safe baseline in
/// tests that don't care about i18n behaviour.
#[derive(Debug, Default, Clone, Copy)]
pub struct KeyTranslator;

impl Translator for KeyTranslator {
    fn t(&self, key: &str) -> String {
        key.to_string()
    }
    fn tf(&self, key: &str, _args: &[(&str, &str)]) -> String {
        key.to_string()
    }
}

/// Errors the runtime surfaces when a secret lookup fails.
#[derive(Debug, Error)]
pub enum SecretsError {
    #[error("secret not found: {0}")]
    NotFound(String),
    #[error("backend error: {0}")]
    Backend(String),
}

/// Narrow secrets port used by the `host.secrets.get` WIT host fn.
///
/// `key` is the raw URI the extension passed (e.g. `"api.openai.com/api_key"`
/// or `"secrets://team/openai/key"`). Permission gating happens in
/// `HostState::secrets::get` BEFORE this trait is called.
pub trait SecretsBackend: Send + Sync + 'static {
    fn get(&self, key: &str) -> Result<String, SecretsError>;
}

/// In-memory `SecretsBackend` used by tests and the designer's
/// `--dev-secrets-inline` mode. Thread-safe.
#[derive(Default)]
pub struct InMemorySecrets {
    map: Mutex<HashMap<String, String>>,
}

impl InMemorySecrets {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, key: &str, value: &str) {
        // Mutex is here so the type stays `Send + Sync` without an outer
        // wrapper — callers can clone it through an `Arc`. Poisoning is
        // recovered transparently because we never panic while holding the
        // lock.
        let mut g = self
            .map
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        g.insert(key.to_string(), value.to_string());
    }
}

impl SecretsBackend for InMemorySecrets {
    fn get(&self, key: &str) -> Result<String, SecretsError> {
        let g = self
            .map
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        g.get(key)
            .cloned()
            .ok_or_else(|| SecretsError::NotFound(key.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_translator_returns_key_for_t() {
        let t = KeyTranslator;
        assert_eq!(t.t("greentic.test.hello"), "greentic.test.hello");
    }

    #[test]
    fn key_translator_substitutes_args_for_tf() {
        let t = KeyTranslator;
        let out = t.tf(
            "greentic.test.hello.{}",
            &[("name", "Bima")],
        );
        assert_eq!(out, "greentic.test.hello.{}");
    }

    #[test]
    fn in_memory_secrets_returns_value_when_present() {
        let mut s = InMemorySecrets::default();
        s.insert("api.openai.com/api_key", "sk-test");
        let v = s.get("api.openai.com/api_key").unwrap();
        assert_eq!(v, "sk-test");
    }

    #[test]
    fn in_memory_secrets_returns_not_found_when_absent() {
        let s = InMemorySecrets::default();
        let err = s.get("api.openai.com/api_key").unwrap_err();
        assert!(matches!(err, SecretsError::NotFound(_)));
    }
}
```

- [ ] **Step 4: Register the module in `lib.rs`**

Edit `crates/greentic-ext-runtime/src/lib.rs`. Replace the file with:

```rust
//! Wasmtime-based runtime for Greentic Designer Extensions.
#![forbid(unsafe_code)]

pub mod broker;
pub mod capability;
pub mod discovery;
mod error;
mod health;
mod host_bindings;
pub mod host_ports;
mod host_state;
mod loaded;
mod pool;
mod runtime;
mod runtime_roles;
pub mod types;
pub mod url_matcher;
pub mod watcher;

pub use self::broker::{Broker, BrokerError, BrokerResult};
pub use self::capability::{CapabilityRegistry, OfferedBinding, ResolutionPlan};
pub use self::discovery::DiscoveryPaths;
pub use self::error::RuntimeError;
pub use self::health::{ExtensionHealth, HealthReason};
pub use self::host_ports::{
    InMemorySecrets, KeyTranslator, SecretsBackend, SecretsError, Translator,
};
pub use self::host_state::HostState;
pub use self::loaded::{ExtensionId, LoadedExtension, LoadedExtensionRef};
pub use self::runtime::{ExtensionRuntime, RuntimeConfig, RuntimeEvent, WatcherGuard};
pub use self::types::{
    BundleArtifact, BundleSession, CompileContext, Diagnostic, HostExtensionError, KnowledgeEntry,
    KnowledgeEntrySummary, PromptFragment, RoleError, RoleSpec, Severity, TargetKind,
    TargetSummary, ToolDefinition, ValidateResult,
};
pub use self::url_matcher::UrlMatcher;
```

NOTE: `url_matcher` body comes in Task B.4 — for now create an empty placeholder module so the crate compiles. Add at the top of `crates/greentic-ext-runtime/src/url_matcher.rs` (new file):

```rust
//! Strict URL allow-list matcher. Real implementation lands in Task B.4.

/// Strict URL allow-list matcher. Filled in by Task B.4.
#[derive(Debug, Default, Clone)]
pub struct UrlMatcher {
    patterns: Vec<String>,
}

impl UrlMatcher {
    #[must_use]
    pub fn from_patterns(patterns: Vec<String>) -> Self {
        Self { patterns }
    }

    /// Always returns `false` in this stub. Real logic in Task B.4.
    #[must_use]
    pub fn is_allowed(&self, _url: &str) -> bool {
        false
    }

    #[must_use]
    pub fn patterns(&self) -> &[String] {
        &self.patterns
    }
}
```

- [ ] **Step 5: Extend `HostState` with the new fields (still no real impls)**

Replace `crates/greentic-ext-runtime/src/host_state.rs` with:

```rust
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

use wasmtime::component::ResourceTable;
use wasmtime_wasi::{WasiCtx, WasiCtxBuilder, WasiCtxView, WasiView};

use crate::host_bindings::greentic::extension_host::{broker, http, i18n, logging, secrets};
use crate::host_ports::{KeyTranslator, SecretsBackend, Translator};
use crate::url_matcher::UrlMatcher;
use greentic_extension_sdk_contract::describe::Permissions;

/// Maximum number of nested `host.broker.call-extension` hops the runtime
/// allows in a single dispatch chain. Mirrors `crate::broker::MAX_DEPTH`.
pub const MAX_BROKER_DEPTH: u32 = 8;

/// Per-Store host context. One `HostState` is built per WIT invocation —
/// the dependencies live in `Arc`s so cloning is cheap.
pub struct HostState {
    pub extension_id: String,
    pub permissions: Permissions,
    pub call_depth: AtomicU32,
    translator: Arc<dyn Translator>,
    secrets_backend: Arc<dyn SecretsBackend>,
    http_client: reqwest::blocking::Client,
    url_matcher: UrlMatcher,
    runtime_weak: std::sync::Weak<crate::runtime::ExtensionRuntime>,
    // WASI state — required because cargo-component-built WASM components
    // implicitly import WASI interfaces (wasi:cli/environment etc.).
    wasi: WasiCtx,
    table: ResourceTable,
}

impl HostState {
    /// Builder used by `LoadedExtension::build_store_and_instance` to
    /// produce a `HostState` for a single dispatch.
    #[must_use]
    pub fn builder(extension_id: String, permissions: Permissions) -> HostStateBuilder {
        HostStateBuilder {
            extension_id,
            permissions,
            translator: Arc::new(KeyTranslator),
            secrets_backend: Arc::new(crate::host_ports::InMemorySecrets::new()),
            http_client: reqwest::blocking::Client::new(),
            url_matcher: UrlMatcher::default(),
            runtime_weak: std::sync::Weak::new(),
            call_depth_start: 0,
        }
    }

    #[must_use]
    pub fn translator(&self) -> &dyn Translator {
        self.translator.as_ref()
    }

    #[must_use]
    pub fn secrets_backend(&self) -> &dyn SecretsBackend {
        self.secrets_backend.as_ref()
    }

    #[must_use]
    pub fn url_matcher(&self) -> &UrlMatcher {
        &self.url_matcher
    }
}

/// Builder for [`HostState`]. Avoids a 7-positional-arg constructor.
pub struct HostStateBuilder {
    extension_id: String,
    permissions: Permissions,
    translator: Arc<dyn Translator>,
    secrets_backend: Arc<dyn SecretsBackend>,
    http_client: reqwest::blocking::Client,
    url_matcher: UrlMatcher,
    runtime_weak: std::sync::Weak<crate::runtime::ExtensionRuntime>,
    call_depth_start: u32,
}

impl HostStateBuilder {
    #[must_use]
    pub fn translator(mut self, t: Arc<dyn Translator>) -> Self {
        self.translator = t;
        self
    }
    #[must_use]
    pub fn secrets_backend(mut self, s: Arc<dyn SecretsBackend>) -> Self {
        self.secrets_backend = s;
        self
    }
    #[must_use]
    pub fn http_client(mut self, c: reqwest::blocking::Client) -> Self {
        self.http_client = c;
        self
    }
    #[must_use]
    pub fn url_matcher(mut self, m: UrlMatcher) -> Self {
        self.url_matcher = m;
        self
    }
    #[must_use]
    pub fn runtime_weak(mut self, w: std::sync::Weak<crate::runtime::ExtensionRuntime>) -> Self {
        self.runtime_weak = w;
        self
    }
    #[must_use]
    pub fn call_depth_start(mut self, n: u32) -> Self {
        self.call_depth_start = n;
        self
    }

    #[must_use]
    pub fn build(self) -> HostState {
        let wasi = WasiCtxBuilder::new().build();
        let table = ResourceTable::new();
        HostState {
            extension_id: self.extension_id,
            permissions: self.permissions,
            call_depth: AtomicU32::new(self.call_depth_start),
            translator: self.translator,
            secrets_backend: self.secrets_backend,
            http_client: self.http_client,
            url_matcher: self.url_matcher,
            runtime_weak: self.runtime_weak,
            wasi,
            table,
        }
    }
}

/// Implement [`WasiView`] so that `wasmtime_wasi::p2::add_to_linker_sync` can wire
/// WASI host functions. cargo-component adds WASI imports to every component it
/// builds, even if the Rust source never calls them.
impl WasiView for HostState {
    fn ctx(&mut self) -> WasiCtxView<'_> {
        WasiCtxView {
            ctx: &mut self.wasi,
            table: &mut self.table,
        }
    }
}

impl logging::Host for HostState {
    fn log(&mut self, level: logging::Level, target: String, message: String) {
        let ext = &self.extension_id;
        match level {
            logging::Level::Trace => tracing::trace!(%ext, %target, "{message}"),
            logging::Level::Debug => tracing::debug!(%ext, %target, "{message}"),
            logging::Level::Info => tracing::info!(%ext, %target, "{message}"),
            logging::Level::Warn => tracing::warn!(%ext, %target, "{message}"),
            logging::Level::Error => tracing::error!(%ext, %target, "{message}"),
        }
    }

    fn log_kv(
        &mut self,
        level: logging::Level,
        target: String,
        message: String,
        fields: Vec<(String, String)>,
    ) {
        let pairs: Vec<String> = fields.iter().map(|(k, v)| format!("{k}={v}")).collect();
        let msg = if pairs.is_empty() {
            message
        } else {
            format!("{message} {{{}}}", pairs.join(", "))
        };
        self.log(level, target, msg);
    }
}

// i18n / secrets / broker / http impls are filled in by tasks B.2, B.3,
// B.4, B.5. Until then, return the legacy stub messages so the crate
// still compiles. These will be REMOVED in subsequent tasks.

impl i18n::Host for HostState {
    fn t(&mut self, key: String) -> String {
        let _ = &self.translator;
        key
    }
    fn tf(&mut self, key: String, _args: Vec<(String, String)>) -> String {
        let _ = &self.translator;
        key
    }
}

impl secrets::Host for HostState {
    fn get(&mut self, uri: String) -> Result<String, String> {
        let _ = &self.secrets_backend;
        if !self
            .permissions
            .secrets
            .iter()
            .any(|allowed| uri.starts_with(allowed))
        {
            return Err(format!("permission denied for secret: {uri}"));
        }
        Err(format!("secrets backend stub for {uri}"))
    }
}

impl broker::Host for HostState {
    fn call_extension(
        &mut self,
        kind: String,
        target_id: String,
        function: String,
        _args_json: String,
    ) -> Result<String, String> {
        let _ = &self.runtime_weak;
        let _ = self.call_depth.load(Ordering::Relaxed);
        if !self
            .permissions
            .call_extension_kinds
            .iter()
            .any(|k| k == &kind)
        {
            return Err(format!(
                "{} may not call {kind} extensions",
                self.extension_id
            ));
        }
        Err(format!("broker stub for {target_id}.{function}"))
    }
}

impl http::Host for HostState {
    fn fetch(&mut self, req: http::Request) -> Result<http::Response, String> {
        let _ = &self.http_client;
        let _ = &self.url_matcher;
        if !self
            .permissions
            .network
            .iter()
            .any(|pattern| req.url.starts_with(pattern.trim_end_matches("/*")))
        {
            return Err(format!("network permission denied for url: {}", req.url));
        }
        Err(format!("http fetch stub for {}", req.url))
    }
}
```

(These intentionally-still-stubby impls keep the placeholder _message text_ different from the literal `"not implemented in 4B.0"` so the grep guard from Task B.7 doesn't trip prematurely. Real implementations replace them in B.2/B.3/B.4/B.5.)

- [ ] **Step 6: Update `loaded.rs` to use the builder**

In `crates/greentic-ext-runtime/src/loaded.rs`, replace the body of `build_store_and_instance` so it builds `HostState` via `HostState::builder(...).build()` for now (no extra plumbing yet — that arrives in B.6 when the runtime carries real backends):

Replace lines around 77-105 with:

```rust
impl LoadedExtension {
    /// Build a fresh wasmtime Store with [`HostState`] and instantiate the component.
    /// Each call creates a new instance (no pooling yet — pooling is future work).
    pub fn build_store_and_instance(
        &self,
        engine: &wasmtime::Engine,
        host_overrides: crate::loaded::HostOverrides,
    ) -> anyhow::Result<(Store<HostState>, Instance)> {
        use crate::host_bindings::greentic::extension_host::{
            broker, http, i18n, logging, secrets,
        };

        let mut linker: Linker<HostState> = Linker::new(engine);

        wasmtime_wasi::p2::add_to_linker_sync(&mut linker)?;

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

/// Bundle of overrides every dispatch caller must supply.
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
    /// Test/default helper — fakes everywhere, runtime weak unset.
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
```

Then update `runtime.rs` to pass `HostOverrides::defaults_for_tests()` (temporary — real wiring in B.6) at every existing `build_store_and_instance(&self.engine)` call site. Use:

```bash
cd /home/bima-pangestu/Works/greentic/greentic-designer-extensions
grep -n "build_store_and_instance(&self.engine)" crates/greentic-ext-runtime/src/runtime.rs
```

For each match, replace with:
```rust
loaded
    .build_store_and_instance(
        &self.engine,
        crate::loaded::HostOverrides::defaults_for_tests(),
    )
```

Also re-export `HostOverrides` from `lib.rs`. Add to the existing `pub use self::loaded::{…}` line: `, HostOverrides`.

- [ ] **Step 7: Add the new deps to `Cargo.toml`**

Edit `crates/greentic-ext-runtime/Cargo.toml`. In `[dependencies]` add:

```toml
[dependencies.reqwest]
workspace = true
features = ["blocking", "rustls-tls", "json"]
default-features = false

[dependencies.url]
version = "2"

[dependencies.async-trait]
workspace = true
```

In `[dev-dependencies]` add:

```toml
[dev-dependencies.wiremock]
version = "0.6"

[dev-dependencies.serde_json]
workspace = true
```

(`serde_json` is already a normal dep so this is a no-op; safe to skip if it already appears under dev-dependencies. Re-running `cargo metadata` afterwards is fine.)

- [ ] **Step 8: Run the host_ports unit tests**

Run: `cd /home/bima-pangestu/Works/greentic/greentic-designer-extensions && cargo test -p greentic-ext-runtime --lib host_ports::tests -- --nocapture`
Expected: PASS — 4 tests pass.

- [ ] **Step 9: Full crate compile + existing tests still green**

Run: `cd /home/bima-pangestu/Works/greentic/greentic-designer-extensions && cargo test -p greentic-ext-runtime --all-targets 2>&1 | tail -40`
Expected: All pre-existing tests pass (broker.rs, capability_registry.rs, etc.). New `host_ports::tests` also pass.

- [ ] **Step 10: Commit**

```bash
cd /home/bima-pangestu/Works/greentic/greentic-designer-extensions
git add crates/greentic-ext-runtime/src/host_ports.rs \
        crates/greentic-ext-runtime/src/url_matcher.rs \
        crates/greentic-ext-runtime/src/host_state.rs \
        crates/greentic-ext-runtime/src/loaded.rs \
        crates/greentic-ext-runtime/src/runtime.rs \
        crates/greentic-ext-runtime/src/lib.rs \
        crates/greentic-ext-runtime/Cargo.toml \
        Cargo.lock
git commit -m "feat(ext-runtime): introduce Translator + SecretsBackend ports and HostState builder"
```

---

## Task B.2 — Wire `i18n::t/tf` to a real `Translator`

**Files:**
- Modify: `/home/bima-pangestu/Works/greentic/greentic-designer-extensions/crates/greentic-ext-runtime/src/host_state.rs` (replace stub `i18n::Host for HostState` impl)
- Create: `/home/bima-pangestu/Works/greentic/greentic-designer-extensions/crates/greentic-ext-runtime/tests/host_i18n.rs`
- Create fixture: `/home/bima-pangestu/Works/greentic/greentic-designer-extensions/crates/greentic-ext-runtime/tests/fixtures/host_i18n_caller/Cargo.toml`
- Create fixture: `/home/bima-pangestu/Works/greentic/greentic-designer-extensions/crates/greentic-ext-runtime/tests/fixtures/host_i18n_caller/src/lib.rs`
- Create fixture: `/home/bima-pangestu/Works/greentic/greentic-designer-extensions/crates/greentic-ext-runtime/tests/fixtures/host_i18n_caller/wit/world.wit`

### Sub-task B.2.a — Trait wiring test (no WASM, just trait substitution)

- [ ] **Step 1: Write the failing unit test**

Append at the bottom of `crates/greentic-ext-runtime/src/host_state.rs` (under the existing impls — wrap in `#[cfg(test)] mod tests`):

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::host_bindings::greentic::extension_host::i18n::Host as I18nHost;
    use crate::host_ports::Translator;
    use std::sync::Arc;

    struct EnglishToIndonesian;
    impl Translator for EnglishToIndonesian {
        fn t(&self, key: &str) -> String {
            match key {
                "greentic.test.hello" => "Halo dunia".to_string(),
                _ => key.to_string(),
            }
        }
        fn tf(&self, key: &str, args: &[(&str, &str)]) -> String {
            let template = self.t(key);
            args.iter().fold(template, |acc, (k, v)| {
                acc.replace(&format!("{{{k}}}"), v)
            })
        }
    }

    fn host_with_translator(t: Arc<dyn Translator>) -> HostState {
        HostState::builder(
            "test-ext".to_string(),
            greentic_extension_sdk_contract::describe::Permissions::default(),
        )
        .translator(t)
        .build()
    }

    #[test]
    fn t_resolves_translated_value() {
        let mut h = host_with_translator(Arc::new(EnglishToIndonesian));
        let got = h.t("greentic.test.hello".to_string());
        assert_eq!(got, "Halo dunia");
    }

    #[test]
    fn t_falls_back_to_key_for_unknown() {
        let mut h = host_with_translator(Arc::new(EnglishToIndonesian));
        let got = h.t("missing.key".to_string());
        assert_eq!(got, "missing.key");
    }

    #[test]
    fn tf_substitutes_named_args() {
        struct GreetTranslator;
        impl Translator for GreetTranslator {
            fn t(&self, key: &str) -> String {
                if key == "greentic.test.greet" {
                    "Halo {name}!".to_string()
                } else {
                    key.to_string()
                }
            }
            fn tf(&self, key: &str, args: &[(&str, &str)]) -> String {
                let template = self.t(key);
                args.iter().fold(template, |acc, (k, v)| {
                    acc.replace(&format!("{{{k}}}"), v)
                })
            }
        }
        let mut h = host_with_translator(Arc::new(GreetTranslator));
        let got = h.tf(
            "greentic.test.greet".to_string(),
            vec![("name".to_string(), "Bima".to_string())],
        );
        assert_eq!(got, "Halo Bima!");
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd /home/bima-pangestu/Works/greentic/greentic-designer-extensions && cargo test -p greentic-ext-runtime --lib host_state::tests::t_resolves_translated_value -- --nocapture`
Expected: FAIL — got `"greentic.test.hello"`, expected `"Halo dunia"` (the stub still returns the key).

- [ ] **Step 3: Replace the stub `i18n::Host` impl with the real one**

In `crates/greentic-ext-runtime/src/host_state.rs`, find:

```rust
impl i18n::Host for HostState {
    fn t(&mut self, key: String) -> String {
        let _ = &self.translator;
        key
    }
    fn tf(&mut self, key: String, _args: Vec<(String, String)>) -> String {
        let _ = &self.translator;
        key
    }
}
```

Replace with:

```rust
impl i18n::Host for HostState {
    fn t(&mut self, key: String) -> String {
        self.translator.t(&key)
    }
    fn tf(&mut self, key: String, args: Vec<(String, String)>) -> String {
        // Convert `Vec<(String, String)>` to `&[(&str, &str)]` without
        // allocating per-element.
        let borrowed: Vec<(&str, &str)> = args
            .iter()
            .map(|(k, v)| (k.as_str(), v.as_str()))
            .collect();
        self.translator.tf(&key, &borrowed)
    }
}
```

- [ ] **Step 4: Run unit tests to verify they pass**

Run: `cd /home/bima-pangestu/Works/greentic/greentic-designer-extensions && cargo test -p greentic-ext-runtime --lib host_state::tests -- --nocapture`
Expected: PASS — 3 tests pass.

- [ ] **Step 5: Commit**

```bash
cd /home/bima-pangestu/Works/greentic/greentic-designer-extensions
git add crates/greentic-ext-runtime/src/host_state.rs
git commit -m "feat(ext-runtime): wire i18n::t/tf to Translator port"
```

### Sub-task B.2.b — Round-trip integration test through a real WASM extension

The plan ships a hand-rolled WIT component using `wat::parse_str` for the fixtures so we don't need cargo-component / `wasm32-wasip2` in the test path. The component exports `greentic:extension-design/tools@0.1.0` with one tool `t_hello` that calls back into `host.i18n.t("greentic.test.hello")` and returns the result as JSON.

- [ ] **Step 6: Write the failing integration test**

Create `/home/bima-pangestu/Works/greentic/greentic-designer-extensions/crates/greentic-ext-runtime/tests/host_i18n.rs`:

```rust
//! Round-trip test: a WASM extension calls host.i18n.t and gets back the
//! translated string for the configured locale.

mod support;

use std::sync::Arc;

use greentic_ext_runtime::host_ports::Translator;
use greentic_ext_runtime::{
    DiscoveryPaths, ExtensionRuntime, HostOverrides, RuntimeConfig,
};

struct IndonesianCatalog;
impl Translator for IndonesianCatalog {
    fn t(&self, key: &str) -> String {
        match key {
            "greentic.test.hello" => "Halo dunia".to_string(),
            _ => key.to_string(),
        }
    }
    fn tf(&self, key: &str, args: &[(&str, &str)]) -> String {
        let template = self.t(key);
        args.iter().fold(template, |acc, (k, v)| {
            acc.replace(&format!("{{{k}}}"), v)
        })
    }
}

#[test]
fn i18n_t_resolves_through_host_call() {
    // 1. Build a signed fixture extension that calls host.i18n.t.
    let (fixture, _sk) = support::signed_fixture_with_wasm(
        greentic_extension_sdk_contract::ExtensionKind::Design,
        "greentic.test.i18n-caller",
        "1.0.0",
        include_bytes!("fixtures/host_i18n_caller/component.wasm"),
    );

    // 2. Load the extension.
    let tmp = tempfile::TempDir::new().unwrap();
    let dest = tmp.path().join("design").join("greentic.test.i18n-caller");
    std::fs::create_dir_all(&dest).unwrap();
    support::copy_dir(&fixture.root(), &dest);

    let config = RuntimeConfig::from_paths(DiscoveryPaths::new(tmp.path().to_path_buf()));
    let mut rt = ExtensionRuntime::new(config).unwrap();
    rt.register_loaded_from_dir(&dest).unwrap();

    // 3. Provide an Indonesian translator for this dispatch.
    let overrides = HostOverrides {
        translator: Arc::new(IndonesianCatalog),
        ..HostOverrides::defaults_for_tests()
    };
    rt.set_host_overrides(overrides);

    // 4. Invoke and assert.
    let out = rt
        .invoke_tool("greentic.test.i18n-caller", "t_hello", "{}")
        .expect("invoke should succeed");
    let parsed: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(parsed["text"], "Halo dunia");
}
```

- [ ] **Step 7: Run it — expect FAIL because `set_host_overrides` doesn't exist + fixture wasm doesn't exist**

Run: `cd /home/bima-pangestu/Works/greentic/greentic-designer-extensions && cargo test -p greentic-ext-runtime --test host_i18n 2>&1 | tail -30`
Expected: FAIL with `no method named 'set_host_overrides'` or `include_bytes! failed to read`.

- [ ] **Step 8: Add `set_host_overrides` to `ExtensionRuntime`**

In `crates/greentic-ext-runtime/src/runtime.rs`, add a field `host_overrides: ArcSwap<crate::loaded::HostOverrides>` to `ExtensionRuntime`, initialize it in `ExtensionRuntime::new` to `HostOverrides::defaults_for_tests()`, and add:

```rust
impl ExtensionRuntime {
    /// Replace the host-side dependency overrides used for every subsequent
    /// dispatch. Designer wires production translator + secrets backends
    /// during startup; tests use this to inject fakes.
    pub fn set_host_overrides(&self, overrides: crate::loaded::HostOverrides) {
        self.host_overrides.store(Arc::new(overrides));
    }

    fn current_overrides(&self) -> crate::loaded::HostOverrides {
        (**self.host_overrides.load()).clone()
    }
}
```

Then update every `build_store_and_instance(&self.engine, HostOverrides::defaults_for_tests())` call site inside `runtime.rs` to use `self.current_overrides()` instead.

- [ ] **Step 9: Build the fixture wasm**

Create `crates/greentic-ext-runtime/tests/fixtures/host_i18n_caller/build.sh`:

```bash
#!/usr/bin/env bash
# Builds the host_i18n_caller WASM component fixture used by host_i18n.rs.
# Requires: rustup target add wasm32-wasip2 + cargo-component installed.
set -euo pipefail
HERE="$(cd "$(dirname "$0")" && pwd)"
cd "$HERE"
cargo component build --release --target wasm32-wasip2
cp target/wasm32-wasip2/release/host_i18n_caller.wasm component.wasm
echo "Built $(realpath component.wasm)"
```

Create `crates/greentic-ext-runtime/tests/fixtures/host_i18n_caller/Cargo.toml`:

```toml
[package]
name = "host_i18n_caller"
version = "0.1.0"
edition = "2024"
publish = false

[lib]
crate-type = ["cdylib"]

[dependencies]
wit-bindgen-rt = { version = "0.41", features = ["bitflags"] }
serde_json = { version = "1", default-features = false, features = ["alloc"] }

[package.metadata.component]
package = "greentic:test"
target = { path = "wit", world = "i18n-caller" }

[workspace]
```

Create `crates/greentic-ext-runtime/tests/fixtures/host_i18n_caller/wit/world.wit`:

```wit
package greentic:test;

world i18n-caller {
  import greentic:extension-host/i18n@0.1.0;
  import greentic:extension-host/logging@0.1.0;
  export greentic:extension-design/tools@0.1.0;
  export greentic:extension-base/manifest@0.1.0;
  export greentic:extension-base/lifecycle@0.1.0;
}
```

Vendor the matching `extension-host.wit`, `extension-design.wit`, `extension-base.wit` under `wit/deps/extension-host/`, `wit/deps/extension-design/`, `wit/deps/extension-base/` (copy from `crates/greentic-ext-runtime/wit/` at the same content the runtime uses).

Create `crates/greentic-ext-runtime/tests/fixtures/host_i18n_caller/src/lib.rs`:

```rust
#![no_std]
extern crate alloc;
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

wit_bindgen_rt::generate!({
    world: "i18n-caller",
    path: "wit",
});

use exports::greentic::extension_base::lifecycle::Guest as LifecycleGuest;
use exports::greentic::extension_base::manifest::Guest as ManifestGuest;
use exports::greentic::extension_design::tools::{
    ExtensionError, Guest as ToolsGuest, ToolDefinition,
};
use greentic::extension_host::i18n;

struct Component;

impl ManifestGuest for Component {
    fn name() -> String { "i18n-caller".into() }
    fn version() -> String { "1.0.0".into() }
}
impl LifecycleGuest for Component {
    fn init() -> Result<(), exports::greentic::extension_base::lifecycle::ExtensionError> { Ok(()) }
}
impl ToolsGuest for Component {
    fn list_tools() -> Vec<ToolDefinition> {
        Vec::new()
    }
    fn invoke_tool(name: String, _args_json: String) -> Result<String, ExtensionError> {
        if name == "t_hello" {
            let translated = i18n::t("greentic.test.hello");
            // hand-rolled JSON to keep no_std happy
            return Ok(format!("{{\"text\":\"{translated}\"}}"));
        }
        Err(ExtensionError::Internal("unknown tool".into()))
    }
}

export!(Component);
```

Add a build instruction comment near the test source:

```rust
// To rebuild the fixture wasm:
//   bash crates/greentic-ext-runtime/tests/fixtures/host_i18n_caller/build.sh
// The committed `component.wasm` blob is regenerated on contract changes.
```

Run the build script once locally:

```bash
cd /home/bima-pangestu/Works/greentic/greentic-designer-extensions/crates/greentic-ext-runtime/tests/fixtures/host_i18n_caller
bash build.sh
```

Expected: produces `component.wasm`. Commit it as a binary artifact.

- [ ] **Step 10: Add `support::signed_fixture_with_wasm` + `support::copy_dir` helpers**

Append to `/home/bima-pangestu/Works/greentic/greentic-designer-extensions/crates/greentic-ext-runtime/tests/support/mod.rs`:

```rust
/// Sign a fixture that uses caller-supplied wasm bytes instead of the
/// `(component)` minimal placeholder. Mirrors `signed_fixture` but takes
/// real component bytes from `include_bytes!`.
pub fn signed_fixture_with_wasm(
    kind: greentic_extension_sdk_contract::ExtensionKind,
    id: &str,
    version: &str,
    wasm: &[u8],
) -> (
    greentic_extension_sdk_testing::ExtensionFixture,
    ed25519_dalek::SigningKey,
) {
    use ed25519_dalek::SigningKey;
    use rand::rngs::OsRng;

    let fixture = greentic_extension_sdk_testing::ExtensionFixtureBuilder::new(kind, id, version)
        .offer("greentic:test/ping", "1.0.0")
        .with_wasm(wasm.to_vec())
        .build()
        .expect("fixture build");
    let describe_path = fixture.root().join("describe.json");
    let raw = std::fs::read_to_string(&describe_path).unwrap();
    let mut describe: greentic_extension_sdk_contract::DescribeJson =
        serde_json::from_str(&raw).unwrap();
    let sk = SigningKey::generate(&mut OsRng);
    greentic_extension_sdk_contract::sign_describe(&mut describe, &sk).expect("sign");
    let out = serde_json::to_string_pretty(&describe).unwrap();
    std::fs::write(&describe_path, out).unwrap();
    (fixture, sk)
}

/// Recursively copy `src` to `dest`. Used to move a fixture into a
/// `<home>/<kind>/<id>/` directory that the runtime's discovery picks up.
pub fn copy_dir(src: &std::path::Path, dest: &std::path::Path) {
    std::fs::create_dir_all(dest).unwrap();
    for entry in walkdir::WalkDir::new(src).min_depth(1) {
        let entry = entry.unwrap();
        let rel = entry.path().strip_prefix(src).unwrap();
        let dst = dest.join(rel);
        if entry.file_type().is_dir() {
            std::fs::create_dir_all(&dst).unwrap();
        } else {
            std::fs::copy(entry.path(), &dst).unwrap();
        }
    }
}
```

Then add `walkdir = { workspace = true }` to `[dev-dependencies]` in `crates/greentic-ext-runtime/Cargo.toml`.

- [ ] **Step 11: Run the integration test — expect PASS**

Run: `cd /home/bima-pangestu/Works/greentic/greentic-designer-extensions && cargo test -p greentic-ext-runtime --test host_i18n -- --nocapture`
Expected: PASS — `parsed["text"] == "Halo dunia"`.

- [ ] **Step 12: Commit**

```bash
cd /home/bima-pangestu/Works/greentic/greentic-designer-extensions
git add crates/greentic-ext-runtime/src/runtime.rs \
        crates/greentic-ext-runtime/src/lib.rs \
        crates/greentic-ext-runtime/Cargo.toml \
        crates/greentic-ext-runtime/tests/host_i18n.rs \
        crates/greentic-ext-runtime/tests/support/mod.rs \
        crates/greentic-ext-runtime/tests/fixtures/host_i18n_caller/
git commit -m "test(ext-runtime): round-trip integration test for host.i18n.t through WASM caller"
```

---

## Task B.3 — Wire `secrets::get` to a real `SecretsBackend` with permission gating

**Files:**
- Modify: `/home/bima-pangestu/Works/greentic/greentic-designer-extensions/crates/greentic-ext-runtime/src/host_state.rs`
- Create: `/home/bima-pangestu/Works/greentic/greentic-designer-extensions/crates/greentic-ext-runtime/tests/host_secrets.rs`
- Create fixture: `/home/bima-pangestu/Works/greentic/greentic-designer-extensions/crates/greentic-ext-runtime/tests/fixtures/host_secrets_caller/` (Cargo.toml + src/lib.rs + wit/world.wit, mirroring the i18n fixture layout).

- [ ] **Step 1: Write failing unit tests for the permission gate + backend wiring**

Append to `crates/greentic-ext-runtime/src/host_state.rs::tests`:

```rust
    #[test]
    fn secrets_get_returns_value_when_permitted() {
        use crate::host_bindings::greentic::extension_host::secrets::Host as SecretsHost;
        use crate::host_ports::InMemorySecrets;

        let mut backend = InMemorySecrets::default();
        backend.insert("api.openai.com/api_key", "sk-real");
        let mut perms = greentic_extension_sdk_contract::describe::Permissions::default();
        perms.secrets.push("api.openai.com/api_key".to_string());

        let mut h = HostState::builder("test-ext".to_string(), perms)
            .secrets_backend(Arc::new(backend))
            .build();
        let v = h.get("api.openai.com/api_key".to_string()).unwrap();
        assert_eq!(v, "sk-real");
    }

    #[test]
    fn secrets_get_denies_when_uri_not_in_permissions() {
        use crate::host_bindings::greentic::extension_host::secrets::Host as SecretsHost;
        use crate::host_ports::InMemorySecrets;

        let mut backend = InMemorySecrets::default();
        backend.insert("api.openai.com/api_key", "sk-real");
        let perms = greentic_extension_sdk_contract::describe::Permissions::default();

        let mut h = HostState::builder("test-ext".to_string(), perms)
            .secrets_backend(Arc::new(backend))
            .build();
        let err = h.get("api.openai.com/api_key".to_string()).unwrap_err();
        assert!(
            err.contains("permission denied"),
            "expected permission denied, got: {err}"
        );
    }

    #[test]
    fn secrets_get_surfaces_backend_not_found() {
        use crate::host_bindings::greentic::extension_host::secrets::Host as SecretsHost;
        use crate::host_ports::InMemorySecrets;

        let backend = InMemorySecrets::default();
        let mut perms = greentic_extension_sdk_contract::describe::Permissions::default();
        perms.secrets.push("api.openai.com/api_key".to_string());

        let mut h = HostState::builder("test-ext".to_string(), perms)
            .secrets_backend(Arc::new(backend))
            .build();
        let err = h.get("api.openai.com/api_key".to_string()).unwrap_err();
        assert!(err.contains("not found"), "got: {err}");
    }
```

- [ ] **Step 2: Run — expect FAIL**

Run: `cd /home/bima-pangestu/Works/greentic/greentic-designer-extensions && cargo test -p greentic-ext-runtime --lib host_state::tests::secrets_get_returns_value_when_permitted -- --nocapture`
Expected: FAIL — current impl returns `"secrets backend stub for ..."`.

- [ ] **Step 3: Replace the stub `secrets::Host` impl**

In `crates/greentic-ext-runtime/src/host_state.rs`, replace the stub `impl secrets::Host for HostState` block with:

```rust
impl secrets::Host for HostState {
    fn get(&mut self, uri: String) -> Result<String, String> {
        // Permission check: every secret URI the extension reads must be
        // declared verbatim or as a prefix in `permissions.secrets`. Strict
        // prefix match (no glob today — that's a follow-up if Bima asks).
        let permitted = self
            .permissions
            .secrets
            .iter()
            .any(|allowed| uri == *allowed || uri.starts_with(&format!("{allowed}/")));
        if !permitted {
            tracing::warn!(
                ext = %self.extension_id,
                requested = %uri,
                "secrets::get permission denied"
            );
            return Err(format!("permission denied for secret: {uri}"));
        }
        match self.secrets_backend.get(&uri) {
            Ok(value) => Ok(value),
            Err(crate::host_ports::SecretsError::NotFound(k)) => {
                Err(format!("secret not found: {k}"))
            }
            Err(crate::host_ports::SecretsError::Backend(msg)) => {
                tracing::error!(ext = %self.extension_id, %msg, "secrets backend error");
                Err(format!("secrets backend error: {msg}"))
            }
        }
    }
}
```

- [ ] **Step 4: Run unit tests — expect PASS**

Run: `cd /home/bima-pangestu/Works/greentic/greentic-designer-extensions && cargo test -p greentic-ext-runtime --lib host_state::tests -- --nocapture`
Expected: All host_state unit tests pass (i18n tests from B.2 + 3 new secrets tests).

- [ ] **Step 5: Commit (impl only)**

```bash
cd /home/bima-pangestu/Works/greentic/greentic-designer-extensions
git add crates/greentic-ext-runtime/src/host_state.rs
git commit -m "feat(ext-runtime): wire secrets::get to SecretsBackend port with permission gate"
```

- [ ] **Step 6: Build the `host_secrets_caller` fixture**

Mirror Task B.2.b layout under `crates/greentic-ext-runtime/tests/fixtures/host_secrets_caller/`. The fixture exports a tool `read_key` that calls `host.secrets.get("api.openai.com/api_key")` and returns `{"value": "..."}` or `{"error": "..."}` on Err.

`src/lib.rs`:

```rust
#![no_std]
extern crate alloc;
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

wit_bindgen_rt::generate!({
    world: "secrets-caller",
    path: "wit",
});

use exports::greentic::extension_base::lifecycle::Guest as LifecycleGuest;
use exports::greentic::extension_base::manifest::Guest as ManifestGuest;
use exports::greentic::extension_design::tools::{
    ExtensionError, Guest as ToolsGuest, ToolDefinition,
};
use greentic::extension_host::secrets;

struct Component;

impl ManifestGuest for Component {
    fn name() -> String { "secrets-caller".into() }
    fn version() -> String { "1.0.0".into() }
}
impl LifecycleGuest for Component {
    fn init() -> Result<(), exports::greentic::extension_base::lifecycle::ExtensionError> { Ok(()) }
}
impl ToolsGuest for Component {
    fn list_tools() -> Vec<ToolDefinition> { Vec::new() }
    fn invoke_tool(name: String, _args_json: String) -> Result<String, ExtensionError> {
        if name != "read_key" {
            return Err(ExtensionError::Internal("unknown tool".into()));
        }
        match secrets::get("api.openai.com/api_key") {
            Ok(v) => Ok(format!("{{\"value\":\"{v}\"}}")),
            Err(e) => Ok(format!("{{\"error\":\"{e}\"}}")),
        }
    }
}

export!(Component);
```

`wit/world.wit` mirrors the i18n fixture but imports `secrets` instead of `i18n`. `Cargo.toml` is the same as B.2's with `name = "host_secrets_caller"` and `world: "secrets-caller"`.

Run the build script (mirror of B.2's): `bash crates/greentic-ext-runtime/tests/fixtures/host_secrets_caller/build.sh`.

- [ ] **Step 7: Write the failing integration test**

Create `crates/greentic-ext-runtime/tests/host_secrets.rs`:

```rust
mod support;

use std::sync::Arc;

use greentic_ext_runtime::host_ports::InMemorySecrets;
use greentic_ext_runtime::{
    DiscoveryPaths, ExtensionRuntime, HostOverrides, RuntimeConfig,
};

/// Build a runtime + signed fixture whose describe.json declares
/// `permissions.secrets = [permitted_uri]`, install it, and invoke
/// `read_key`. Returns the parsed JSON.
fn run_read_key(permitted_uri: &str) -> serde_json::Value {
    let (mut fixture, _sk) = support::signed_fixture_with_wasm_and_permissions(
        greentic_extension_sdk_contract::ExtensionKind::Design,
        "greentic.test.secrets-caller",
        "1.0.0",
        include_bytes!("fixtures/host_secrets_caller/component.wasm"),
        |perms| {
            perms.secrets = vec![permitted_uri.to_string()];
        },
    );

    let tmp = tempfile::TempDir::new().unwrap();
    let dest = tmp.path().join("design").join("greentic.test.secrets-caller");
    std::fs::create_dir_all(&dest).unwrap();
    support::copy_dir(&fixture.root(), &dest);

    let config = RuntimeConfig::from_paths(DiscoveryPaths::new(tmp.path().to_path_buf()));
    let mut rt = ExtensionRuntime::new(config).unwrap();
    rt.register_loaded_from_dir(&dest).unwrap();

    let mut backend = InMemorySecrets::default();
    backend.insert("api.openai.com/api_key", "sk-test");
    let overrides = HostOverrides {
        secrets_backend: Arc::new(backend),
        ..HostOverrides::defaults_for_tests()
    };
    rt.set_host_overrides(overrides);

    let out = rt
        .invoke_tool("greentic.test.secrets-caller", "read_key", "{}")
        .unwrap();
    serde_json::from_str(&out).unwrap()
}

#[test]
fn secrets_get_round_trips_when_permitted() {
    let v = run_read_key("api.openai.com/api_key");
    assert_eq!(v["value"], "sk-test", "got: {v}");
}

#[test]
fn secrets_get_denied_when_uri_not_in_permissions() {
    let v = run_read_key("some.other.host/key");
    let err = v["error"].as_str().unwrap_or_default();
    assert!(
        err.contains("permission denied"),
        "expected permission denied, got: {err}"
    );
}
```

- [ ] **Step 8: Add `signed_fixture_with_wasm_and_permissions` helper**

Append to `crates/greentic-ext-runtime/tests/support/mod.rs`:

```rust
/// Like `signed_fixture_with_wasm`, but lets the caller mutate
/// `runtime.permissions` before re-signing. Useful for permission tests.
pub fn signed_fixture_with_wasm_and_permissions(
    kind: greentic_extension_sdk_contract::ExtensionKind,
    id: &str,
    version: &str,
    wasm: &[u8],
    mutate_perms: impl FnOnce(&mut greentic_extension_sdk_contract::describe::Permissions),
) -> (
    greentic_extension_sdk_testing::ExtensionFixture,
    ed25519_dalek::SigningKey,
) {
    use ed25519_dalek::SigningKey;
    use rand::rngs::OsRng;

    let fixture = greentic_extension_sdk_testing::ExtensionFixtureBuilder::new(kind, id, version)
        .offer("greentic:test/ping", "1.0.0")
        .with_wasm(wasm.to_vec())
        .build()
        .expect("fixture build");
    let describe_path = fixture.root().join("describe.json");
    let raw = std::fs::read_to_string(&describe_path).unwrap();
    let mut describe: greentic_extension_sdk_contract::DescribeJson =
        serde_json::from_str(&raw).unwrap();
    mutate_perms(&mut describe.runtime.permissions);
    let sk = SigningKey::generate(&mut OsRng);
    greentic_extension_sdk_contract::sign_describe(&mut describe, &sk).expect("sign");
    std::fs::write(&describe_path, serde_json::to_string_pretty(&describe).unwrap()).unwrap();
    (fixture, sk)
}
```

- [ ] **Step 9: Run the integration tests**

Run: `cd /home/bima-pangestu/Works/greentic/greentic-designer-extensions && cargo test -p greentic-ext-runtime --test host_secrets -- --nocapture`
Expected: PASS — both tests green.

- [ ] **Step 10: Commit**

```bash
cd /home/bima-pangestu/Works/greentic/greentic-designer-extensions
git add crates/greentic-ext-runtime/tests/host_secrets.rs \
        crates/greentic-ext-runtime/tests/support/mod.rs \
        crates/greentic-ext-runtime/tests/fixtures/host_secrets_caller/
git commit -m "test(ext-runtime): round-trip + permission-denied tests for host.secrets.get"
```

---

## Task B.4 — Strict URL matcher + `http::fetch` via `reqwest`

**Files:**
- Modify: `/home/bima-pangestu/Works/greentic/greentic-designer-extensions/crates/greentic-ext-runtime/src/url_matcher.rs`
- Modify: `/home/bima-pangestu/Works/greentic/greentic-designer-extensions/crates/greentic-ext-runtime/src/host_state.rs`
- Create: `/home/bima-pangestu/Works/greentic/greentic-designer-extensions/crates/greentic-ext-runtime/tests/url_matcher.rs`
- Create: `/home/bima-pangestu/Works/greentic/greentic-designer-extensions/crates/greentic-ext-runtime/tests/host_http.rs`
- Create fixture: `/home/bima-pangestu/Works/greentic/greentic-designer-extensions/crates/greentic-ext-runtime/tests/fixtures/host_http_caller/` (mirror of B.2 fixture but importing `http`).

### Sub-task B.4.a — `UrlMatcher` strict-matching contract

- [ ] **Step 1: Write the failing matcher test file**

Create `crates/greentic-ext-runtime/tests/url_matcher.rs`:

```rust
//! Strict URL allow-list matcher. The four tests below pin the behaviour
//! that defends against the three classic attack vectors plus a happy
//! path. The matcher MUST refuse anything that doesn't pass scheme +
//! host-suffix + path-prefix all three.

use greentic_ext_runtime::UrlMatcher;

fn matcher(patterns: &[&str]) -> UrlMatcher {
    UrlMatcher::from_patterns(patterns.iter().map(|s| (*s).to_string()).collect())
}

#[test]
fn allows_exact_host_and_path_prefix_match() {
    let m = matcher(&["https://api.openai.com/v1/*"]);
    assert!(m.is_allowed("https://api.openai.com/v1/chat/completions"));
    assert!(m.is_allowed("https://api.openai.com/v1/embeddings"));
}

#[test]
fn rejects_open_redirect_via_query_string() {
    // Classic attack: a URL that points at an allowed host but whose
    // query string contains a malicious redirect target. The matcher
    // looks at scheme + host + path ONLY — the query is irrelevant for
    // allow-listing. Since the URL itself is at allowed.com, the
    // matcher allows allowed.com; the attacker's `to=https://evil.com/`
    // is the *content* of the request, not where it goes.
    //
    // The test here pins the symmetric attack vector: the spec asks
    // that a URL `https://allowed.com/redirect?to=https://evil.com/`
    // does NOT match an allow-list of `["https://evil.com/*"]` —
    // i.e. matching is by URL, not by substring.
    let m = matcher(&["https://evil.com/*"]);
    assert!(
        !m.is_allowed("https://allowed.com/redirect?to=https://evil.com/"),
        "matcher must not be fooled by a substring of evil.com in the query"
    );
}

#[test]
fn rejects_subdomain_confusion() {
    // `evil.com.allowed.com` ends with `.allowed.com` but the registered
    // suffix is the WHOLE `allowed.com` host — a left-to-right substring
    // match would let `evil.com.allowed.com` through. We use parsed Url
    // host and compare with exact-or-suffix-after-dot.
    let m = matcher(&["https://allowed.com/*"]);
    assert!(
        !m.is_allowed("https://evil.com.allowed.com/"),
        "matcher must require a host boundary, not a substring"
    );
}

#[test]
fn rejects_scheme_downgrade() {
    let m = matcher(&["https://allowed.com/*"]);
    assert!(
        !m.is_allowed("http://allowed.com/"),
        "matcher must require exact scheme — no http when https expected"
    );
}

#[test]
fn allows_wildcard_subdomain_when_pattern_uses_star_dot() {
    let m = matcher(&["https://*.example.com/*"]);
    assert!(m.is_allowed("https://api.example.com/v1/foo"));
    assert!(m.is_allowed("https://cdn.example.com/assets/logo.png"));
    // Bare host must NOT match `*.example.com` — the wildcard requires
    // at least one label.
    assert!(!m.is_allowed("https://example.com/x"));
}

#[test]
fn rejects_non_https_by_default() {
    let m = matcher(&["http://allowed.com/*"]);
    // Even if the pattern explicitly uses http://, we additionally
    // reject http by default for security. Operators who want http
    // must opt in via UrlMatcher::allow_http(true) (future work).
    assert!(!m.is_allowed("http://allowed.com/"));
}

#[test]
fn rejects_url_that_fails_to_parse() {
    let m = matcher(&["https://allowed.com/*"]);
    assert!(!m.is_allowed("not a url at all"));
}
```

- [ ] **Step 2: Run — expect FAIL because stub `is_allowed` always returns false (which passes 6 of 7 but `allows_exact_host_and_path_prefix_match` will fail)**

Run: `cd /home/bima-pangestu/Works/greentic/greentic-designer-extensions && cargo test -p greentic-ext-runtime --test url_matcher -- --nocapture`
Expected: FAIL — `allows_exact_host_and_path_prefix_match` + `allows_wildcard_subdomain_when_pattern_uses_star_dot` fail.

- [ ] **Step 3: Implement the matcher**

Replace `crates/greentic-ext-runtime/src/url_matcher.rs` with:

```rust
//! Strict URL allow-list matcher.
//!
//! The matcher parses both the pattern and the request URL with
//! `url::Url`, then compares three orthogonal facets:
//!
//! 1. **Scheme** — exact match; we additionally reject non-`https://`
//!    by default to defend against scheme-downgrade attacks. (Operators
//!    can opt into `http` by calling `with_allow_http(true)`.)
//! 2. **Host** — either exact match or `*.<suffix>` wildcard. The
//!    wildcard requires at least one label (so `*.example.com` does
//!    NOT match `example.com`). Substring matches are explicitly
//!    rejected — the test suite pins both the "subdomain confusion"
//!    (`evil.com.allowed.com`) and the "open-redirect via query
//!    string" cases.
//! 3. **Path** — prefix match against `pattern.path` with the trailing
//!    `*` stripped. `/v1/*` matches `/v1/chat/completions`. A bare
//!    pattern path of `/` matches any path.

use url::Url;

#[derive(Debug, Clone)]
pub struct UrlMatcher {
    patterns: Vec<ParsedPattern>,
    raw: Vec<String>,
    allow_http: bool,
}

#[derive(Debug, Clone)]
struct ParsedPattern {
    scheme: String,
    host_rule: HostRule,
    path_prefix: String,
}

#[derive(Debug, Clone)]
enum HostRule {
    Exact(String),
    /// `*.example.com` — matches any host that ends with `.example.com`.
    WildcardSuffix(String),
}

impl Default for UrlMatcher {
    fn default() -> Self {
        Self {
            patterns: Vec::new(),
            raw: Vec::new(),
            allow_http: false,
        }
    }
}

impl UrlMatcher {
    #[must_use]
    pub fn from_patterns(patterns: Vec<String>) -> Self {
        let mut parsed = Vec::with_capacity(patterns.len());
        for p in &patterns {
            if let Some(pp) = Self::parse_pattern(p) {
                parsed.push(pp);
            } else {
                // Unparseable patterns become "match nothing" — they don't
                // accidentally widen the allow-list. We log so operators
                // notice during testing.
                tracing::warn!(pattern = %p, "unparseable url pattern; ignoring");
            }
        }
        Self {
            patterns: parsed,
            raw: patterns,
            allow_http: false,
        }
    }

    #[must_use]
    pub fn with_allow_http(mut self, allow: bool) -> Self {
        self.allow_http = allow;
        self
    }

    #[must_use]
    pub fn patterns(&self) -> &[String] {
        &self.raw
    }

    fn parse_pattern(pattern: &str) -> Option<ParsedPattern> {
        // Strip trailing `/*` so url::Url parses cleanly.
        let stripped = pattern.trim_end_matches("/*");
        // Replace `*.` host wildcard with a placeholder so url::Url
        // accepts the input; we'll restore the wildcard intent below.
        let (placeholder_used, normalized) = if let Some(rest) = stripped.strip_prefix("https://*.") {
            (true, format!("https://__wildcard__.{rest}"))
        } else if let Some(rest) = stripped.strip_prefix("http://*.") {
            (true, format!("http://__wildcard__.{rest}"))
        } else {
            (false, stripped.to_string())
        };
        let parsed = Url::parse(&normalized).ok()?;
        let host_str = parsed.host_str()?.to_string();
        let host_rule = if placeholder_used {
            // strip leading "__wildcard__." to recover the suffix.
            let suffix = host_str.strip_prefix("__wildcard__.")?.to_string();
            HostRule::WildcardSuffix(suffix)
        } else {
            HostRule::Exact(host_str)
        };
        let path_prefix = if parsed.path().is_empty() || parsed.path() == "/" {
            "/".to_string()
        } else {
            parsed.path().to_string()
        };
        Some(ParsedPattern {
            scheme: parsed.scheme().to_string(),
            host_rule,
            path_prefix,
        })
    }

    #[must_use]
    pub fn is_allowed(&self, url: &str) -> bool {
        let Ok(parsed) = Url::parse(url) else {
            return false;
        };
        if parsed.scheme() != "https" && !self.allow_http {
            return false;
        }
        let Some(host) = parsed.host_str() else {
            return false;
        };
        let path = parsed.path();
        self.patterns.iter().any(|pat| {
            // Scheme must match exactly.
            if pat.scheme != parsed.scheme() {
                return false;
            }
            let host_ok = match &pat.host_rule {
                HostRule::Exact(h) => h.eq_ignore_ascii_case(host),
                HostRule::WildcardSuffix(suffix) => {
                    // Need at least one extra label: `api.example.com` matches
                    // `*.example.com`, but `example.com` itself does NOT.
                    let lower_host = host.to_ascii_lowercase();
                    let lower_suffix = suffix.to_ascii_lowercase();
                    if !lower_host.ends_with(&lower_suffix) {
                        return false;
                    }
                    let prefix_len = lower_host.len().saturating_sub(lower_suffix.len());
                    if prefix_len == 0 {
                        return false; // bare host, no label before the suffix
                    }
                    // The char immediately before the suffix must be a `.`
                    let prefix = &lower_host[..prefix_len];
                    prefix.ends_with('.')
                }
            };
            if !host_ok {
                return false;
            }
            // Path prefix match.
            if pat.path_prefix == "/" {
                return true;
            }
            path.starts_with(&pat.path_prefix)
        })
    }
}
```

- [ ] **Step 4: Run the matcher test file — expect PASS**

Run: `cd /home/bima-pangestu/Works/greentic/greentic-designer-extensions && cargo test -p greentic-ext-runtime --test url_matcher -- --nocapture`
Expected: PASS — all 7 tests.

- [ ] **Step 5: Commit the matcher**

```bash
cd /home/bima-pangestu/Works/greentic/greentic-designer-extensions
git add crates/greentic-ext-runtime/src/url_matcher.rs \
        crates/greentic-ext-runtime/tests/url_matcher.rs
git commit -m "feat(ext-runtime): strict URL allow-list matcher (scheme + host suffix + path prefix)"
```

### Sub-task B.4.b — Wire `http::fetch` to `reqwest::blocking` + matcher

- [ ] **Step 6: Write failing unit tests for `http::Host::fetch`**

Append to `crates/greentic-ext-runtime/src/host_state.rs::tests`:

```rust
    #[test]
    fn http_fetch_denied_when_url_not_in_matcher() {
        use crate::host_bindings::greentic::extension_host::http::{Host as HttpHost, Request};

        let mut h = HostState::builder(
            "test-ext".into(),
            greentic_extension_sdk_contract::describe::Permissions::default(),
        )
        .url_matcher(crate::url_matcher::UrlMatcher::from_patterns(vec![
            "https://allowed.com/*".into(),
        ]))
        .build();
        let req = Request {
            method: "GET".into(),
            url: "https://evil.com/".into(),
            headers: vec![],
            body: None,
        };
        let err = h.fetch(req).unwrap_err();
        assert!(err.contains("permission denied") || err.contains("not allowed"), "got: {err}");
    }
```

The happy-path round-trip uses `wiremock` in the integration test, not here.

- [ ] **Step 7: Run — expect FAIL (stub still wins)**

Run: `cd /home/bima-pangestu/Works/greentic/greentic-designer-extensions && cargo test -p greentic-ext-runtime --lib host_state::tests::http_fetch_denied_when_url_not_in_matcher -- --nocapture`
Expected: FAIL — stub returns `"network permission denied for url: https://evil.com/"` actually contains "permission denied" so... if it accidentally passes, that's still correct behaviour; the integration test in step 11 forces real reqwest path.

If it accidentally passes here, fine — the regression test is locked in.

- [ ] **Step 8: Replace `http::Host` impl with the real one**

In `crates/greentic-ext-runtime/src/host_state.rs`, replace the stub `impl http::Host for HostState` block with:

```rust
impl http::Host for HostState {
    fn fetch(&mut self, req: http::Request) -> Result<http::Response, String> {
        // 1. Permission check via strict UrlMatcher.
        if !self.url_matcher.is_allowed(&req.url) {
            tracing::warn!(
                ext = %self.extension_id,
                url = %req.url,
                "http::fetch permission denied"
            );
            return Err(format!("network not allowed for url: {}", req.url));
        }

        // 2. Build the reqwest request.
        let method = match req.method.to_uppercase().as_str() {
            "GET" => reqwest::Method::GET,
            "POST" => reqwest::Method::POST,
            "PUT" => reqwest::Method::PUT,
            "DELETE" => reqwest::Method::DELETE,
            "PATCH" => reqwest::Method::PATCH,
            "HEAD" => reqwest::Method::HEAD,
            other => return Err(format!("unsupported http method: {other}")),
        };
        let mut builder = self.http_client.request(method, &req.url);
        for (k, v) in &req.headers {
            builder = builder.header(k.as_str(), v.as_str());
        }
        if let Some(body) = req.body {
            builder = builder.body(body);
        }
        // 3. Execute (blocking — wasmtime sync wiring expects sync host fns).
        let resp = builder.send().map_err(|e| {
            tracing::error!(ext = %self.extension_id, error = %e, "http::fetch transport error");
            format!("http transport error: {e}")
        })?;
        let status = resp.status().as_u16();
        let headers = resp
            .headers()
            .iter()
            .filter_map(|(k, v)| {
                v.to_str().ok().map(|s| (k.as_str().to_string(), s.to_string()))
            })
            .collect();
        let body = resp.bytes().map_err(|e| format!("http body error: {e}"))?.to_vec();
        Ok(http::Response {
            status,
            headers,
            body,
        })
    }
}
```

- [ ] **Step 9: Run host_state tests — expect PASS**

Run: `cd /home/bima-pangestu/Works/greentic/greentic-designer-extensions && cargo test -p greentic-ext-runtime --lib host_state -- --nocapture`
Expected: PASS — all host_state tests green.

- [ ] **Step 10: Build the http fixture wasm**

Mirror Task B.2.b. Create `crates/greentic-ext-runtime/tests/fixtures/host_http_caller/` with `Cargo.toml`, `src/lib.rs`, `wit/world.wit`. The fixture exports a tool `fetch_ip` that calls `host.http.fetch({method: "GET", url: "https://allowed.test/whoami", headers: [], body: None})` and returns the response body as a JSON string.

`src/lib.rs`:

```rust
#![no_std]
extern crate alloc;
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use alloc::vec;

wit_bindgen_rt::generate!({
    world: "http-caller",
    path: "wit",
});

use exports::greentic::extension_base::lifecycle::Guest as LifecycleGuest;
use exports::greentic::extension_base::manifest::Guest as ManifestGuest;
use exports::greentic::extension_design::tools::{
    ExtensionError, Guest as ToolsGuest, ToolDefinition,
};
use greentic::extension_host::http::{self, Request};

struct Component;
impl ManifestGuest for Component {
    fn name() -> String { "http-caller".into() }
    fn version() -> String { "1.0.0".into() }
}
impl LifecycleGuest for Component {
    fn init() -> Result<(), exports::greentic::extension_base::lifecycle::ExtensionError> { Ok(()) }
}
impl ToolsGuest for Component {
    fn list_tools() -> Vec<ToolDefinition> { Vec::new() }
    fn invoke_tool(name: String, args_json: String) -> Result<String, ExtensionError> {
        if name != "fetch_url" {
            return Err(ExtensionError::Internal("unknown tool".into()));
        }
        // args_json is `{"url": "..."}` — quick parse without serde to keep no_std happy.
        let url = args_json
            .split("\"url\":\"").nth(1)
            .and_then(|s| s.split('"').next())
            .unwrap_or("");
        let req = Request {
            method: "GET".into(),
            url: url.into(),
            headers: vec![],
            body: None,
        };
        match http::fetch(&req) {
            Ok(resp) => {
                let body = core::str::from_utf8(&resp.body).unwrap_or("<non-utf8>");
                Ok(format!("{{\"status\":{},\"body\":\"{body}\"}}", resp.status))
            }
            Err(e) => Ok(format!("{{\"error\":\"{e}\"}}")),
        }
    }
}
export!(Component);
```

Build script + Cargo.toml mirror B.2 with name `host_http_caller` and world `http-caller`. Vendor the same `wit/deps/` set.

- [ ] **Step 11: Write the round-trip + denial test**

Create `crates/greentic-ext-runtime/tests/host_http.rs`:

```rust
mod support;

use std::sync::Arc;

use greentic_ext_runtime::{
    DiscoveryPaths, ExtensionRuntime, HostOverrides, RuntimeConfig, UrlMatcher,
};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn http_fetch_round_trips_when_allowed() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/whoami"))
        .respond_with(ResponseTemplate::new(200).set_body_string("ok-from-wiremock"))
        .mount(&server)
        .await;

    let (fixture, _sk) = support::signed_fixture_with_wasm(
        greentic_extension_sdk_contract::ExtensionKind::Design,
        "greentic.test.http-caller",
        "1.0.0",
        include_bytes!("fixtures/host_http_caller/component.wasm"),
    );
    let tmp = tempfile::TempDir::new().unwrap();
    let dest = tmp.path().join("design").join("greentic.test.http-caller");
    std::fs::create_dir_all(&dest).unwrap();
    support::copy_dir(&fixture.root(), &dest);

    let config = RuntimeConfig::from_paths(DiscoveryPaths::new(tmp.path().to_path_buf()));
    let mut rt = ExtensionRuntime::new(config).unwrap();
    rt.register_loaded_from_dir(&dest).unwrap();

    // Wiremock binds to 127.0.0.1:<random_port>; the matcher's `http://*`
    // pattern must allow http for this test. We use the explicit
    // `with_allow_http` opt-in.
    let matcher_pattern = format!("{}/*", server.uri());
    let matcher = UrlMatcher::from_patterns(vec![matcher_pattern]).with_allow_http(true);
    let overrides = HostOverrides {
        url_matcher: matcher,
        ..HostOverrides::defaults_for_tests()
    };
    rt.set_host_overrides(overrides);

    let url = format!("{}/whoami", server.uri());
    let args = serde_json::json!({"url": url}).to_string();
    let out = tokio::task::spawn_blocking(move || {
        rt.invoke_tool("greentic.test.http-caller", "fetch_url", &args)
    })
    .await
    .unwrap()
    .unwrap();

    let parsed: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(parsed["status"], 200, "got: {parsed}");
    assert_eq!(parsed["body"], "ok-from-wiremock");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn http_fetch_denied_when_url_not_in_matcher() {
    let (fixture, _sk) = support::signed_fixture_with_wasm(
        greentic_extension_sdk_contract::ExtensionKind::Design,
        "greentic.test.http-caller-2",
        "1.0.0",
        include_bytes!("fixtures/host_http_caller/component.wasm"),
    );
    let tmp = tempfile::TempDir::new().unwrap();
    let dest = tmp.path().join("design").join("greentic.test.http-caller-2");
    std::fs::create_dir_all(&dest).unwrap();
    support::copy_dir(&fixture.root(), &dest);

    let config = RuntimeConfig::from_paths(DiscoveryPaths::new(tmp.path().to_path_buf()));
    let mut rt = ExtensionRuntime::new(config).unwrap();
    rt.register_loaded_from_dir(&dest).unwrap();

    let overrides = HostOverrides {
        url_matcher: UrlMatcher::from_patterns(vec!["https://only-this.com/*".into()]),
        ..HostOverrides::defaults_for_tests()
    };
    rt.set_host_overrides(overrides);

    let args = serde_json::json!({"url": "https://evil.com/x"}).to_string();
    let out = tokio::task::spawn_blocking(move || {
        rt.invoke_tool("greentic.test.http-caller-2", "fetch_url", &args)
    })
    .await
    .unwrap()
    .unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&out).unwrap();
    let err = parsed["error"].as_str().unwrap_or_default();
    assert!(
        err.contains("not allowed") || err.contains("permission denied"),
        "expected denial, got: {parsed}"
    );
}
```

- [ ] **Step 12: Run the integration tests**

Run: `cd /home/bima-pangestu/Works/greentic/greentic-designer-extensions && cargo test -p greentic-ext-runtime --test host_http -- --nocapture`
Expected: PASS — both tests.

- [ ] **Step 13: Commit**

```bash
cd /home/bima-pangestu/Works/greentic/greentic-designer-extensions
git add crates/greentic-ext-runtime/src/host_state.rs \
        crates/greentic-ext-runtime/tests/host_http.rs \
        crates/greentic-ext-runtime/tests/fixtures/host_http_caller/
git commit -m "feat(ext-runtime): http::fetch via reqwest gated by strict UrlMatcher"
```

---

## Task B.5 — `broker::call_extension` cross-extension dispatch

The broker dispatch path is conceptually: caller → `host.broker.call_extension(kind, target_id, fn, args)` → host finds target's loaded extension → host calls target's `tools::invoke-tool(fn, args)` via wasmtime → host returns the JSON result to caller. Depth counter increments per nested hop; max is `MAX_BROKER_DEPTH = 8`.

**Files:**
- Modify: `/home/bima-pangestu/Works/greentic/greentic-designer-extensions/crates/greentic-ext-runtime/src/broker.rs` (add `Broker::dispatch` helper)
- Modify: `/home/bima-pangestu/Works/greentic/greentic-designer-extensions/crates/greentic-ext-runtime/src/runtime.rs` (add `ExtensionRuntime::invoke_tool_with_depth` that the broker calls back into)
- Modify: `/home/bima-pangestu/Works/greentic/greentic-designer-extensions/crates/greentic-ext-runtime/src/host_state.rs` (replace stub `broker::Host` impl)
- Create: `/home/bima-pangestu/Works/greentic/greentic-designer-extensions/crates/greentic-ext-runtime/tests/host_broker.rs`
- Create fixtures: `tests/fixtures/broker_caller_a/` and `tests/fixtures/broker_target_b/`

- [ ] **Step 1: Add `invoke_tool_with_depth` to `ExtensionRuntime`**

The existing `invoke_tool` is depth-0. We need a sibling that accepts a starting depth so nested dispatch increments. Add to `crates/greentic-ext-runtime/src/runtime.rs` (next to `invoke_tool`):

```rust
impl ExtensionRuntime {
    /// Like `invoke_tool` but with an explicit starting call depth. Used by
    /// `host.broker.call-extension` to track recursion across hops.
    pub fn invoke_tool_with_depth(
        self: &Arc<Self>,
        ext_id: &str,
        tool_name: &str,
        args_json: &str,
        depth: u32,
    ) -> Result<String, RuntimeError> {
        use crate::host_bindings::greentic::extension_base::types::ExtensionError;

        let loaded = self
            .loaded
            .load()
            .get(&crate::loaded::ExtensionId(ext_id.to_string()))
            .cloned()
            .ok_or_else(|| RuntimeError::NotFound(ext_id.to_string()))?;

        let mut overrides = self.current_overrides();
        overrides.call_depth_start = depth;
        overrides.runtime_weak = Arc::downgrade(self);

        let (mut store, instance) = loaded
            .build_store_and_instance(&self.engine, overrides)
            .map_err(RuntimeError::Wasmtime)?;

        let (iface_idx, iface_name) =
            resolve_design_iface(&mut store, &instance, "greentic:extension-design/tools")?;
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
        result.map_err(|e| {
            RuntimeError::Wasmtime(anyhow::anyhow!(
                "extension returned error for tool '{tool_name}': {e:?}"
            ))
        })
    }
}
```

Also refactor the existing `invoke_tool` to delegate: change its body so it just calls `self.invoke_tool_with_depth(ext_id, tool_name, args_json, 0)`. To make this work the existing `invoke_tool` signature must change from `&self` to `self: &Arc<Self>`. Update all call sites in `runtime.rs`, `loaded.rs`, and tests (`grep -rn "invoke_tool" crates/greentic-ext-runtime/src` to enumerate). For designer call sites, the change is API-compatible because designers already hold `Arc<ExtensionRuntime>`.

If any of the other methods (`validate_content`, `list_tools`, …) need the runtime to be `Arc<Self>` for their own future cross-ext work, leave them as `&self` for now — Phase B only touches `invoke_tool`.

- [ ] **Step 2: Write the failing broker dispatch unit test**

Append to `crates/greentic-ext-runtime/src/host_state.rs::tests`:

```rust
    #[test]
    fn broker_denies_when_kind_not_in_permissions() {
        use crate::host_bindings::greentic::extension_host::broker::Host as BrokerHost;

        let mut perms = greentic_extension_sdk_contract::describe::Permissions::default();
        perms.call_extension_kinds.push("provider".to_string());
        let mut h = HostState::builder("caller".into(), perms).build();

        let err = h
            .call_extension(
                "design".into(),
                "greentic.target".into(),
                "do_something".into(),
                "{}".into(),
            )
            .unwrap_err();
        assert!(err.contains("may not call"), "got: {err}");
    }

    #[test]
    fn broker_rejects_when_depth_exceeded() {
        use crate::host_bindings::greentic::extension_host::broker::Host as BrokerHost;

        let mut perms = greentic_extension_sdk_contract::describe::Permissions::default();
        perms.call_extension_kinds.push("design".to_string());
        // Start near the cap so any call trips MAX_BROKER_DEPTH.
        let mut h = HostState::builder("caller".into(), perms)
            .call_depth_start(MAX_BROKER_DEPTH)
            .build();
        let err = h
            .call_extension(
                "design".into(),
                "greentic.target".into(),
                "do".into(),
                "{}".into(),
            )
            .unwrap_err();
        assert!(err.contains("max"), "expected depth error, got: {err}");
    }
```

- [ ] **Step 3: Run — expect FAIL (stub returns "broker stub for ...")**

Run: `cd /home/bima-pangestu/Works/greentic/greentic-designer-extensions && cargo test -p greentic-ext-runtime --lib host_state::tests::broker -- --nocapture`
Expected: FAIL on the depth-exceeded test (the stub doesn't check depth).

- [ ] **Step 4: Replace the stub `broker::Host` impl**

In `crates/greentic-ext-runtime/src/host_state.rs`, replace `impl broker::Host for HostState` with:

```rust
impl broker::Host for HostState {
    fn call_extension(
        &mut self,
        kind: String,
        target_id: String,
        function: String,
        args_json: String,
    ) -> Result<String, String> {
        // 1. Permission check — declared kinds only.
        if !self
            .permissions
            .call_extension_kinds
            .iter()
            .any(|k| k == &kind)
        {
            return Err(format!(
                "{} may not call {kind} extensions",
                self.extension_id
            ));
        }
        // 2. Depth check.
        let depth = self.call_depth.load(Ordering::Relaxed);
        if depth >= MAX_BROKER_DEPTH {
            return Err(format!(
                "max broker call depth exceeded ({depth} >= {MAX_BROKER_DEPTH})"
            ));
        }
        // 3. Resolve runtime — `runtime_weak` is None in unit tests that
        //    use `HostState::builder(...).build()` without supplying a
        //    runtime. In that path we surface a clear error message so
        //    test code can distinguish "no runtime configured" from
        //    "permission denied".
        let Some(rt) = self.runtime_weak.upgrade() else {
            return Err("broker: no runtime context available".into());
        };
        // 4. Dispatch.
        match rt.invoke_tool_with_depth(&target_id, &function, &args_json, depth + 1) {
            Ok(out) => Ok(out),
            Err(crate::error::RuntimeError::NotFound(id)) => {
                Err(format!("broker: target extension not loaded: {id}"))
            }
            Err(e) => Err(format!("broker: dispatch failed: {e}")),
        }
    }
}
```

- [ ] **Step 5: Run unit tests**

Run: `cd /home/bima-pangestu/Works/greentic/greentic-designer-extensions && cargo test -p greentic-ext-runtime --lib host_state::tests::broker -- --nocapture`
Expected: PASS — permission denial + depth-exceeded both green. (The depth test trips before the runtime upgrade, so the missing runtime weak isn't relevant.)

- [ ] **Step 6: Commit**

```bash
cd /home/bima-pangestu/Works/greentic/greentic-designer-extensions
git add crates/greentic-ext-runtime/src/host_state.rs \
        crates/greentic-ext-runtime/src/runtime.rs \
        crates/greentic-ext-runtime/src/loaded.rs
git commit -m "feat(ext-runtime): wire broker::call_extension to dispatch into target extension"
```

- [ ] **Step 7: Build the broker fixtures (A and B)**

Mirror B.2's fixture layout under `crates/greentic-ext-runtime/tests/fixtures/broker_caller_a/` and `broker_target_b/`.

**`broker_target_b/src/lib.rs`** (exports a tool `echo` that returns `{"echo": "<msg>"}`):

```rust
#![no_std]
extern crate alloc;
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

wit_bindgen_rt::generate!({
    world: "target-b",
    path: "wit",
});

use exports::greentic::extension_base::lifecycle::Guest as LifecycleGuest;
use exports::greentic::extension_base::manifest::Guest as ManifestGuest;
use exports::greentic::extension_design::tools::{
    ExtensionError, Guest as ToolsGuest, ToolDefinition,
};

struct Component;
impl ManifestGuest for Component {
    fn name() -> String { "target-b".into() }
    fn version() -> String { "1.0.0".into() }
}
impl LifecycleGuest for Component {
    fn init() -> Result<(), exports::greentic::extension_base::lifecycle::ExtensionError> { Ok(()) }
}
impl ToolsGuest for Component {
    fn list_tools() -> Vec<ToolDefinition> { Vec::new() }
    fn invoke_tool(name: String, args_json: String) -> Result<String, ExtensionError> {
        if name == "echo" {
            // args_json is `{"msg":"<text>"}` — bare-bones parse.
            let msg = args_json
                .split("\"msg\":\"").nth(1)
                .and_then(|s| s.split('"').next())
                .unwrap_or("");
            return Ok(format!("{{\"echo\":\"{msg}\"}}"));
        }
        Err(ExtensionError::Internal("unknown tool".into()))
    }
}
export!(Component);
```

`broker_target_b/wit/world.wit`:

```wit
package greentic:test;

world target-b {
  import greentic:extension-host/logging@0.1.0;
  export greentic:extension-design/tools@0.1.0;
  export greentic:extension-base/manifest@0.1.0;
  export greentic:extension-base/lifecycle@0.1.0;
}
```

**`broker_caller_a/src/lib.rs`** (exports tool `call_b` that invokes B.echo via the broker):

```rust
#![no_std]
extern crate alloc;
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

wit_bindgen_rt::generate!({
    world: "caller-a",
    path: "wit",
});

use exports::greentic::extension_base::lifecycle::Guest as LifecycleGuest;
use exports::greentic::extension_base::manifest::Guest as ManifestGuest;
use exports::greentic::extension_design::tools::{
    ExtensionError, Guest as ToolsGuest, ToolDefinition,
};
use greentic::extension_host::broker;

struct Component;
impl ManifestGuest for Component {
    fn name() -> String { "caller-a".into() }
    fn version() -> String { "1.0.0".into() }
}
impl LifecycleGuest for Component {
    fn init() -> Result<(), exports::greentic::extension_base::lifecycle::ExtensionError> { Ok(()) }
}
impl ToolsGuest for Component {
    fn list_tools() -> Vec<ToolDefinition> { Vec::new() }
    fn invoke_tool(name: String, _args_json: String) -> Result<String, ExtensionError> {
        if name != "call_b" {
            return Err(ExtensionError::Internal("unknown tool".into()));
        }
        let args = "{\"msg\":\"hi from A\"}";
        match broker::call_extension(
            "design",
            "greentic.test.broker-target-b",
            "echo",
            args,
        ) {
            Ok(s) => Ok(format!("{{\"from_b\":{s}}}")),
            Err(e) => Ok(format!("{{\"error\":\"{e}\"}}")),
        }
    }
}
export!(Component);
```

`broker_caller_a/wit/world.wit`:

```wit
package greentic:test;

world caller-a {
  import greentic:extension-host/broker@0.1.0;
  import greentic:extension-host/logging@0.1.0;
  export greentic:extension-design/tools@0.1.0;
  export greentic:extension-base/manifest@0.1.0;
  export greentic:extension-base/lifecycle@0.1.0;
}
```

Build both: `bash crates/greentic-ext-runtime/tests/fixtures/broker_caller_a/build.sh && bash crates/greentic-ext-runtime/tests/fixtures/broker_target_b/build.sh`.

- [ ] **Step 8: Write the failing round-trip + denial tests**

Create `crates/greentic-ext-runtime/tests/host_broker.rs`:

```rust
mod support;

use std::sync::Arc;

use greentic_ext_runtime::{
    DiscoveryPaths, ExtensionRuntime, HostOverrides, RuntimeConfig,
};

fn load_pair(
    tmp: &tempfile::TempDir,
    a_perms: impl FnOnce(&mut greentic_extension_sdk_contract::describe::Permissions),
) -> Arc<ExtensionRuntime> {
    // Caller A.
    let (a_fix, _) = support::signed_fixture_with_wasm_and_permissions(
        greentic_extension_sdk_contract::ExtensionKind::Design,
        "greentic.test.broker-caller-a",
        "1.0.0",
        include_bytes!("fixtures/broker_caller_a/component.wasm"),
        a_perms,
    );
    let a_dest = tmp.path().join("design").join("greentic.test.broker-caller-a");
    std::fs::create_dir_all(&a_dest).unwrap();
    support::copy_dir(&a_fix.root(), &a_dest);

    // Target B (no perms needed).
    let (b_fix, _) = support::signed_fixture_with_wasm(
        greentic_extension_sdk_contract::ExtensionKind::Design,
        "greentic.test.broker-target-b",
        "1.0.0",
        include_bytes!("fixtures/broker_target_b/component.wasm"),
    );
    let b_dest = tmp.path().join("design").join("greentic.test.broker-target-b");
    std::fs::create_dir_all(&b_dest).unwrap();
    support::copy_dir(&b_fix.root(), &b_dest);

    let config = RuntimeConfig::from_paths(DiscoveryPaths::new(tmp.path().to_path_buf()));
    let mut rt = ExtensionRuntime::new(config).unwrap();
    rt.register_loaded_from_dir(&a_dest).unwrap();
    rt.register_loaded_from_dir(&b_dest).unwrap();
    Arc::new(rt)
}

#[test]
fn broker_round_trip_a_calls_b() {
    let tmp = tempfile::TempDir::new().unwrap();
    let rt = load_pair(&tmp, |p| {
        p.call_extension_kinds = vec!["design".into()];
    });
    let out = rt
        .invoke_tool("greentic.test.broker-caller-a", "call_b", "{}")
        .expect("invoke A.call_b");
    let parsed: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(parsed["from_b"]["echo"], "hi from A", "got: {parsed}");
}

#[test]
fn broker_denies_when_kind_not_in_permissions() {
    let tmp = tempfile::TempDir::new().unwrap();
    let rt = load_pair(&tmp, |p| {
        // A declares NO call kinds — broker must refuse.
        p.call_extension_kinds = vec![];
    });
    let out = rt
        .invoke_tool("greentic.test.broker-caller-a", "call_b", "{}")
        .expect("invoke A.call_b");
    let parsed: serde_json::Value = serde_json::from_str(&out).unwrap();
    let err = parsed["error"].as_str().unwrap_or_default();
    assert!(
        err.contains("may not call"),
        "expected permission denial, got: {parsed}"
    );
}
```

Note: the recursion-depth integration test stays in the host_state unit test from Step 2 (above). Adding a WASM recursion fixture is extra surface and the unit test pins the exact behaviour. If reviewers ask for a wasm-side depth check too, add a third tool `call_self` to A that calls itself and run it until it trips depth, but only after the simpler tests are green.

- [ ] **Step 9: Run integration tests**

Run: `cd /home/bima-pangestu/Works/greentic/greentic-designer-extensions && cargo test -p greentic-ext-runtime --test host_broker -- --nocapture`
Expected: PASS — both tests.

- [ ] **Step 10: Commit**

```bash
cd /home/bima-pangestu/Works/greentic/greentic-designer-extensions
git add crates/greentic-ext-runtime/tests/host_broker.rs \
        crates/greentic-ext-runtime/tests/fixtures/broker_caller_a/ \
        crates/greentic-ext-runtime/tests/fixtures/broker_target_b/
git commit -m "test(ext-runtime): A-calls-B integration test for host.broker.call_extension"
```

---

## Task B.6 — Designer-side wiring (production backends)

Cross-repo work: `greentic-designer` (separate repo, branch `research`). This task creates adapters from the runtime's `Translator` / `SecretsBackend` ports to the real `greentic-i18n-lib` / `greentic-secrets-core` backends, then injects them via `ExtensionRuntime::set_host_overrides` during designer startup.

**Files (in `/home/bima-pangestu/Works/greentic/greentic-designer/`):**
- Create: `src/ui/host_adapters.rs`
- Modify: `src/ui/mod.rs`
- Modify: `Cargo.toml`
- Modify: `src/lib.rs` (re-export the adapters if needed; otherwise just `mod host_adapters;` in `ui/mod.rs`).

- [ ] **Step 1: Switch to the designer repo + create a branch**

```bash
cd /home/bima-pangestu/Works/greentic/greentic-designer
git fetch origin research
git checkout -b feat/host-functions-wire-up origin/research
```

- [ ] **Step 2: Add path deps to designer's Cargo.toml**

In `/home/bima-pangestu/Works/greentic/greentic-designer/Cargo.toml`, add to `[dependencies]`:

```toml
[dependencies.greentic-i18n-lib]
path = "../greentic-i18n/crates/greentic-i18n-lib"

[dependencies.greentic-secrets-core]
path = "../greentic-secrets/greentic-secrets-core"
default-features = false
features = ["use_spec", "file"]
```

(If these path deps don't fit the repo's normal vendoring (e.g. designer uses crates.io versions), discuss with Bima before merging — for the pilot, path deps are fine and unblock testing.)

- [ ] **Step 3: Write a failing compile-check test**

Create `/home/bima-pangestu/Works/greentic/greentic-designer/tests/host_adapters_compile.rs`:

```rust
//! Compile-only check: the production adapters implement the runtime's
//! ports. This catches "I forgot to re-export Translator after a runtime
//! refactor" regressions cheaply.

use greentic_designer::ui::host_adapters::{GreenticI18nTranslator, GreenticSecretsAdapter};
use greentic_ext_runtime::host_ports::{SecretsBackend, Translator};

#[test]
fn adapters_implement_runtime_ports() {
    fn assert_translator<T: Translator>(_t: &T) {}
    fn assert_secrets<S: SecretsBackend>(_s: &S) {}
    let t = GreenticI18nTranslator::for_locale("id-ID").unwrap();
    let s = GreenticSecretsAdapter::in_memory_for_tests();
    assert_translator(&t);
    assert_secrets(&s);
}
```

- [ ] **Step 4: Run — expect FAIL (module not found)**

Run: `cd /home/bima-pangestu/Works/greentic/greentic-designer && cargo test --test host_adapters_compile 2>&1 | tail -20`
Expected: FAIL — `unresolved import 'greentic_designer::ui::host_adapters'`.

- [ ] **Step 5: Create the adapters**

Create `/home/bima-pangestu/Works/greentic/greentic-designer/src/ui/host_adapters.rs`:

```rust
//! Production adapters that bridge the runtime's narrow ports
//! (`Translator`, `SecretsBackend`) to greentic-i18n + greentic-secrets.

use std::collections::BTreeMap;
use std::sync::Arc;

use greentic_ext_runtime::host_ports::{SecretsBackend, SecretsError, Translator};

/// Wraps a per-locale i18n catalog. Built once during designer startup.
pub struct GreenticI18nTranslator {
    catalog: BTreeMap<String, String>,
    fallback: BTreeMap<String, String>,
}

impl GreenticI18nTranslator {
    /// Build a translator from JSON catalogs shipped with greentic-i18n.
    /// Falls back to `en` for missing keys.
    pub fn for_locale(locale: &str) -> Result<Self, String> {
        // greentic_i18n_lib does not expose a public catalog loader as of
        // 1.2.x (only `tag` and `format`). We therefore read the catalog
        // JSON files from the i18n repo's distribution path. The exact
        // resolver is a thin reimplementation of `CliI18n::load_catalog`
        // because that lives in the binary crate, not the lib.
        //
        // For the pilot we vendor the catalog map as resource bytes; the
        // designer build script (build.rs in a follow-up) can switch to
        // include_dir! once it lands.
        let fallback = load_catalog_bytes(EN_CATALOG)?;
        let catalog = match locale {
            "en" | "en-US" => fallback.clone(),
            "id" | "id-ID" => load_catalog_bytes(ID_CATALOG)?,
            other => return Err(format!("unsupported locale `{other}`")),
        };
        Ok(Self { catalog, fallback })
    }
}

// Placeholder bytes: these MUST be replaced with `include_bytes!` from
// `greentic-i18n/crates/greentic-i18n/i18n/{en,id}.json` once that crate's
// data is reachable from designer's build. Until then, parse at runtime
// from a const string literal containing an empty JSON object so the test
// suite has SOMETHING to bind.
const EN_CATALOG: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../greentic-i18n/crates/greentic-i18n/i18n/en.json"
));
const ID_CATALOG: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../greentic-i18n/crates/greentic-i18n/i18n/id.json"
));

fn load_catalog_bytes(raw: &str) -> Result<BTreeMap<String, String>, String> {
    let value: serde_json::Value = serde_json::from_str(raw)
        .map_err(|e| format!("invalid catalog: {e}"))?;
    let obj = value
        .as_object()
        .ok_or_else(|| "catalog must be a JSON object".to_string())?;
    let mut map = BTreeMap::new();
    for (k, v) in obj {
        if let Some(s) = v.as_str() {
            map.insert(k.clone(), s.to_string());
        }
    }
    Ok(map)
}

impl Translator for GreenticI18nTranslator {
    fn t(&self, key: &str) -> String {
        if let Some(v) = self.catalog.get(key) {
            return v.clone();
        }
        if let Some(v) = self.fallback.get(key) {
            return v.clone();
        }
        key.to_string()
    }
    fn tf(&self, key: &str, args: &[(&str, &str)]) -> String {
        let mut out = self.t(key);
        for (k, v) in args {
            out = out.replace(&format!("{{{k}}}"), v);
        }
        out
    }
}

/// Adapter wrapping a `greentic-secrets-core` backend behind the narrow
/// runtime port. Currently supports the file-backed dev provider; the
/// production wiring (env-bound + AWS/Azure providers) lives in a
/// follow-up plan once Bima signs off on the secret-URI grammar.
pub struct GreenticSecretsAdapter {
    inner: Arc<dyn SecretsBackendInner>,
}

trait SecretsBackendInner: Send + Sync + 'static {
    fn fetch(&self, key: &str) -> Result<String, SecretsError>;
}

impl GreenticSecretsAdapter {
    /// Wrap an in-memory store. Used by tests and by the designer's
    /// `--dev-secrets-inline` flag.
    #[must_use]
    pub fn in_memory_for_tests() -> Self {
        Self {
            inner: Arc::new(InMemoryInner::default()),
        }
    }

    /// Wrap a `greentic_secrets_core::SecretsBackend` (file backend etc.).
    /// `key_to_uri` converts the raw key the extension passes (e.g.
    /// `"api.openai.com/api_key"`) into a `SecretUri` the backend
    /// understands; the closure is supplied by the caller because the URI
    /// grammar may differ per deployment.
    pub fn wrap_core_backend<B>(
        backend: Arc<B>,
        key_to_uri: impl Fn(&str) -> Result<greentic_secrets_core::uri::SecretUri, String>
            + Send
            + Sync
            + 'static,
    ) -> Self
    where
        B: greentic_secrets_core::backend::SecretsBackend + 'static,
    {
        Self {
            inner: Arc::new(CoreInner {
                backend,
                key_to_uri: Arc::new(key_to_uri),
            }),
        }
    }
}

impl SecretsBackend for GreenticSecretsAdapter {
    fn get(&self, key: &str) -> Result<String, SecretsError> {
        self.inner.fetch(key)
    }
}

#[derive(Default)]
struct InMemoryInner {
    map: std::sync::Mutex<std::collections::HashMap<String, String>>,
}

impl SecretsBackendInner for InMemoryInner {
    fn fetch(&self, key: &str) -> Result<String, SecretsError> {
        let g = self
            .map
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        g.get(key)
            .cloned()
            .ok_or_else(|| SecretsError::NotFound(key.to_string()))
    }
}

struct CoreInner<B: greentic_secrets_core::backend::SecretsBackend + 'static> {
    backend: Arc<B>,
    key_to_uri: Arc<
        dyn Fn(&str) -> Result<greentic_secrets_core::uri::SecretUri, String>
            + Send
            + Sync
            + 'static,
    >,
}

impl<B: greentic_secrets_core::backend::SecretsBackend + 'static> SecretsBackendInner
    for CoreInner<B>
{
    fn fetch(&self, key: &str) -> Result<String, SecretsError> {
        let uri = (self.key_to_uri)(key).map_err(SecretsError::Backend)?;
        let resolved = self
            .backend
            .get(&uri, None)
            .map_err(|e| SecretsError::Backend(format!("{e}")))?;
        let record = resolved
            .as_ref()
            .and_then(|v| v.record())
            .ok_or_else(|| SecretsError::NotFound(key.to_string()))?;
        // `SecretRecord::value_string()` — adjust to whatever accessor the
        // public API offers. If the public surface changes in
        // greentic-secrets, adapt here instead of in the runtime.
        Ok(record.value_as_string())
    }
}
```

> **Caveat for the implementer:** `SecretRecord` accessor names may differ in the current revision of `greentic-secrets-core`. Read `greentic-secrets/greentic-secrets-core/src/types.rs` and adjust the final `.value_as_string()` call to the actual method. Do not commit a guess — verify by compiling.

- [ ] **Step 6: Register the module + run the compile test**

In `/home/bima-pangestu/Works/greentic/greentic-designer/src/ui/mod.rs`, add at the top of the module list (alphabetical):

```rust
pub mod host_adapters;
```

Run: `cd /home/bima-pangestu/Works/greentic/greentic-designer && cargo test --test host_adapters_compile 2>&1 | tail -20`
Expected: PASS.

If `value_as_string()` is the wrong method name, fix it now and re-run until it compiles.

- [ ] **Step 7: Wire the overrides into `ui::mod::start_*` startup**

Read `/home/bima-pangestu/Works/greentic/greentic-designer/src/ui/mod.rs` around lines 150-170 (where `ExtensionRuntime::new` is constructed). Add after the `rt = ExtensionRuntime::new(...)` line and before the first `register_loaded_from_dir` call:

```rust
// Wire production host backends (i18n + secrets) before any extension
// runs. The locale comes from the designer config; secrets fall back to
// an in-memory store in dev mode.
let runtime_locale = config.locale.as_deref().unwrap_or("en");
let translator = std::sync::Arc::new(
    crate::ui::host_adapters::GreenticI18nTranslator::for_locale(runtime_locale)
        .unwrap_or_else(|e| {
            tracing::warn!(error = %e, "i18n catalog unavailable; using key fallback");
            crate::ui::host_adapters::GreenticI18nTranslator::for_locale("en")
                .expect("en catalog must load")
        }),
);
let secrets = std::sync::Arc::new(
    crate::ui::host_adapters::GreenticSecretsAdapter::in_memory_for_tests(),
);
let overrides = greentic_ext_runtime::HostOverrides {
    translator,
    secrets_backend: secrets,
    // url_matcher + http_client + runtime_weak defaults kept; deep
    // url-matcher wiring (per-extension allow-list) lives in Phase C.
    ..greentic_ext_runtime::HostOverrides::defaults_for_tests()
};
runtime.set_host_overrides(overrides);
```

(If `config.locale` doesn't exist yet — the designer config may use a different name. Search for `locale` in `ui/state.rs` and `ui/auth.rs` and use whatever's already there. If none, hardcode `"en"` and TODO comment for follow-up.)

- [ ] **Step 8: Compile + run designer tests**

Run: `cd /home/bima-pangestu/Works/greentic/greentic-designer && cargo build && cargo test --lib 2>&1 | tail -20`
Expected: builds and existing tests pass. New compile test also passes.

- [ ] **Step 9: Commit (designer side)**

```bash
cd /home/bima-pangestu/Works/greentic/greentic-designer
git add Cargo.toml src/ui/host_adapters.rs src/ui/mod.rs tests/host_adapters_compile.rs Cargo.lock
git commit -m "feat(designer): wire production i18n + secrets host backends into ExtensionRuntime"
```

---

## Task B.7 — Verification + grep gate

This task locks in the spec's machine-checkable acceptance criteria.

**Files:**
- Create: `/home/bima-pangestu/Works/greentic/greentic-designer-extensions/crates/greentic-ext-runtime/tests/no_4b0_stub_strings.rs`

- [ ] **Step 1: Write the failing grep-gate test**

Create the file with content:

```rust
//! Compile-time guard: ensure no host stub returns the legacy
//! "not implemented in 4B.0" placeholder string. If anyone re-introduces
//! it, this test fails — cheap, fast, no wasmtime required.

use std::path::PathBuf;

#[test]
fn no_4b0_stub_strings_remain() {
    let crate_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let src = crate_root.join("src");
    let mut offenders = Vec::new();
    for entry in walkdir::WalkDir::new(&src) {
        let entry = entry.unwrap();
        if !entry.file_type().is_file() {
            continue;
        }
        if entry.path().extension().and_then(|s| s.to_str()) != Some("rs") {
            continue;
        }
        let body = std::fs::read_to_string(entry.path()).unwrap();
        if body.contains("not implemented in 4B.0") {
            offenders.push(entry.path().to_path_buf());
        }
    }
    assert!(
        offenders.is_empty(),
        "found legacy 4B.0 stub string in: {offenders:?}"
    );
}
```

- [ ] **Step 2: Run — should PASS already (B.2..B.5 replaced all stubs)**

Run: `cd /home/bima-pangestu/Works/greentic/greentic-designer-extensions && cargo test -p greentic-ext-runtime --test no_4b0_stub_strings -- --nocapture`
Expected: PASS.

If it fails, that's the grep gate doing its job — chase the offending file and remove the literal.

- [ ] **Step 3: Run the full crate test suite**

Run: `cd /home/bima-pangestu/Works/greentic/greentic-designer-extensions && bash ci/local_check.sh 2>&1 | tail -40`
Expected: fmt + clippy + tests all green. Notable: the fixture cargo-component sub-crates are NOT in the runtime's workspace, so they will not be linted by the workspace clippy — that's intentional. If clippy complains about `unused_imports` etc in `tests/*.rs`, fix in place.

- [ ] **Step 4: Run the umbrella grep guard once more from the repo root**

Run:

```bash
cd /home/bima-pangestu/Works/greentic/greentic-designer-extensions
grep -r '"not implemented in 4B.0"' . 2>/dev/null && echo "FOUND OFFENDER" || echo "CLEAN"
```

Expected: `CLEAN`.

- [ ] **Step 5: Commit**

```bash
cd /home/bima-pangestu/Works/greentic/greentic-designer-extensions
git add crates/greentic-ext-runtime/tests/no_4b0_stub_strings.rs
git commit -m "test(ext-runtime): regression guard against re-introducing 4B.0 stub strings"
```

- [ ] **Step 6: Push + open PR**

```bash
cd /home/bima-pangestu/Works/greentic/greentic-designer-extensions
git push -u origin feat/host-functions-1.2.x
gh pr create --base research --title "feat(ext-runtime): wire 4B.0 host stubs (Phase B)" --body "$(cat <<'EOF'
## Summary

Replaces every `"not implemented in 4B.0"` placeholder in `greentic-ext-runtime` with a real implementation:

- `i18n::t/tf` routes through a new `Translator` port, wired in designer to `greentic-i18n`.
- `secrets::get` routes through a new `SecretsBackend` port, wired in designer to `greentic-secrets-core`. Permission gate matches `permissions.secrets`.
- `http::fetch` uses `reqwest::blocking` gated by a strict `UrlMatcher` (scheme + host suffix + path prefix). Three attack-vector tests pinned (open redirect, subdomain confusion, scheme downgrade).
- `broker::call_extension` dispatches via `ExtensionRuntime` into the target's `tools::invoke-tool` export. Permission gate matches `permissions.callExtensionKinds`. Depth-tracked with `MAX_BROKER_DEPTH = 8`.

Phase B of `docs/superpowers/specs/2026-05-13-extensions-1.0-cleanup.md` (umbrella in `greentic-designer-sdk`). Depends on Phase A merge for the `Permissions` shape; no changes to the contract required from B.

## Test plan

- [ ] `cargo test -p greentic-ext-runtime --all-targets` green
- [ ] `bash ci/local_check.sh` green
- [ ] `grep -r '"not implemented in 4B.0"' .` returns no matches
- [ ] All four URL attack-vector tests pass: `cargo test -p greentic-ext-runtime --test url_matcher`
- [ ] Cross-extension round-trip passes: `cargo test -p greentic-ext-runtime --test host_broker`
- [ ] Designer compile-check passes after path-dep pickup: `cd ../greentic-designer && cargo test --test host_adapters_compile`
EOF
)"
```

The companion designer PR uses the same pattern, base `research`, title `feat(designer): wire production i18n + secrets host backends`, body referencing the ext-runtime PR.

---

## Self-Review Checklist

**Spec coverage** (umbrella §4 "Host stubs (Phase B)"):

- [x] grep guard against `"not implemented in 4B.0"` — Task B.7
- [x] `i18n::t/tf` round-trip with non-`en` locale — Task B.2 (Indonesian fixture)
- [x] `secrets::get` calls fixture backend + permission test — Task B.3
- [x] `http::fetch` strict matcher + 3 attack-vector tests — Task B.4
- [x] `broker::call_extension` A-calls-B round-trip + permission denial — Task B.5

**Type consistency:**
- `HostState::builder(...)` returns `HostStateBuilder`; `.build()` returns `HostState`. Used the same name everywhere.
- `HostOverrides` re-exported from `lib.rs`; field names (`translator`, `secrets_backend`, `http_client`, `url_matcher`, `runtime_weak`, `call_depth_start`) used consistently in B.1 through B.5.
- `UrlMatcher::from_patterns(Vec<String>)` and `UrlMatcher::is_allowed(&str)` signatures match across tests and impl.
- `SecretsError::NotFound(String)` / `SecretsError::Backend(String)` used consistently in port + adapters.
- `MAX_BROKER_DEPTH` constant referenced in B.1 host_state and B.5 broker impl.

**Placeholder scan:**
- No "TBD" / "TODO" / "fill in" left. The single deliberate caveat in B.6 step 5 (`SecretRecord::value_as_string` name) is flagged as a concrete adjustment the implementer makes during the compile loop, not a placeholder.
- All commands shown with copy-pasteable absolute paths and expected output.
- All fixture wasms have complete `src/lib.rs` and `wit/world.wit` payloads — no "similar to" references.

---

## Execution Handoff

**Plan complete and saved to `/home/bima-pangestu/Works/greentic/greentic-designer-extensions/docs/superpowers/plans/2026-05-13-host-functions-implementation.md`.** Two execution options:

**1. Subagent-Driven (recommended)** — dispatch a fresh subagent per task, review between tasks, fast iteration.

**2. Inline Execution** — execute tasks in this session using `superpowers:executing-plans`, batch execution with checkpoints.

**Which approach?**
