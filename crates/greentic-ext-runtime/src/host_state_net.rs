//! `Host` impls for the network-facing host interfaces: http and llm.
//!
//! Split out of [`crate::host_state`]; the local impls (logging, i18n, secrets,
//! broker) live in [`crate::host_state_ports`] and the oauth-broker impl in
//! [`crate::host_state_oauth`].

use std::io::Read;

use crate::host_bindings::greentic::extension_host::{http, llm};
use crate::host_state::HostState;

/// Maximum response body the host will hand back to a guest, in bytes.
///
/// `Response::bytes()` buffers the whole body with no ceiling, so an allowed
/// endpoint serving an endless stream could exhaust host memory — the guest
/// does not even have to be malicious, only pointed at the wrong URL. 32 MiB is
/// far above any real design-extension payload (schemas, card JSON, small
/// assets) and far below anything that threatens the process.
const MAX_RESPONSE_BYTES: u64 = 32 * 1024 * 1024;

/// Render a URL for logging with its credential-bearing parts removed.
///
/// Query strings routinely carry `?api_key=`, `?access_token=`, presigned SAS
/// tokens and OAuth `?code=`, and userinfo carries a password outright. The
/// denial log below is the sharp case: it fires *because* the URL was rejected,
/// which is to say on attacker-influenced input.
fn loggable(url: &str) -> String {
    match url::Url::parse(url) {
        Ok(mut u) => {
            u.set_query(None);
            u.set_fragment(None);
            let _ = u.set_password(None);
            let _ = u.set_username("");
            u.to_string()
        }
        // Unparseable: the matcher rejected it anyway, and echoing it back into
        // a log is not worth the chance that it is a credential-bearing string.
        Err(_) => "<unparseable url>".to_string(),
    }
}

impl http::Host for HostState {
    fn fetch(&mut self, req: http::Request) -> Result<http::Response, String> {
        // 1. Permission check via strict UrlMatcher.
        if !self.url_matcher.is_allowed(&req.url) {
            tracing::warn!(
                ext = %self.extension_id,
                url = %loggable(&req.url),
                "http::fetch permission denied"
            );
            return Err(format!("network not allowed for url: {}", req.url));
        }

        // 2. Build the reqwest request. `http_client` is `None` when the
        //    host wasn't given one (typical for unit tests). Surface a
        //    clean error rather than panic, and don't lazy-construct a
        //    client here — see `HostOverrides` doc comment.
        let client = self
            .http_client
            .as_ref()
            .ok_or_else(|| "http client not configured for this runtime".to_string())?;
        let method = match req.method.to_uppercase().as_str() {
            "GET" => reqwest::Method::GET,
            "POST" => reqwest::Method::POST,
            "PUT" => reqwest::Method::PUT,
            "DELETE" => reqwest::Method::DELETE,
            "PATCH" => reqwest::Method::PATCH,
            "HEAD" => reqwest::Method::HEAD,
            other => return Err(format!("unsupported http method: {other}")),
        };
        let mut builder = client.request(method, &req.url);
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

        // 4. Re-check the URL we actually landed on. The allow-list is enforced
        //    per URL, but a reqwest client follows redirects by default, so an
        //    allowed host can 302 the guest onto an internal address — a cloud
        //    metadata endpoint, a service on loopback — and the allow-list
        //    would never see it. Refusing the response keeps that content out
        //    of the guest. Hosts that want the request never to leave the list
        //    at all should build the client with `redirect::Policy::none()`;
        //    see `HostStateBuilder::http_client`.
        let final_url = resp.url().clone();
        if final_url.as_str() != req.url && !self.url_matcher.is_allowed(final_url.as_str()) {
            tracing::warn!(
                ext = %self.extension_id,
                requested = %loggable(&req.url),
                final_url = %loggable(final_url.as_str()),
                "http::fetch redirected off the allow-list; response withheld"
            );
            return Err(format!(
                "network not allowed for redirect target: {}",
                loggable(final_url.as_str())
            ));
        }

        let status = resp.status().as_u16();
        let headers = resp
            .headers()
            .iter()
            // The WIT response type is `list<tuple<string, string>>`, so a
            // header whose bytes are not valid UTF-8 has no representation to
            // hand the guest. Dropping it is the only faithful option — this is
            // a limit of the contract, not a failure of the request.
            .filter_map(|(k, v)| {
                v.to_str()
                    .ok()
                    .map(|s| (k.as_str().to_string(), s.to_string()))
            })
            .collect();

        // 5. Read the body under a ceiling. Reading one byte past the cap is
        //    what distinguishes "exactly at the limit" from "truncated", so an
        //    oversized body is rejected rather than silently cut short.
        let mut body = Vec::new();
        resp.take(MAX_RESPONSE_BYTES + 1)
            .read_to_end(&mut body)
            .map_err(|e| format!("http body error: {e}"))?;
        if body.len() as u64 > MAX_RESPONSE_BYTES {
            tracing::warn!(
                ext = %self.extension_id,
                url = %loggable(&req.url),
                cap = MAX_RESPONSE_BYTES,
                "http::fetch response exceeded the body cap"
            );
            return Err(format!(
                "http response body exceeds the {MAX_RESPONSE_BYTES} byte cap"
            ));
        }

        Ok(http::Response {
            status,
            headers,
            body,
        })
    }
}

