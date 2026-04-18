use std::path::{Path, PathBuf};

use clap::Args as ClapArgs;

use crate::scaffold::Kind;

#[derive(ClapArgs, Debug)]
pub struct Args {
    /// Project folder name (kebab-case). Also default id suffix.
    pub name: String,

    /// Extension kind
    #[arg(short = 'k', long, value_enum, default_value = "design")]
    pub kind: Kind,

    /// Extension id (reverse-DNS). Default: com.example.<name>
    #[arg(short = 'i', long)]
    pub id: Option<String>,

    /// Initial version
    #[arg(short = 'v', long, default_value = "0.1.0")]
    pub version: String,

    /// Author name; defaults to git config user.name
    #[arg(long)]
    pub author: Option<String>,

    /// SPDX license id
    #[arg(long, default_value = "Apache-2.0")]
    pub license: String,

    /// Skip `git init`
    #[arg(long)]
    pub no_git: bool,

    /// Output directory; defaults to ./<name>
    #[arg(long)]
    pub dir: Option<PathBuf>,

    /// Overwrite if target exists
    #[arg(long)]
    pub force: bool,

    /// Skip interactive prompts
    #[arg(short = 'y', long)]
    pub yes: bool,
}

pub fn run(args: &Args, _home: &Path) -> anyhow::Result<()> {
    anyhow::bail!(
        "gtdx new is not yet implemented (name={}, kind={:?})",
        args.name,
        args.kind
    )
}
