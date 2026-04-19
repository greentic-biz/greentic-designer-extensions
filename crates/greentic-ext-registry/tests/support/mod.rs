#![allow(dead_code)]

use std::io::Write as _;
use std::path::{Path, PathBuf};

use greentic_ext_contract::{
    DescribeJson, ExtensionKind, RuntimeGtpack,
    describe::{Author, Capabilities, Engine, Metadata, Permissions, Runtime},
};
use sha2::{Digest, Sha256};
use zip::write::SimpleFileOptions;

/// Inline hex encoder — avoids pulling in the `hex` crate as a dev-dep.
fn hex_encode(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    bytes
        .iter()
        .fold(String::with_capacity(bytes.len() * 2), |mut acc, b| {
            let _ = write!(acc, "{b:02x}");
            acc
        })
}

/// Build a minimal `.gtxpack` ZIP containing `describe.json` and the embedded
/// `.gtpack` file at `runtime/provider.gtpack`.
///
/// The describe.json is serialized from a real `DescribeJson` struct so it
/// always round-trips correctly.
pub fn build_provider_fixture_gtxpack(
    staging_root: &Path,
    id: &str,
    version: &str,
    gtpack_bytes: &[u8],
    sha256: &str,
) -> PathBuf {
    let out = staging_root.join(format!("{id}-{version}.gtxpack"));
    let file = std::fs::File::create(&out).unwrap();
    let mut zip = zip::ZipWriter::new(file);
    let opts = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);

    // Build a valid DescribeJson so serde round-trips correctly.
    let describe = DescribeJson {
        schema_ref: None,
        api_version: "greentic.ai/v1".into(),
        kind: ExtensionKind::Provider,
        metadata: Metadata {
            id: id.into(),
            name: id.into(),
            version: version.into(),
            summary: "fixture".into(),
            description: None,
            author: Author {
                name: "Fixture".into(),
                email: None,
                public_key: None,
            },
            license: "MIT".into(),
            homepage: None,
            repository: None,
            keywords: vec![],
            icon: None,
            screenshots: vec![],
        },
        engine: Engine {
            greentic_designer: "*".into(),
            ext_runtime: "^0.1.0".into(),
        },
        capabilities: Capabilities {
            offered: vec![],
            required: vec![],
        },
        runtime: Runtime {
            component: "wasm/stub.wasm".into(),
            memory_limit_mb: 64,
            permissions: Permissions::default(),
            gtpack: Some(RuntimeGtpack {
                file: "runtime/provider.gtpack".into(),
                sha256: sha256.into(),
                pack_id: id.into(),
                component_version: "0.6.0".into(),
            }),
        },
        contributions: serde_json::json!({}),
        signature: None,
    };

    // Write describe.json
    zip.start_file("describe.json", opts).unwrap();
    zip.write_all(serde_json::to_string_pretty(&describe).unwrap().as_bytes())
        .unwrap();

    // Write stub wasm (empty bytes are fine — component path is just a string)
    zip.start_file("wasm/stub.wasm", opts).unwrap();
    zip.write_all(b"").unwrap();

    // Write the embedded gtpack
    zip.start_file("runtime/provider.gtpack", opts).unwrap();
    zip.write_all(gtpack_bytes).unwrap();

    zip.finish().unwrap();
    out
}

/// Compute a valid SHA-256 hex string for the given bytes.
pub fn sha256_hex(bytes: &[u8]) -> String {
    hex_encode(&Sha256::digest(bytes))
}

/// Build a minimal `.gtpack` ZIP whose `manifest.cbor` entry contains
/// `{"pack_id": pack_id, "version": "0.1.0"}` encoded in CBOR.
pub fn encode_gtpack_with_pack_id(pack_id: &str) -> Vec<u8> {
    let manifest = serde_json::json!({ "pack_id": pack_id, "version": "0.1.0" });
    let mut cbor_bytes = Vec::new();
    ciborium::into_writer(&manifest, &mut cbor_bytes).unwrap();

    let mut buf = Vec::new();
    {
        let cursor = std::io::Cursor::new(&mut buf);
        let mut zip = zip::ZipWriter::new(cursor);
        let opts = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
        zip.start_file("manifest.cbor", opts).unwrap();
        zip.write_all(&cbor_bytes).unwrap();
        zip.finish().unwrap();
    }
    buf
}
