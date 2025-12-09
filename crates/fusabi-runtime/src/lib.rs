//! # fusabi-runtime
//!
//! Fusabi bytecode runtime for executing plugins in Scryforge.
//!
//! This crate provides:
//! - Plugin discovery from well-known paths
//! - Plugin manifest parsing
//! - Bytecode loading and validation
//! - Capability-based security model
//!
//! ## Plugin Structure
//!
//! Plugins are directories containing:
//! - `manifest.toml` - Plugin metadata and capabilities
//! - `plugin.fzb` - Compiled Fusabi bytecode (optional, for bytecode plugins)
//! - `plugin.fsx` - Source file (optional, for interpreted plugins)
//!
//! ## Security Model
//!
//! Plugins declare required capabilities in their manifest. The runtime
//! validates that plugins only use capabilities they've declared.

pub mod bytecode;
pub mod capability;
pub mod discovery;
pub mod error;
pub mod manifest;

pub use bytecode::{Bytecode, BytecodeLoader};
pub use capability::{Capability, CapabilitySet};
pub use discovery::{discover_plugin, discover_plugins, PluginPath};
pub use error::{RuntimeError, RuntimeResult};
pub use manifest::{PluginManifest, PluginMetadata};
