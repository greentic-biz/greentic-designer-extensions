//! Inner-loop dev command: rebuild -> pack -> install on source change.

pub mod builder;
pub mod event;
pub mod installer;
pub mod packer;
pub mod state;
pub mod watcher;

use std::path::{Path, PathBuf};
use std::time::Duration;

use self::builder::{Profile, run_build};
use self::event::{DevEvent, Emitter};
use self::installer::install_pack;
use self::packer::build_pack;

/// Runtime parameters, resolved from `commands::dev::Args`.
#[derive(Debug, Clone)]
pub struct DevConfig {
    pub project_dir: PathBuf,
    pub home: PathBuf,
    pub profile: Profile,
    pub install: bool,
    pub debounce: Duration,
}

/// Resolve `Cargo.toml` path to the project root (its parent dir).
pub fn project_dir_from_manifest(manifest: &Path) -> anyhow::Result<PathBuf> {
    let canonical = std::fs::canonicalize(manifest)
        .map_err(|e| anyhow::anyhow!("canonicalize {}: {e}", manifest.display()))?;
    canonical
        .parent()
        .map(Path::to_path_buf)
        .ok_or_else(|| anyhow::anyhow!("manifest has no parent dir: {}", canonical.display()))
}

/// Perform a single build -> pack -> install cycle.
pub async fn run_once(cfg: &DevConfig, out: &mut dyn Emitter) -> anyhow::Result<()> {
    out.emit(&DevEvent::BuildStart {
        profile: cfg.profile.as_str().into(),
    });
    let build = match run_build(&cfg.project_dir, cfg.profile) {
        Ok(b) => b,
        Err(e) => {
            out.emit(&DevEvent::BuildFailed { duration_ms: 0 });
            return Err(e);
        }
    };
    out.emit(&DevEvent::BuildOk {
        duration_ms: build.duration_ms,
        wasm_size: build.wasm_size,
    });

    let dist = cfg.project_dir.join("dist");
    std::fs::create_dir_all(&dist)?;
    let out_pack = dist.join("dev.gtxpack");
    let info = build_pack(&cfg.project_dir, &build.wasm_path, &out_pack)?;
    let final_pack = dist.join(format!("{}-{}.gtxpack", info.ext_name, info.ext_version));
    let info = if final_pack != info.pack_path {
        if final_pack.exists() {
            std::fs::remove_file(&final_pack)?;
        }
        std::fs::rename(&info.pack_path, &final_pack)?;
        packer::PackInfo {
            pack_path: final_pack,
            pack_name: format!("{}-{}.gtxpack", info.ext_name, info.ext_version),
            ..info
        }
    } else {
        info
    };
    out.emit(&DevEvent::PackOk {
        pack_name: info.pack_name.clone(),
        size: info.size,
    });

    if !cfg.install {
        out.emit(&DevEvent::InstallSkipped {
            reason: "--no-install".into(),
        });
        out.emit(&DevEvent::Idle { last_build_ok: true });
        return Ok(());
    }

    match install_pack(&cfg.home, &info).await {
        Ok(summary) => {
            out.emit(&DevEvent::InstallOk {
                registry: summary.registry.display().to_string(),
                version: summary.version,
            });
            out.emit(&DevEvent::Idle { last_build_ok: true });
            Ok(())
        }
        Err(e) => {
            out.emit(&DevEvent::Error {
                message: format!("install failed: {e}"),
            });
            out.emit(&DevEvent::Idle { last_build_ok: false });
            Err(e)
        }
    }
}
