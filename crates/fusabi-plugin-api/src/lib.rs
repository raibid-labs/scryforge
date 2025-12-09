//! # fusabi-plugin-api
//!
//! Plugin API for Scryforge providers using Fusabi.
//!
//! This crate provides the bridge between the Fusabi runtime and Scryforge's
//! provider system. It allows plugins to:
//!
//! - Implement the `Provider` trait
//! - Access capabilities (network, credentials, etc.)
//! - Interact with the Scryforge cache
//!
//! ## Plugin Development
//!
//! Plugins are developed using the Fusabi language and compiled to bytecode.
//! The runtime executes the bytecode and translates calls to the provider API.

pub mod host;
pub mod plugin;
pub mod registry;

pub use host::HostFunctions;
pub use plugin::{PluginInstance, PluginProvider};
pub use registry::PluginRegistry;
