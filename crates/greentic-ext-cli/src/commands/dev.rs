use std::path::{Path, PathBuf};

use clap::Args as ClapArgs;

#[derive(ClapArgs, Debug, Clone)]
pub struct Args {
    /// Build + install once, then exit (CI-friendly)
    #[arg(long, conflicts_with = "watch")]
    pub once: bool,

    /// Continuous watch mode (default)
    #[arg(long)]
    pub watch: bool,

    /// Target registry dir; defaults to $GREENTIC_HOME (installs via Installer)
    #[arg(long)]
    pub install_to: Option<PathBuf>,

    /// Build and pack only; skip installation
    #[arg(long)]
    pub no_install: bool,

    /// Build with --release (default: debug for speed)
    #[arg(long)]
    pub release: bool,

    /// Override describe.json id for this run (multi-variant dev)
    #[arg(long)]
    pub ext_id: Option<String>,

    /// File-watch debounce window
    #[arg(long, default_value_t = default_debounce_ms())]
    pub debounce_ms: u64,

    /// Log filter level
    #[arg(long, default_value = "info")]
    pub log: String,

    /// Path to the project's Cargo.toml
    #[arg(long, default_value = "./Cargo.toml")]
    pub manifest: PathBuf,

    /// Force a full rebuild by ignoring cargo cache (`--offline` off; does `cargo clean -p <crate>` first)
    #[arg(long)]
    pub force_rebuild: bool,

    /// Output format: human | json
    #[arg(long, default_value = "human")]
    pub format: String,
}

fn default_debounce_ms() -> u64 {
    if cfg!(windows) { 1000 } else { 500 }
}

pub async fn run(_args: Args, _home: &Path) -> anyhow::Result<()> {
    anyhow::bail!("gtdx dev is not yet implemented")
}
