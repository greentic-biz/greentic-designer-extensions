use std::path::Path;

use clap::Args as ClapArgs;
use greentic_ext_contract::ExtensionKind;
use greentic_ext_registry::storage::Storage;

#[derive(ClapArgs, Debug)]
pub struct Args;

pub fn run(_args: Args, home: &Path) -> anyhow::Result<()> {
    let storage = Storage::new(home);
    for kind in [
        ExtensionKind::Design,
        ExtensionKind::Bundle,
        ExtensionKind::Deploy,
    ] {
        let dir = storage.kind_dir(kind);
        if !dir.exists() {
            continue;
        }
        let mut any = false;
        for entry in std::fs::read_dir(&dir)? {
            let entry = entry?;
            if !entry.file_type()?.is_dir() {
                continue;
            }
            let describe_path = entry.path().join("describe.json");
            if !describe_path.exists() {
                continue;
            }
            let bytes = std::fs::read(&describe_path)?;
            let value: serde_json::Value = serde_json::from_slice(&bytes)?;
            let d: greentic_ext_contract::DescribeJson = serde_json::from_value(value)?;
            if !any {
                println!("[{}]", kind.dir_name());
                any = true;
            }
            println!(
                "  {}@{}  {}",
                d.metadata.id, d.metadata.version, d.metadata.summary
            );
        }
    }
    Ok(())
}
