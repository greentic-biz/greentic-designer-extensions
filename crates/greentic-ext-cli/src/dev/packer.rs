//! `.gtxpack` builder: stages describe + wasm + assets into a ZIP.

use std::fs::File;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use walkdir::WalkDir;
use zip::write::SimpleFileOptions;

/// Summary of a packed `.gtxpack`.
#[derive(Debug, Clone)]
pub struct PackInfo {
    pub pack_path: PathBuf,
    pub pack_name: String,
    pub size: u64,
    pub sha256: String,
    pub ext_name: String,
    pub ext_version: String,
    pub ext_kind: String,
}

/// Build a `.gtxpack` at `output_pack` from `project_dir` + the already-built
/// `wasm_path`. The ZIP contains `describe.json`, the wasm renamed to
/// `extension.wasm` (matches the describe.json `runtime.component` default),
/// and any optional asset dirs that exist (`i18n/`, `schemas/`, `prompts/`).
pub fn build_pack(
    project_dir: &Path,
    wasm_path: &Path,
    output_pack: &Path,
) -> anyhow::Result<PackInfo> {
    let describe_path = project_dir.join("describe.json");
    let describe_bytes = std::fs::read(&describe_path)
        .map_err(|e| anyhow::anyhow!("read describe.json: {e}"))?;
    let describe: serde_json::Value = serde_json::from_slice(&describe_bytes)
        .map_err(|e| anyhow::anyhow!("parse describe.json: {e}"))?;
    let ext_name = describe["metadata"]["name"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("describe.metadata.name missing"))?
        .to_string();
    let ext_version = describe["metadata"]["version"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("describe.metadata.version missing"))?
        .to_string();
    let ext_kind = describe["kind"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("describe.kind missing"))?
        .to_string();

    if let Some(parent) = output_pack.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let file = File::create(output_pack)?;
    let mut zip = zip::ZipWriter::new(file);
    let opts = SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated)
        .unix_permissions(0o644);

    // 1) describe.json verbatim
    zip.start_file("describe.json", opts)?;
    zip.write_all(&describe_bytes)?;

    // 2) the wasm, renamed to extension.wasm
    zip.start_file("extension.wasm", opts)?;
    let mut wasm = File::open(wasm_path)?;
    std::io::copy(&mut wasm, &mut zip)?;

    // 3) optional asset dirs — sorted for deterministic output
    for asset_dir in ["i18n", "schemas", "prompts"] {
        let src = project_dir.join(asset_dir);
        if !src.is_dir() {
            continue;
        }
        let mut paths: Vec<PathBuf> = WalkDir::new(&src)
            .into_iter()
            .flatten()
            .filter(|e| e.file_type().is_file())
            .map(|e| e.path().to_path_buf())
            .collect();
        paths.sort();
        for abs in paths {
            let rel = abs
                .strip_prefix(project_dir)
                .expect("asset under project")
                .to_string_lossy()
                .replace('\\', "/");
            zip.start_file(rel, opts)?;
            let mut f = File::open(&abs)?;
            std::io::copy(&mut f, &mut zip)?;
        }
    }

    let writer = zip.finish()?;
    drop(writer);

    let size = std::fs::metadata(output_pack)?.len();
    let pack_name = output_pack
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("pack.gtxpack")
        .to_string();
    let sha256 = sha256_of_file(output_pack)?;

    Ok(PackInfo {
        pack_path: output_pack.to_path_buf(),
        pack_name,
        size,
        sha256,
        ext_name,
        ext_version,
        ext_kind,
    })
}

/// SHA-256 of a file as lowercase hex.
pub fn sha256_of_file(path: &Path) -> anyhow::Result<String> {
    use sha2::{Digest, Sha256};
    let mut file = File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 8192];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    let digest = hasher.finalize();
    let mut out = String::with_capacity(64);
    for byte in digest {
        use std::fmt::Write as _;
        write!(&mut out, "{byte:02x}")?;
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_project(root: &Path) -> PathBuf {
        let desc = r#"{
  "apiVersion": "greentic.ai/v1",
  "kind": "design",
  "metadata": {"id": "com.example.demo", "name": "demo", "version": "0.1.0", "description": "x", "authors": ["a"], "license": "Apache-2.0"},
  "engine": {"contract": "greentic:extension-design@0.1.0"},
  "capabilities": {"offered": [], "required": []},
  "permissions": {"network": [], "secrets": [], "callExtensionKinds": []},
  "runtime": {"component": "extension.wasm"}
}"#;
        std::fs::write(root.join("describe.json"), desc).unwrap();
        let wasm_dir = root.join("target/wasm32-wasip2/debug");
        std::fs::create_dir_all(&wasm_dir).unwrap();
        let wasm = wasm_dir.join("demo.wasm");
        std::fs::write(&wasm, b"\0asm\x01\x00\x00\x00").unwrap();
        wasm
    }

    #[test]
    fn build_pack_produces_zip_with_describe_and_wasm() {
        let tmp = tempfile::tempdir().unwrap();
        let wasm = make_project(tmp.path());
        let out = tmp.path().join("dist/demo-0.1.0.gtxpack");
        let info = build_pack(tmp.path(), &wasm, &out).expect("pack");
        assert_eq!(info.ext_name, "demo");
        assert_eq!(info.ext_version, "0.1.0");
        assert_eq!(info.ext_kind, "design");
        assert!(info.size > 0);

        let file = File::open(&out).unwrap();
        let mut zip = zip::ZipArchive::new(file).unwrap();
        let names: Vec<_> = (0..zip.len())
            .map(|i| zip.by_index(i).unwrap().name().to_string())
            .collect();
        assert!(names.iter().any(|n| n == "describe.json"));
        assert!(names.iter().any(|n| n == "extension.wasm"));
    }

    #[test]
    fn build_pack_includes_assets_when_present() {
        let tmp = tempfile::tempdir().unwrap();
        let wasm = make_project(tmp.path());
        std::fs::create_dir_all(tmp.path().join("i18n")).unwrap();
        std::fs::write(tmp.path().join("i18n/en.json"), br#"{"hello":"world"}"#).unwrap();
        let out = tmp.path().join("demo.gtxpack");
        build_pack(tmp.path(), &wasm, &out).unwrap();
        let file = File::open(&out).unwrap();
        let zip = zip::ZipArchive::new(file).unwrap();
        let names: Vec<_> = zip.file_names().map(str::to_string).collect();
        assert!(names.iter().any(|n| n == "i18n/en.json"));
    }

    #[test]
    fn sha256_of_file_is_lowercase_hex_64_chars() {
        let tmp = tempfile::tempdir().unwrap();
        let p = tmp.path().join("x");
        std::fs::write(&p, b"hello").unwrap();
        let h = sha256_of_file(&p).unwrap();
        assert_eq!(h.len(), 64);
        assert!(h.chars().all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()));
    }

    #[test]
    fn build_pack_errors_if_describe_missing() {
        let tmp = tempfile::tempdir().unwrap();
        let wasm_dir = tmp.path().join("target/wasm32-wasip2/debug");
        std::fs::create_dir_all(&wasm_dir).unwrap();
        std::fs::write(wasm_dir.join("x.wasm"), b"\0asm").unwrap();
        let out = tmp.path().join("out.gtxpack");
        let err = build_pack(tmp.path(), &wasm_dir.join("x.wasm"), &out).unwrap_err();
        assert!(err.to_string().contains("describe.json"));
    }
}
