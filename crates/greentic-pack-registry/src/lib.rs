//! Pack registry client — resolves catalog refs to `.gtpack` bytes by
//! hitting the greentic-store-server's pack download endpoint.

#![forbid(unsafe_code)]

mod client;
mod error;

pub use client::{PackRef, PackRegistryClient, PackVersionMetadata, StoreServerClient};
pub use error::RegistryError;
