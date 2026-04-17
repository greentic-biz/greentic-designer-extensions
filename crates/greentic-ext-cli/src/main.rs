use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "gtdx", version, about = "Greentic Designer Extensions CLI")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Validate an extension directory against the describe.json schema
    Validate {
        #[arg(default_value = ".")]
        path: String,
    },
    /// List installed extensions (placeholder — Plan 2)
    List,
    /// Print version
    Version,
}

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let cli = Cli::parse();
    match cli.command {
        Command::Validate { path } => {
            let describe_path = std::path::Path::new(&path).join("describe.json");
            let bytes = std::fs::read(&describe_path)?;
            let value: serde_json::Value = serde_json::from_slice(&bytes)?;
            greentic_ext_contract::schema::validate_describe_json(&value)
                .map_err(|e| anyhow::anyhow!("{e}"))?;
            println!("✓ {} valid", describe_path.display());
        }
        Command::List => {
            println!("Extension listing not yet implemented (see Plan 2)");
        }
        Command::Version => {
            println!("gtdx {}", env!("CARGO_PKG_VERSION"));
        }
    }
    Ok(())
}
