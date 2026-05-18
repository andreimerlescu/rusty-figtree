//! Configuration sources for figtree.
//!
//! Each source implements the Source trait from priority.rs.
//! Sources are feature-gated — only the sources whose feature
//! flags are enabled in Cargo.toml are compiled into the binary.

pub mod env;

#[cfg(feature = "cli")]
pub mod cli;

#[cfg(feature = "yaml")]
pub mod yaml;

#[cfg(feature = "json")]
pub mod json;

#[cfg(feature = "ini")]
pub mod ini;

#[cfg(feature = "embedded")]
pub mod embedded;

// Re-export all source types at the sources module level
// so consumers can write figtree::sources::EnvSource rather
// than figtree::sources::env::EnvSource.

pub use env::EnvSource;

#[cfg(feature = "cli")]
pub use cli::CliSource;

#[cfg(feature = "yaml")]
pub use yaml::YamlSource;

#[cfg(feature = "json")]
pub use json::JsonSource;

#[cfg(feature = "ini")]
pub use ini::IniSource;

#[cfg(feature = "embedded")]
pub use embedded::EmbeddedSource;
