use std::path::Path;

use clap::Args as ClapArgs;
use greentic_ext_contract::ExtensionKind;
use greentic_ext_registry::storage::Storage;

#[derive(ClapArgs, Debug)]
pub struct Args;

pub fn run(_args: Args, home: &Path) -> anyhow::Result<()> {
    let storage = Storage::new(home);
    let mut total = 0;
    let mut bad = 0;
    for kind in [
        ExtensionKind::Design,
        ExtensionKind::Bundle,
        ExtensionKind::Deploy,
    ] {
        let dir = storage.kind_dir(kind);
        if !dir.exists() {
            continue;
        }
        for entry in std::fs::read_dir(&dir)? {
            let entry = entry?;
            if !entry.file_type()?.is_dir() {
                continue;
            }
            total += 1;
            let describe_path = entry.path().join("describe.json");
            if !describe_path.exists() {
                println!("✗ {} (no describe.json)", entry.path().display());
                bad += 1;
                continue;
            }
            let bytes = std::fs::read(&describe_path)?;
            let value: serde_json::Value = match serde_json::from_slice(&bytes) {
                Ok(v) => v,
                Err(e) => {
                    println!("✗ {}: invalid JSON: {e}", describe_path.display());
                    bad += 1;
                    continue;
                }
            };
            if let Err(e) = greentic_ext_contract::schema::validate_describe_json(&value) {
                println!("✗ {}: {e}", describe_path.display());
                bad += 1;
            } else {
                println!("✓ {}", describe_path.display());
            }
        }
    }
    println!();
    println!("{total} total, {bad} bad");
    if bad > 0 {
        std::process::exit(1);
    }
    Ok(())
}
