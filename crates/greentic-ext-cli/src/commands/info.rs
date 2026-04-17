use std::path::Path;

use clap::Args as ClapArgs;
use greentic_ext_registry::{ExtensionRegistry, store::GreenticStoreRegistry};

#[derive(ClapArgs, Debug)]
pub struct Args {
    pub name: String,
    #[arg(long)]
    pub version: Option<String>,
    #[arg(long)]
    pub registry: Option<String>,
}

pub async fn run(args: Args, home: &Path) -> anyhow::Result<()> {
    let cfg = super::load_config(home)?;
    let reg_name = args.registry.as_deref().unwrap_or(&cfg.default.registry);
    let entry = cfg
        .registries
        .iter()
        .find(|r| r.name == reg_name)
        .ok_or_else(|| anyhow::anyhow!("no such registry: {reg_name}"))?;
    let reg = GreenticStoreRegistry::new(&entry.name, &entry.url, None);

    let versions = reg.list_versions(&args.name).await?;
    let version = match args.version {
        Some(v) => v,
        None => versions
            .last()
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("no versions published"))?,
    };
    let meta = reg.metadata(&args.name, &version).await?;
    println!("name:     {}", meta.name);
    println!("version:  {}", meta.version);
    println!("kind:     {:?}", meta.describe.kind);
    println!("license:  {}", meta.describe.metadata.license);
    println!("summary:  {}", meta.describe.metadata.summary);
    println!("sha256:   {}", meta.artifact_sha256);
    println!("versions:");
    for v in versions {
        println!("  {v}");
    }
    Ok(())
}
