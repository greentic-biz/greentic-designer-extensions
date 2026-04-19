//! `.gtxpack` builder: stages describe + wasm + assets and hands off to the
//! shared `greentic-ext-contract::pack_writer` for deterministic ZIP emission.

use std::path::{Path, PathBuf};

use greentic_ext_contract::pack_writer::{PackEntry, build_gtxpack, sha256_hex};
use walkdir::WalkDir;

/// Summary of a packed `.gtxpack`.
#[derive(Debug, Clone)]
pub struct PackInfo {
    pub pack_path: PathBuf,
    pub pack_name: String,
    pub size: u64,
    pub sha256: String,
    pub ext_name: String,
    pub ext_version: String,
    #[allow(dead_code)] // Reserved for richer InstallOk events in Phase 2.
    pub ext_kind: String,
}

/// Build a `.gtxpack` at `output_pack` from `project_dir` + the already-built
/// `wasm_path`. The ZIP contains `describe.json`, the wasm renamed to
/// `extension.wasm` (matches `runtime.component` default), and any optional
/// asset dirs that exist (`i18n/`, `schemas/`, `prompts/`).
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

    let mut entries = vec![
        PackEntry::file("describe.json", describe_bytes),
        PackEntry::file("extension.wasm", std::fs::read(wasm_path)?),
    ];

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
            entries.push(PackEntry::file(rel, std::fs::read(&abs)?));
        }
    }

    if let Some(parent) = output_pack.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let zip_bytes = build_gtxpack(entries)
        .map_err(|e| anyhow::anyhow!("build_gtxpack: {e}"))?;
    std::fs::write(output_pack, &zip_bytes)?;

    let size = u64::try_from(zip_bytes.len()).unwrap_or(u64::MAX);
    let pack_name = output_pack
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("pack.gtxpack")
        .to_string();
    let sha256 = sha256_hex(&zip_bytes);

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

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;

    fn make_project(root: &Path) -> PathBuf {
        let desc = br#"{
  "apiVersion": "greentic.ai/v1",
  "kind": "DesignExtension",
  "metadata": {"id": "com.example.demo", "name": "demo", "version": "0.1.0", "summary": "x", "author": {"name": "a"}, "license": "Apache-2.0"},
  "engine": {"greenticDesigner": "^0.1.0", "extRuntime": "^0.1.0"},
  "capabilities": {"offered": [], "required": []},
  "runtime": {"component": "extension.wasm", "permissions": {"network": [], "secrets": [], "callExtensionKinds": []}},
  "contributions": {}
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
        let info = build_pack(tmp.path(), &wasm, &out).unwrap();
        assert_eq!(info.ext_name, "demo");
        assert_eq!(info.ext_version, "0.1.0");
        assert_eq!(info.ext_kind, "DesignExtension");
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
    fn build_pack_is_deterministic_across_runs() {
        let tmp = tempfile::tempdir().unwrap();
        let wasm = make_project(tmp.path());
        let out1 = tmp.path().join("a.gtxpack");
        let out2 = tmp.path().join("b.gtxpack");
        let a = build_pack(tmp.path(), &wasm, &out1).unwrap();
        let b = build_pack(tmp.path(), &wasm, &out2).unwrap();
        assert_eq!(a.sha256, b.sha256);
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
