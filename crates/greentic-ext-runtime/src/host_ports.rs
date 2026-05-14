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
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, key: &str, value: &str) {
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
        let out = t.tf("greentic.test.hello.{}", &[("name", "Bima")]);
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
