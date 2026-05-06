//! Wasmtime-based runtime for Greentic Designer Extensions.

pub mod broker;
pub mod capability;
pub mod discovery;
mod error;
mod health;
mod host_bindings;
mod host_state;
mod loaded;
mod pool;
mod runtime;
mod runtime_roles;
pub mod types;
pub mod watcher;

pub use self::broker::{Broker, BrokerError, BrokerResult};
pub use self::capability::{CapabilityRegistry, OfferedBinding, ResolutionPlan};
pub use self::discovery::DiscoveryPaths;
pub use self::error::RuntimeError;
pub use self::health::{ExtensionHealth, HealthReason};
pub use self::host_state::HostState;
pub use self::loaded::{ExtensionId, LoadedExtension, LoadedExtensionRef};
pub use self::runtime::{ExtensionRuntime, RuntimeConfig, RuntimeEvent, WatcherGuard};
pub use self::types::{
    BundleArtifact, BundleSession, CompileContext, Diagnostic, HostExtensionError, KnowledgeEntry,
    KnowledgeEntrySummary, PromptFragment, RoleError, RoleSpec, Severity, TargetKind,
    TargetSummary, ToolDefinition, ValidateResult,
};
