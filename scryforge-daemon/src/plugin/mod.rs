//! # Plugin Management
//!
//! Manages Fusabi plugins for the Scryforge daemon.
//!
//! This module provides:
//! - Plugin discovery and loading
//! - Plugin lifecycle management (enable/disable/reload)
//! - Integration with the provider registry

mod manager;

pub use manager::{PluginManager, PluginStatus};
