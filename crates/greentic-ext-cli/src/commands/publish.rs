use std::path::{Path, PathBuf};

use clap::Args as ClapArgs;

#[derive(ClapArgs, Debug, Clone)]
#[allow(clippy::struct_excessive_bools)]
pub struct Args {
    /// Registry URI. `local` resolves to $GREENTIC_HOME/registries/local.
    /// Accepts file://<path> for explicit paths.
    #[arg(short = 'r', long, default_value = "local")]
    pub registry: String,

    /// Override describe.json version for this run (CI version bumps).
    #[arg(long)]
    pub version: Option<String>,

    /// Build + pack + validate; skip registry write.
    #[arg(long)]
    pub dry_run: bool,

    /// Sign .gtxpack with local key from ~/.greentic/keys/.
    #[arg(long)]
    pub sign: bool,

    /// Signing key id (requires --sign).
    #[arg(long)]
    pub key_id: Option<String>,

    /// loose | normal | strict
    #[arg(long, default_value = "loose")]
    pub trust: String,

    /// Copy artifact here as well.
    #[arg(long, default_value = "./dist")]
    pub dist: PathBuf,

    /// Overwrite existing version.
    #[arg(long)]
    pub force: bool,

    /// cargo component build --release (default true for publish).
    #[arg(long, default_value_t = true)]
    pub release: bool,

    /// Skip build; only check registry for version conflict.
    #[arg(long)]
    pub verify_only: bool,

    /// Path to the project's Cargo.toml.
    #[arg(long, default_value = "./Cargo.toml")]
    pub manifest: PathBuf,

    /// human | json
    #[arg(long, default_value = "human")]
    pub format: String,
}

pub async fn run(_args: Args, _home: &Path) -> anyhow::Result<()> {
    anyhow::bail!("gtdx publish is not yet implemented")
}