impl llm::Host for HostState {
    fn complete(&mut self, request: llm::LlmRequest) -> Result<llm::LlmResponse, String> {
        // 1. Resolve the effective role from describe permissions. A `role_hint`
        //    must be one the extension declared; with no hint we allow the sole
        //    declared role and otherwise require disambiguation.
        let declared = &self.permissions.llm_roles;
        let role = match (&request.role_hint, declared.as_slice()) {
            (Some(hint), roles) if roles.iter().any(|r| r == hint) => hint.clone(),
            (Some(hint), _) => {
                tracing::warn!(ext = %self.extension_id, requested = %hint, "llm role not permitted");
                return Err(format!("llm role not permitted: {hint}"));
            }
            (None, [sole]) => sole.clone(),
            (None, []) => {
                return Err("llm role not permitted: extension declares no llm_roles".to_string());
            }
            (None, _many) => {
                return Err(
                    "llm role-hint required: extension declares multiple llm_roles".to_string(),
                );
            }
        };

        // 2. Resolve the port. Absent in unit tests and runtimes the host did
        //    not wire for LLM use — surface a clean error rather than panic.
        let Some(port) = self.llm_port.as_ref() else {
            return Err("llm not configured for this runtime".to_string());
        };

        // 3. Map the WIT request onto the host port, call, map the response.
        let port_req = crate::host_ports::LlmPortRequest {
            system_prompt: request.system_prompt,
            messages: request
                .messages
                .into_iter()
                .map(|m| (m.role, m.content))
                .collect(),
            response_format: match request.response_format {
                None | Some(llm::ResponseFormat::Text) => {
                    crate::host_ports::LlmPortResponseFormat::Text
                }
                Some(llm::ResponseFormat::Json) => crate::host_ports::LlmPortResponseFormat::Json,
                Some(llm::ResponseFormat::JsonSchema(s)) => {
                    crate::host_ports::LlmPortResponseFormat::JsonSchema(s)
                }
            },
        };
        match port.complete(&self.extension_id, &self.call_ctx, &role, port_req) {
            Ok(r) => Ok(llm::LlmResponse {
                content: r.content,
                total_tokens: r.total_tokens,
            }),
            Err(e) => {
                tracing::warn!(ext = %self.extension_id, %role, error = %e, "llm port error");
                Err(e.to_string())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::host_bindings::greentic::extension_host::http::{Host as HttpHost, Request};
    use crate::host_bindings::greentic::extension_host::llm::Host as LlmHost;
    use crate::host_ports::{HostCallContext, LlmPort, LlmPortRequest, LlmPortResponse};
    use greentic_extension_sdk_contract::describe::Permissions;
    use std::sync::Arc;

    #[test]
    fn http_fetch_denied_when_url_not_in_matcher() {
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
    fn http_fetch_without_a_client_reports_it_rather_than_panicking() {
        let mut h = HostState::builder("test-ext".into(), Permissions::default())
            .url_matcher(crate::url_matcher::UrlMatcher::from_patterns(vec![
                "https://allowed.com/*".into(),
            ]))
            .build();
        let req = Request {
            method: "GET".into(),
            url: "https://allowed.com/x".into(),
            headers: vec![],
            body: None,
        };
        let err = h.fetch(req).unwrap_err();
        assert!(err.contains("http client not configured"), "got: {err}");
    }

    /// In-test [`LlmPort`] that records the `extension_id` / `ctx` / `role`
    /// it was called with and echoes the system prompt back. Asserts the host
    /// resolved the expected role and threaded the expected tenant + user email
    /// before forwarding.
    ///
    /// `expected_*` prefix is deliberate (these are the values the port asserts
    /// against, not generic data), so the shared-prefix lint is silenced here.
    #[allow(clippy::struct_field_names)]
    struct FakeLlm {
        expected_extension_id: String,
        expected_tenant: Option<String>,
        expected_user_email: Option<String>,
        expected_role: String,
    }

    impl LlmPort for FakeLlm {
        fn complete(
            &self,
            extension_id: &str,
            ctx: &HostCallContext,
            role: &str,
            request: LlmPortRequest,
        ) -> Result<LlmPortResponse, crate::host_ports::LlmPortError> {
            assert_eq!(extension_id, self.expected_extension_id, "extension_id");
            assert_eq!(ctx.tenant, self.expected_tenant, "tenant");
            assert_eq!(ctx.user_email, self.expected_user_email, "user_email");
            assert_eq!(role, self.expected_role, "role");
            Ok(LlmPortResponse {
                content: format!("echo:{}", request.system_prompt),
                total_tokens: Some(7),
            })
        }
    }

    fn llm_request(role_hint: Option<&str>) -> llm::LlmRequest {
        llm::LlmRequest {
            role_hint: role_hint.map(str::to_string),
            system_prompt: "you are a composer".to_string(),
            messages: vec![],
            response_format: None,
        }
    }

    fn fake_llm(tenant: Option<&str>, user_email: Option<&str>) -> Arc<FakeLlm> {
        Arc::new(FakeLlm {
            expected_extension_id: "test-ext".to_string(),
            expected_tenant: tenant.map(str::to_string),
            expected_user_email: user_email.map(str::to_string),
            expected_role: "sorla_composer".to_string(),
        })
    }

    fn perms_with_roles(roles: &[&str]) -> Permissions {
        let mut perms = Permissions::default();
        perms
            .llm_roles
            .extend(roles.iter().map(|r| (*r).to_string()));
        perms
    }

    #[test]
    fn llm_complete_resolves_sole_declared_role() {
        let mut h = HostState::builder(
            "test-ext".to_string(),
            perms_with_roles(&["sorla_composer"]),
        )
        .llm_port(Some(fake_llm(Some("acme"), None)))
        .call_ctx(HostCallContext {
            tenant: Some("acme".into()),
            user_email: None,
        })
        .build();

        let resp = h
            .complete(llm_request(None))
            .expect("complete should succeed");
        assert_eq!(resp.content, "echo:you are a composer");
        assert_eq!(resp.total_tokens, Some(7));
    }

    #[test]
    fn llm_complete_passes_none_tenant_by_default() {
        // No `.call_ctx(...)` in the builder chain — the host runs
        // single-tenant/dev, so the port must observe a default (all-`None`)
        // context.
        let mut h = HostState::builder(
            "test-ext".to_string(),
            perms_with_roles(&["sorla_composer"]),
        )
        .llm_port(Some(fake_llm(None, None)))
        .build();

        let resp = h
            .complete(llm_request(None))
            .expect("complete should succeed");
        assert_eq!(resp.content, "echo:you are a composer");
    }

    #[test]
    fn llm_complete_passes_user_email() {
        let mut h = HostState::builder(
            "test-ext".to_string(),
            perms_with_roles(&["sorla_composer"]),
        )
        .llm_port(Some(fake_llm(Some("acme"), Some("alice@acme.com"))))
        .call_ctx(HostCallContext {
            tenant: Some("acme".into()),
            user_email: Some("alice@acme.com".into()),
        })
        .build();

        let resp = h
            .complete(llm_request(None))
            .expect("complete should succeed");
        assert_eq!(resp.content, "echo:you are a composer");
    }

    #[test]
    fn llm_complete_rejects_undeclared_role() {
        let mut h = HostState::builder("test-ext".to_string(), Permissions::default())
            .llm_port(Some(fake_llm(None, None)))
            .build();

        let err = h.complete(llm_request(Some("sorla_composer"))).unwrap_err();
        assert!(err.contains("llm role not permitted"), "got: {err}");
    }

    #[test]
    fn llm_complete_honours_a_valid_hint_among_several_roles() {
        // The selection arm itself: with more than one role declared, the hint
        // decides. Nothing covered a hint that is actually accepted, so the
        // arm could have returned any declared role and stayed green.
        let port = Arc::new(FakeLlm {
            expected_extension_id: "test-ext".to_string(),
            expected_tenant: None,
            expected_user_email: None,
            expected_role: "reviewer".to_string(),
        });
        let mut h = HostState::builder(
            "test-ext".to_string(),
            perms_with_roles(&["composer", "reviewer"]),
        )
        .llm_port(Some(port))
        .build();

        let resp = h
            .complete(llm_request(Some("reviewer")))
            .expect("a declared role named by the hint must be accepted");
        assert_eq!(resp.content, "echo:you are a composer");
    }

    #[test]
    fn llm_complete_rejects_a_hint_that_is_not_among_the_declared_roles() {
        let mut h = HostState::builder(
            "test-ext".to_string(),
            perms_with_roles(&["composer", "reviewer"]),
        )
        .llm_port(Some(fake_llm(None, None)))
        .build();

        let err = h.complete(llm_request(Some("admin"))).unwrap_err();
        assert!(err.contains("llm role not permitted: admin"), "got: {err}");
    }

    #[test]
    fn llm_complete_without_port_errors() {
        let mut h = HostState::builder(
            "test-ext".to_string(),
            perms_with_roles(&["sorla_composer"]),
        )
        .build();
        let err = h.complete(llm_request(None)).unwrap_err();
        assert!(err.contains("llm not configured"), "got: {err}");
    }

    #[test]
    fn llm_complete_requires_hint_when_multiple_roles() {
        let mut h = HostState::builder("test-ext".to_string(), perms_with_roles(&["a", "b"]))
            .llm_port(Some(fake_llm(None, None)))
            .build();
        let err = h.complete(llm_request(None)).unwrap_err();
        assert!(err.contains("role-hint required"), "got: {err}");
    }
}
