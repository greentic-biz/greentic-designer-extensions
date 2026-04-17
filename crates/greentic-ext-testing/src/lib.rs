//! Test utilities for Greentic Designer Extensions.
//!
//! Builders for synthetic extensions and gtxpack ZIP helpers used across
//! the runtime and CLI test suites.

mod fixture;
mod gtxpack;

pub use self::fixture::{ExtensionFixture, ExtensionFixtureBuilder};
pub use self::gtxpack::{pack_directory, unpack_to_dir};
