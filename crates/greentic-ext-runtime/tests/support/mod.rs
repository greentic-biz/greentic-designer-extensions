//! Shared test helpers for runtime integration tests.
//!
//! Tests must not run in parallel when mutating process environment.
//! The `EnvGuard::set` guard serializes via a global Mutex.
#![allow(dead_code)]

use std::sync::{Mutex, MutexGuard, OnceLock};

fn env_mutex() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

pub struct EnvGuard {
    key: String,
    prev: Option<String>,
    _lock: MutexGuard<'static, ()>,
}

impl EnvGuard {
    pub fn set(key: &str, value: &str) -> Self {
        let lock = env_mutex()
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let prev = std::env::var(key).ok();
        // SAFETY: serialized via global mutex; we hold the lock for guard lifetime.
        #[allow(unsafe_code)]
        unsafe {
            std::env::set_var(key, value);
        }
        EnvGuard {
            key: key.to_string(),
            prev,
            _lock: lock,
        }
    }

    /// Remove an env var for the lifetime of the guard, holding the global
    /// mutex for exclusive access. On drop, restore the previous value (if any).
    pub fn remove(key: &str) -> Self {
        let lock = env_mutex()
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let prev = std::env::var(key).ok();
        // SAFETY: serialized via global mutex; we hold the lock for guard lifetime.
        #[allow(unsafe_code)]
        unsafe {
            std::env::remove_var(key);
        }
        EnvGuard {
            key: key.to_string(),
            prev,
            _lock: lock,
        }
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        #[allow(unsafe_code)]
        unsafe {
            match &self.prev {
                Some(v) => std::env::set_var(&self.key, v),
                None => std::env::remove_var(&self.key),
            }
        }
    }
}

/// Build a signed extension fixture using the `ExtensionFixtureBuilder`
/// from `greentic-extension-sdk-testing`, then sign its describe.json with a fresh
/// ed25519 key. Returns the fixture and the signing key used.
pub fn signed_fixture(
    kind: greentic_extension_sdk_contract::ExtensionKind,
    id: &str,
    version: &str,
) -> (
    greentic_extension_sdk_testing::ExtensionFixture,
    ed25519_dalek::SigningKey,
) {
    use ed25519_dalek::SigningKey;
    use rand::rngs::OsRng;

    let minimal_wasm = wat::parse_str(r"(component)").expect("wat component must compile");
    let fixture = greentic_extension_sdk_testing::ExtensionFixtureBuilder::new(kind, id, version)
        .offer("greentic:test/ping", "1.0.0")
        .with_wasm(minimal_wasm)
        .build()
        .expect("fixture build");

    // Read, sign, write back.
    let describe_path = fixture.root().join("describe.json");
    let raw = std::fs::read_to_string(&describe_path).unwrap();
    let mut describe: greentic_extension_sdk_contract::DescribeJson = serde_json::from_str(&raw).unwrap();
    let sk = SigningKey::generate(&mut OsRng);
    greentic_extension_sdk_contract::sign_describe(&mut describe, &sk).expect("sign");
    let out = serde_json::to_string_pretty(&describe).unwrap();
    std::fs::write(&describe_path, out).unwrap();

    (fixture, sk)
}

/// Mutate an installed fixture's describe.json to invalidate its signature.
pub fn tamper_fixture(fixture: &greentic_extension_sdk_testing::ExtensionFixture) {
    let path = fixture.root().join("describe.json");
    let raw = std::fs::read_to_string(&path).unwrap();
    let mut describe: greentic_extension_sdk_contract::DescribeJson = serde_json::from_str(&raw).unwrap();
    describe.metadata.version = "99.99.99".into();
    std::fs::write(&path, serde_json::to_string_pretty(&describe).unwrap()).unwrap();
}

/// Build an **unsigned** fixture (no .signature field). Mirrors existing
/// `ExtensionFixtureBuilder` default output.
pub fn unsigned_fixture(
    kind: greentic_extension_sdk_contract::ExtensionKind,
    id: &str,
    version: &str,
) -> greentic_extension_sdk_testing::ExtensionFixture {
    let minimal_wasm = wat::parse_str(r"(component)").expect("wat component must compile");
    greentic_extension_sdk_testing::ExtensionFixtureBuilder::new(kind, id, version)
        .offer("greentic:test/ping", "1.0.0")
        .with_wasm(minimal_wasm)
        .build()
        .expect("fixture build")
}
