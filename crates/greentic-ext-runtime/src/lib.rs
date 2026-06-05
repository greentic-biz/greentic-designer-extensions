//! Wasmtime-based runtime for Greentic Designer Extensions.
#![forbid(unsafe_code)]

pub mod broker;
pub mod capability;
pub mod discovery;
mod error;
mod health;
mod host_bindings;
pub mod host_ports;
mod host_state;
mod loaded;
mod pool;
mod runtime;
mod runtime_deploy;
mod runtime_dw_composer;
mod runtime_roles;
pub mod types;
pub mod url_matcher;
pub mod watcher;

pub use self::broker::{Broker, BrokerError, BrokerResult};
pub use self::capability::{CapabilityRegistry, OfferedBinding, ResolutionPlan};
pub use self::discovery::DiscoveryPaths;
pub use self::error::RuntimeError;
pub use self::health::{ExtensionHealth, HealthReason};
pub use self::host_ports::{
    InMemorySecrets, KeyTranslator, SecretsBackend, SecretsError, Translator,
};
pub use self::host_state::HostState;
pub use self::loaded::{ExtensionId, HostOverrides, LoadedExtension, LoadedExtensionRef};
pub use self::runtime::{ExtensionRuntime, RuntimeConfig, RuntimeEvent, WatcherGuard};
pub use self::types::{
    BundleArtifact, BundleSession, CompileContext, DeployExtensionError, DeployJob, DeployRequest,
    DeployStatus, Diagnostic, HostExtensionError, KnowledgeEntry, KnowledgeEntrySummary,
    PromptFragment, RoleError, RoleSpec, Severity, TargetKind, TargetSummary, ToolDefinition,
    ValidateResult,
};
pub use self::url_matcher::UrlMatcher;

/// Re-export `reqwest` so consumers wiring `HostOverrides::http_client`
/// always construct the `Client` against the same crate version this
/// crate compiles against. Without this, a downstream crate that pulls
/// `reqwest` at a different semver (e.g. 0.13 vs the 0.12 we depend on)
/// produces two distinct `reqwest::blocking::Client` types and the
/// `http_client: Some(client)` assignment fails with a confusing
/// `expected reqwest::blocking::Client, found reqwest::blocking::Client`.
pub use reqwest;
