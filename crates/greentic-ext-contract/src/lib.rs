//! Contract types + describe.json schema for Greentic Designer Extensions.

pub mod capability;
pub mod describe;
pub mod error;
pub mod kind;
pub mod schema;
pub mod signature;

pub use self::capability::{CapabilityId, CapabilityRef, CapabilityVersion};
pub use self::describe::DescribeJson;
pub use self::error::ContractError;
pub use self::kind::ExtensionKind;
pub use self::signature::{artifact_sha256, sign_ed25519, verify_ed25519};
