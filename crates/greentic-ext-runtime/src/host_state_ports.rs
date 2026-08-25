//! `Host` impls for the local host interfaces: logging, i18n, secrets, broker.
//!
//! Split out of [`crate::host_state`]; the network-facing impls (http, llm,
//! oauth-broker) live in [`crate::host_state_net`].

use std::sync::atomic::Ordering;

use crate::host_bindings::greentic::extension_host::{broker, i18n, logging, secrets};
use crate::host_state::{HostState, MAX_BROKER_DEPTH};

impl logging::Host for HostState {
    fn log(&mut self, level: logging::Level, target: String, message: String) {
        let ext = &self.extension_id;
        // `message` is interpolated as a captured argument, never as the format
        // string, so guest-controlled text cannot inject tracing fields.
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
        self.translator.t(&key)
    }

    fn tf(&mut self, key: String, args: Vec<(String, String)>) -> String {
        let borrowed: Vec<(&str, &str)> =
            args.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
        self.translator.tf(&key, &borrowed)
    }
}

impl secrets::Host for HostState {
    fn get(&mut self, uri: String) -> Result<String, String> {
        // Permission check: every secret URI the extension reads must be
        // declared verbatim or as a path prefix in `permissions.secrets`.
        // The `/` boundary matters — a bare `starts_with` would let a
        // declaration of `api.openai.com` also unlock `api.openai.com.evil/key`.
        let permitted = self
            .permissions
            .secrets
            .iter()
            .filter(|allowed| declaration_is_specific(allowed))
            .any(|allowed| {
                uri == *allowed
                    || (uri.len() > allowed.len()
                        && uri.starts_with(allowed.as_str())
                        && uri.as_bytes()[allowed.len()] == b'/')
            });
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

/// Does a declared secret prefix actually name something?
///
/// The `/`-boundary match below is only as narrow as what it is matching
/// against. `""` covers every URI starting with `/`, and `"secrets:"` covers
/// every `secrets://…` URI — the whole namespace — because the byte at the
/// prefix length is `/` in both cases. There is one process-wide
/// `SecretsBackend` with no per-extension partition, so this predicate is the
/// entire isolation boundary between extensions; a declaration that names no
/// path segment is not a grant, it is a wildcard, and it is refused.
fn declaration_is_specific(declared: &str) -> bool {
    // Drop an optional `scheme:` prefix and any authority slashes, then require
    // something left over. Splitting on the first `:` rather than on `://` is
    // what catches `secrets:` — which has no authority, so it never matched
    // `://`, yet still lined a `/` up at the boundary offset and passed.
    let after_scheme = declared.split_once(':').map_or(declared, |(_, rest)| rest);
    !after_scheme
        .trim_start_matches('/')
        .trim_end_matches('/')
        .is_empty()
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
        //    `ExtensionRuntime::invoke_tool_with_depth(self: &Arc<Self>, ...)`
        //    which cascades the runtime API to `&Arc<Self>` and updates every
        //    caller. Deferred to a follow-up alongside the WASM broker
        //    fixtures. Today's permission + depth check is the
        //    security-load-bearing portion.
        Err(format!(
            "broker: dispatch to {target_id}.{function} is not implemented yet"
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::host_bindings::greentic::extension_host::i18n::Host as I18nHost;
    use crate::host_bindings::greentic::extension_host::secrets::Host as SecretsHost;
    use crate::host_ports::{InMemorySecrets, Translator};
    use greentic_extension_sdk_contract::describe::Permissions;
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

    fn host_with_secret(declared: &[&str], stored: &[(&str, &str)]) -> HostState {
        let backend = InMemorySecrets::default();
        for (k, v) in stored {
            backend.insert(k, v);
        }
        let mut perms = Permissions::default();
        perms
            .secrets
            .extend(declared.iter().map(|s| (*s).to_string()));
        HostState::builder("test-ext".to_string(), perms)
            .secrets_backend(Arc::new(backend))
            .build()
    }

    #[test]
    fn i18n_t_resolves_translated_value() {
        let mut h = host_with_translator(Arc::new(EnglishToIndonesian));
        assert_eq!(h.t("greentic.test.hello".to_string()), "Halo dunia");
    }

    #[test]
    fn i18n_t_falls_back_to_key_for_unknown() {
        let mut h = host_with_translator(Arc::new(EnglishToIndonesian));
        assert_eq!(h.t("missing.key".to_string()), "missing.key");
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

    #[test]
    fn secrets_get_returns_value_when_permitted() {
        let mut h = host_with_secret(
            &["api.openai.com/api_key"],
            &[("api.openai.com/api_key", "sk-real")],
        );
        let v = h.get("api.openai.com/api_key".to_string()).unwrap();
        assert_eq!(v, "sk-real");
    }

    #[test]
    fn secrets_get_denies_when_uri_not_in_permissions() {
        let mut h = host_with_secret(&[], &[("api.openai.com/api_key", "sk-real")]);
        let err = h.get("api.openai.com/api_key".to_string()).unwrap_err();
        assert!(
            err.contains("permission denied"),
            "expected permission denied, got: {err}"
        );
    }

    #[test]
    fn secrets_get_allows_a_declared_prefix_at_a_path_boundary() {
        let mut h = host_with_secret(&["team/acme"], &[("team/acme/openai", "sk-real")]);
        assert_eq!(h.get("team/acme/openai".to_string()).unwrap(), "sk-real");
    }

    #[test]
    fn secrets_get_denies_a_prefix_that_is_not_a_path_boundary() {
        // `team/acme` must not unlock `team/acme-evil/...`: without the `/`
        // boundary check a declared prefix silently covers every sibling key
        // that merely starts with the same characters.
        let mut h = host_with_secret(&["team/acme"], &[("team/acme-evil/openai", "sk-real")]);
        let err = h.get("team/acme-evil/openai".to_string()).unwrap_err();
        assert!(err.contains("permission denied"), "got: {err}");
    }

    #[test]
    fn a_declaration_naming_no_path_segment_grants_nothing() {
        // `""` and `"secrets:"` both satisfy the `/`-boundary test for an
        // entire namespace. Since one backend serves every extension with no
        // partition, honouring either would hand one extension every other
        // extension's secrets.
        for wildcard in ["", "secrets:", "secrets://", "/"] {
            let mut h = host_with_secret(&[wildcard], &[("secrets://team/openai", "sk-real")]);
            let err = h
                .get("secrets://team/openai".to_string())
                .expect_err("a declaration naming no path segment must grant nothing");
            assert!(
                err.contains("permission denied"),
                "{wildcard:?} was honoured as a grant: {err}"
            );
        }
    }

    #[test]
    fn a_scheme_qualified_declaration_still_works() {
        let mut h = host_with_secret(
            &["secrets://team/acme"],
            &[("secrets://team/acme/openai", "sk-real")],
        );
        assert_eq!(
            h.get("secrets://team/acme/openai".to_string()).unwrap(),
            "sk-real"
        );
    }

    #[test]
    fn secrets_get_surfaces_backend_not_found() {
        let mut h = host_with_secret(&["api.openai.com/api_key"], &[]);
        let err = h.get("api.openai.com/api_key".to_string()).unwrap_err();
        assert!(err.contains("not found"), "got: {err}");
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
}
