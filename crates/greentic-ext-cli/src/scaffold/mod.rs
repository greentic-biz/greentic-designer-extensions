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
}

impl Kind {
    #[allow(dead_code)] // used by subsequent scaffolding tasks (T5, T12-T14)
    pub fn as_str(self) -> &'static str {
        match self {
            Kind::Design => "design",
            Kind::Bundle => "bundle",
            Kind::Deploy => "deploy",
        }
    }
}
