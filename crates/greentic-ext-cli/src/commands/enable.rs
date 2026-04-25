//! `gtdx enable <id>[@<version>]` — set an installed extension as enabled.

use anyhow::{Context, Result, anyhow};
use clap::Args;
use greentic_ext_state::ExtensionState;
use std::path::Path;

#[derive(Debug, Args)]
pub struct EnableArgs {
    /// Extension id, optionally with @version (e.g. greentic.foo@0.1.0).
    pub target: String,
}

pub fn run(args: &EnableArgs, home: &Path) -> Result<()> {
    let (id, version) = parse_target(&args.target, home)?;

    verify_installed(home, &id, &version)?;

    let mut state = ExtensionState::load(home).context("loading state")?;
    state.set_enabled(&id, &version, true);
    state.save_atomic(home).context("saving state")?;

    tracing::info!(ext_id = %id, version = %version, action = "enable", "extension state changed");
    println!("Enabled: {id}@{version} (designer will reload)");
    Ok(())
}

pub(crate) fn parse_target(target: &str, home: &Path) -> Result<(String, String)> {
    if let Some((id, ver)) = target.split_once('@') {
        return Ok((id.to_string(), ver.to_string()));
    }
    let versions = installed_versions(home, target)?;
    match versions.len() {
        0 => Err(anyhow!("extension not installed: {target}")),
        1 => Ok((target.to_string(), versions.into_iter().next().unwrap())),
        _ => Err(anyhow!(
            "ambiguous version for {target}: installed = [{}]. Specify with @<version>.",
            versions.join(", ")
        )),
    }
}

pub(crate) fn installed_versions(home: &Path, id: &str) -> Result<Vec<String>> {
    let mut out = vec![];
    for kind in ["design", "deploy", "bundle", "provider"] {
        let dir = home.join("extensions").join(kind);
        if !dir.exists() {
            continue;
        }
        for entry in std::fs::read_dir(&dir)? {
            let entry = entry?;
            let name = entry.file_name().into_string().unwrap_or_default();
            if let Some(rest) = name.strip_prefix(&format!("{id}-")) {
                out.push(rest.to_string());
            }
        }
    }
    out.sort();
    out.dedup();
    Ok(out)
}

pub(crate) fn verify_installed(home: &Path, id: &str, version: &str) -> Result<()> {
    let suffix = format!("{id}-{version}");
    for kind in ["design", "deploy", "bundle", "provider"] {
        if home.join("extensions").join(kind).join(&suffix).exists() {
            return Ok(());
        }
    }
    Err(anyhow!("extension not installed: {id}@{version}"))
}
