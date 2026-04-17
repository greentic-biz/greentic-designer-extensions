//! Wasmtime-based runtime for Greentic Designer Extensions.

pub mod broker;
pub mod capability;
pub mod discovery;
mod error;
mod health;
mod loaded;
mod pool;
mod runtime;
pub mod watcher;

pub use self::broker::{Broker, BrokerError, BrokerResult};
pub use self::capability::{CapabilityRegistry, OfferedBinding, ResolutionPlan};
pub use self::discovery::DiscoveryPaths;
pub use self::error::RuntimeError;
pub use self::health::{ExtensionHealth, HealthReason};
pub use self::loaded::{ExtensionId, LoadedExtension, LoadedExtensionRef};
pub use self::runtime::{ExtensionRuntime, RuntimeConfig, RuntimeEvent, WatcherGuard};
