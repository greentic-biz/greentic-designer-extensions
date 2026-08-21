//! The TOFU trust root must resolve exactly the way `gtdx` resolves it, or
//! the anchored-verify check pins into a store nobody else populates.
//!
//! `gtdx` uses `--home` / `$GREENTIC_HOME`, else `~/.greentic`
//! (`greentic-extension-sdk-cli/src/main.rs:116-124`). Deriving the root from
//! `DiscoveryPaths::home()` instead would agree only by coincidence today and
//! go wrong-but-plausible under either override — a silent re-pin into a
//! *different* store.

#[path = "support/mod.rs"]
mod support;

use std::path::PathBuf;

use greentic_ext_runtime::{DiscoveryPaths, RuntimeConfig};

use support::EnvGuard;

fn config() -> RuntimeConfig {
    RuntimeConfig::from_paths(DiscoveryPaths::new(PathBuf::from(
        "/nonexistent/extensions-root",
    )))
}

#[test]
fn trust_root_prefers_env_then_home() {
    // Arm 1: an explicit override wins over everything, including the env var.
    {
        let _guard = EnvGuard::set("GREENTIC_HOME", "/env/greentic");
        let cfg = config().with_trust_root(PathBuf::from("/explicit/root"));
        assert_eq!(
            cfg.resolve_trust_root().unwrap(),
            PathBuf::from("/explicit/root"),
            "an explicit with_trust_root() override must beat $GREENTIC_HOME"
        );
    }

    // Arm 2: no override → $GREENTIC_HOME, used verbatim (gtdx's `--home` is
    // the greentic home itself, not a parent of it).
    {
        let _guard = EnvGuard::set("GREENTIC_HOME", "/env/greentic");
        assert_eq!(
            config().resolve_trust_root().unwrap(),
            PathBuf::from("/env/greentic"),
            "$GREENTIC_HOME must be honoured verbatim, mirroring gtdx"
        );
    }

    // Arm 3: neither → ~/.greentic.
    {
        let _guard = EnvGuard::remove("GREENTIC_HOME");
        let expected = directories::BaseDirs::new()
            .expect("test host must have a home directory")
            .home_dir()
            .join(".greentic");
        assert_eq!(
            config().resolve_trust_root().unwrap(),
            expected,
            "with no override and no env var the root must be ~/.greentic"
        );
    }
}

/// The trust root must NOT be derived from the extension discovery paths.
/// `DiscoveryPaths::home()` (i.e. `user.parent()`) happens to equal
/// `~/.greentic` in the default layout, but under the runner's
/// `GREENTIC_EXTENSIONS_DIR` override it becomes the parent of an arbitrary
/// directory — which would silently pin into a store gtdx never reads.
#[test]
fn trust_root_ignores_discovery_paths() {
    let _guard = EnvGuard::remove("GREENTIC_HOME");
    let cfg = RuntimeConfig::from_paths(DiscoveryPaths::new(PathBuf::from(
        "/somewhere/else/entirely/extensions",
    )));
    let resolved = cfg.resolve_trust_root().unwrap();
    assert!(
        !resolved.starts_with("/somewhere/else/entirely"),
        "trust root must not be derived from DiscoveryPaths; got {}",
        resolved.display()
    );
}
