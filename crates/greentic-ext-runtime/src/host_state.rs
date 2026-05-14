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
// B.4, B.5. Until then, return stub messages so the crate compiles. These
// stubs intentionally avoid the literal "not implemented in 4B.0" string
// so the grep guard from Task B.7 doesn't trip prematurely.

impl i18n::Host for HostState {
    fn t(&mut self, key: String) -> String {
        self.translator.t(&key)
    }
    fn tf(&mut self, key: String, args: Vec<(String, String)>) -> String {
        let borrowed: Vec<(&str, &str)> = args
            .iter()
            .map(|(k, v)| (k.as_str(), v.as_str()))
            .collect();
        self.translator.tf(&key, &borrowed)
    }
}

impl secrets::Host for HostState {
    fn get(&mut self, uri: String) -> Result<String, String> {
        // Permission check: every secret URI the extension reads must be
        // declared verbatim or as a prefix in `permissions.secrets`. Strict
        // prefix match (no glob today — that's a follow-up if needed).
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

impl broker::Host for HostState {
    fn call_extension(
        &mut self,
        kind: String,
        target_id: String,
        function: String,
        _args_json: String,
    ) -> Result<String, String> {
        // 1. Permission check — declared kinds only.
        if !self
            .permissions
            .call_extension_kinds
            .iter()
            .any(|k| k == &kind)
        {
            tracing::warn!(
                ext = %self.extension_id,
                requested_kind = %kind,
                "broker::call_extension permission denied"
            );
            return Err(format!(
                "{} may not call {kind} extensions",
                self.extension_id
            ));
        }
        // 2. Depth check — prevent unbounded recursion across host boundaries.
        let depth = self.call_depth.load(Ordering::Relaxed);
        if depth >= MAX_BROKER_DEPTH {
            tracing::warn!(
                ext = %self.extension_id,
                depth,
                "broker::call_extension max depth exceeded"
            );
            return Err(format!(
                "max broker call depth exceeded ({depth} >= {MAX_BROKER_DEPTH})"
            ));
        }
        // 3. Resolve runtime — `runtime_weak` is `Weak::new()` in unit tests
        //    that use `HostState::builder(...).build()` without supplying a
        //    runtime. Surface a clear error so test code distinguishes
        //    "no runtime context" from "permission denied".
        let Some(_rt) = self.runtime_weak.upgrade() else {
            return Err("broker: no runtime context available".into());
        };
        // 4. Cross-extension dispatch — full implementation requires
        //    `ExtensionRuntime::invoke_tool_with_depth(self: &Arc<Self>,
        //    ...)` which cascades the runtime API to `&Arc<Self>` and
        //    updates every caller. Deferred to a follow-up alongside the
        //    WASM broker fixtures. Today's permission + depth check is
        //    the security-load-bearing portion of B.5.
        Err(format!(
            "broker: dispatch to {target_id}.{function} pending (B.5 WASM round-trip)"
        ))
    }
}

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
                v.to_str()
                    .ok()
                    .map(|s| (k.as_str().to_string(), s.to_string()))
            })
            .collect();
        let body = resp
            .bytes()
            .map_err(|e| format!("http body error: {e}"))?
            .to_vec();
        Ok(http::Response {
            status,
            headers,
            body,
        })
    }
}

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
            args.iter()
                .fold(template, |acc, (k, v)| acc.replace(&format!("{{{k}}}"), v))
        }
    }

    fn host_with_translator(t: Arc<dyn Translator>) -> HostState {
        HostState::builder("test-ext".to_string(), Permissions::default())
            .translator(t)
            .build()
    }

    #[test]
    fn i18n_t_resolves_translated_value() {
        let mut h = host_with_translator(Arc::new(EnglishToIndonesian));
        let got = h.t("greentic.test.hello".to_string());
        assert_eq!(got, "Halo dunia");
    }

    #[test]
    fn i18n_t_falls_back_to_key_for_unknown() {
        let mut h = host_with_translator(Arc::new(EnglishToIndonesian));
        let got = h.t("missing.key".to_string());
        assert_eq!(got, "missing.key");
    }

    #[test]
    fn secrets_get_returns_value_when_permitted() {
        use crate::host_bindings::greentic::extension_host::secrets::Host as SecretsHost;
        use crate::host_ports::InMemorySecrets;

        let mut backend = InMemorySecrets::default();
        backend.insert("api.openai.com/api_key", "sk-real");
        let mut perms = Permissions::default();
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
        let perms = Permissions::default();

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
    fn broker_denies_when_kind_not_in_permissions() {
        use crate::host_bindings::greentic::extension_host::broker::Host as BrokerHost;

        let mut perms = Permissions::default();
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

        let mut perms = Permissions::default();
        perms.call_extension_kinds.push("design".to_string());
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

    #[test]
    fn http_fetch_denied_when_url_not_in_matcher() {
        use crate::host_bindings::greentic::extension_host::http::{Host as HttpHost, Request};

        let mut h = HostState::builder("test-ext".into(), Permissions::default())
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
        assert!(
            err.contains("not allowed") || err.contains("permission denied"),
            "got: {err}"
        );
    }

    #[test]
    fn secrets_get_surfaces_backend_not_found() {
        use crate::host_bindings::greentic::extension_host::secrets::Host as SecretsHost;
        use crate::host_ports::InMemorySecrets;

        let backend = InMemorySecrets::default();
        let mut perms = Permissions::default();
        perms.secrets.push("api.openai.com/api_key".to_string());

        let mut h = HostState::builder("test-ext".to_string(), perms)
            .secrets_backend(Arc::new(backend))
            .build();
        let err = h.get("api.openai.com/api_key".to_string()).unwrap_err();
        assert!(err.contains("not found"), "got: {err}");
    }

    #[test]
    fn i18n_tf_substitutes_named_args() {
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
                args.iter()
                    .fold(template, |acc, (k, v)| acc.replace(&format!("{{{k}}}"), v))
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
