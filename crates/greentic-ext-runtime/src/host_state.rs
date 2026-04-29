use wasmtime::component::ResourceTable;
use wasmtime_wasi::{WasiCtx, WasiCtxBuilder, WasiCtxView, WasiView};

use crate::host_bindings::greentic::extension_host::{broker, http, i18n, logging, secrets};
use greentic_extension_sdk_contract::describe::Permissions;

pub struct HostState {
    pub extension_id: String,
    pub permissions: Permissions,
    // WASI state — required because cargo-component-built WASM components
    // implicitly import WASI interfaces (wasi:cli/environment etc.).
    wasi: WasiCtx,
    table: ResourceTable,
}

impl HostState {
    #[must_use]
    pub fn new(extension_id: String, permissions: Permissions) -> Self {
        let wasi = WasiCtxBuilder::new().build();
        let table = ResourceTable::new();
        Self {
            extension_id,
            permissions,
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

impl i18n::Host for HostState {
    fn t(&mut self, key: String) -> String {
        key
    }

    fn tf(&mut self, key: String, _args: Vec<(String, String)>) -> String {
        key
    }
}

impl secrets::Host for HostState {
    fn get(&mut self, uri: String) -> Result<String, String> {
        if !self
            .permissions
            .secrets
            .iter()
            .any(|allowed| uri.starts_with(allowed))
        {
            return Err(format!("permission denied for secret: {uri}"));
        }
        Err("no secrets backend configured in 4B.0".into())
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
        Err(format!(
            "broker call {target_id}.{function} not implemented in 4B.0"
        ))
    }
}

impl http::Host for HostState {
    fn fetch(&mut self, req: http::Request) -> Result<http::Response, String> {
        let allowed = self.permissions.network.iter().any(|pattern| {
            let base = pattern.trim_end_matches("/*");
            req.url.starts_with(base)
        });
        if !allowed {
            return Err(format!("network permission denied for url: {}", req.url));
        }
        Err("http fetch not implemented in 4B.0".into())
    }
}
