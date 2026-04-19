//! Scaffolding logic for `gtdx new`.

pub mod contract_lock;
pub mod embedded;
pub mod preflight;
pub mod template;

/// Extension kinds that can be scaffolded.
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum Kind {
    Design,
    Bundle,
    Deploy,
    Provider,
}

impl Kind {
    pub fn as_str(self) -> &'static str {
        match self {
            Kind::Design => "design",
            Kind::Bundle => "bundle",
            Kind::Deploy => "deploy",
            Kind::Provider => "provider",
        }
    }
}
