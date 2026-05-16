//! D.4.runtime: ext-runtime now consults `manifest.json` (when present)
//! during install. Older packs without a manifest still load (fail-open
//! during transition); newer packs must hash-match.

#[path = "support/mod.rs"]
mod support;

use std::path::PathBuf;

use greentic_ext_runtime::{DiscoveryPaths, ExtensionRuntime, RuntimeConfig, RuntimeError};
use greentic_extension_sdk_contract::ExtensionKind;
use sha2::{Digest, Sha256};

use support::{EnvGuard, signed_fixture};

fn new_runtime() -> ExtensionRuntime {
    let config = RuntimeConfig::from_paths(DiscoveryPaths::new(PathBuf::from("/dev/null")));
    ExtensionRuntime::new(config).unwrap()
}

/// Walk the unpacked fixture, build a manifest from the actual files, and
/// write it to `manifest.json` under the same dir. Mirrors what `gtdx`'s
/// post-D.4.2 packer would have emitted at pack time.
fn write_manifest_for_dir(dir: &std::path::Path) {
    let mut entries: Vec<(String, Vec<u8>)> = Vec::new();
    for path in walkdir(dir) {
        let rel = path
            .strip_prefix(dir)
            .unwrap()
            .to_string_lossy()
            .replace('\\', "/");
        if rel == greentic_extension_sdk_contract::MANIFEST_ENTRY_NAME || rel.is_empty() {
            continue;
        }
        entries.push((rel, std::fs::read(&path).unwrap()));
    }
    let manifest = greentic_extension_sdk_contract::build_manifest(
        entries.iter().map(|(p, b)| (p.as_str(), b.as_slice())),
    );
    let bytes = serde_jcs::to_vec(&manifest).unwrap();
    std::fs::write(
        dir.join(greentic_extension_sdk_contract::MANIFEST_ENTRY_NAME),
        bytes,
    )
    .unwrap();
}

fn walkdir(p: &std::path::Path) -> Vec<std::path::PathBuf> {
    let mut out = Vec::new();
    for ent in std::fs::read_dir(p).unwrap().flatten() {
        let path = ent.path();
        if path.is_dir() {
            out.extend(walkdir(&path));
        } else {
            out.push(path);
        }
    }
    out
}

#[test]
fn legacy_pack_without_manifest_still_loads() {
    let _guard = EnvGuard::remove("GREENTIC_EXT_ALLOW_UNSIGNED");
    let (fx, _sk) = signed_fixture(ExtensionKind::Design, "greentic.legacy", "0.1.0");
    let mut rt = new_runtime();
    rt.register_loaded_from_dir(fx.root())
        .expect("legacy pack (no manifest.json) must still load");
}

#[test]
fn pack_with_intact_manifest_loads() {
    let _guard = EnvGuard::remove("GREENTIC_EXT_ALLOW_UNSIGNED");
    let (fx, _sk) = signed_fixture(ExtensionKind::Design, "greentic.with-manifest", "0.1.0");
    write_manifest_for_dir(fx.root());
    let mut rt = new_runtime();
    rt.register_loaded_from_dir(fx.root())
        .expect("intact manifest must verify");
}

#[test]
fn pack_with_tampered_wasm_after_manifest_is_rejected() {
    let _guard = EnvGuard::remove("GREENTIC_EXT_ALLOW_UNSIGNED");
    let (fx, _sk) = signed_fixture(ExtensionKind::Design, "greentic.tampered-wasm", "0.1.0");
    write_manifest_for_dir(fx.root());
    let wasm_path = fx.root().join("extension.wasm");
    let mut bytes = std::fs::read(&wasm_path).unwrap();
    bytes.push(0xff);
    std::fs::write(&wasm_path, bytes).unwrap();
    let mut rt = new_runtime();
    let err = rt.register_loaded_from_dir(fx.root()).unwrap_err();
    match err {
        RuntimeError::SignatureInvalid { reason, .. } => {
            assert!(
                reason.contains("manifest sha256 mismatch"),
                "unexpected reason: {reason}",
            );
        }
        other => panic!("expected SignatureInvalid, got {other:?}"),
    }
}

#[test]
fn pack_with_manifest_listing_missing_file_is_rejected() {
    let _guard = EnvGuard::remove("GREENTIC_EXT_ALLOW_UNSIGNED");
    let (fx, _sk) = signed_fixture(ExtensionKind::Design, "greentic.missing-file", "0.1.0");
    write_manifest_for_dir(fx.root());
    let manifest_path = fx
        .root()
        .join(greentic_extension_sdk_contract::MANIFEST_ENTRY_NAME);
    let raw = std::fs::read(&manifest_path).unwrap();
    let mut manifest: greentic_extension_sdk_contract::Manifest =
        serde_json::from_slice(&raw).unwrap();
    let phantom_hash = format!("{:x}", Sha256::digest(b"ghost"));
    manifest
        .entries
        .push(greentic_extension_sdk_contract::ManifestEntry {
            path: "ghost.txt".to_string(),
            sha256: phantom_hash,
            size: 5,
        });
    let bytes = serde_jcs::to_vec(&manifest).unwrap();
    std::fs::write(&manifest_path, bytes).unwrap();

    let mut rt = new_runtime();
    let err = rt.register_loaded_from_dir(fx.root()).unwrap_err();
    match err {
        RuntimeError::SignatureInvalid { reason, .. } => {
            assert!(
                reason.contains("manifest lists missing file"),
                "unexpected reason: {reason}",
            );
        }
        other => panic!("expected SignatureInvalid, got {other:?}"),
    }
}

#[test]
fn pack_with_unsupported_manifest_schema_is_rejected() {
    let _guard = EnvGuard::remove("GREENTIC_EXT_ALLOW_UNSIGNED");
    let (fx, _sk) = signed_fixture(ExtensionKind::Design, "greentic.bad-schema", "0.1.0");
    let manifest_path = fx
        .root()
        .join(greentic_extension_sdk_contract::MANIFEST_ENTRY_NAME);
    let bogus = serde_json::json!({
        "schema": "greentic.gtxpack.manifest/v999",
        "entries": [],
    });
    std::fs::write(&manifest_path, serde_json::to_vec(&bogus).unwrap()).unwrap();

    let mut rt = new_runtime();
    let err = rt.register_loaded_from_dir(fx.root()).unwrap_err();
    match err {
        RuntimeError::SignatureInvalid { reason, .. } => {
            assert!(
                reason.contains("manifest schema unsupported"),
                "unexpected reason: {reason}",
            );
        }
        other => panic!("expected SignatureInvalid, got {other:?}"),
    }
}
