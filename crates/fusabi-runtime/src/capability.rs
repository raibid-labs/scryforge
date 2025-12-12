//! Capability-based security model for plugins.
//!
//! Plugins must declare the capabilities they need in their manifest.
//! The runtime enforces that plugins only use capabilities they've declared.

use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// A capability that a plugin can request.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Capability {
    /// Access to network (HTTP requests).
    Network,

    /// Read from the filesystem.
    FileRead,

    /// Write to the filesystem.
    FileWrite,

    /// Access to system environment variables.
    Environment,

    /// Spawn subprocesses.
    Process,

    /// Access to provider credentials/tokens.
    Credentials,

    /// Read from the Scryforge cache.
    CacheRead,

    /// Write to the Scryforge cache.
    CacheWrite,

    /// Send notifications to the user.
    Notifications,

    /// Access to clipboard.
    Clipboard,

    /// Open URLs in external browser.
    OpenUrl,

    /// Custom capability for extension.
    Custom(String),
}

impl Capability {
    /// Parse a capability from a string.
    pub fn parse(s: &str) -> Self {
        match s {
            "network" => Capability::Network,
            "file_read" => Capability::FileRead,
            "file_write" => Capability::FileWrite,
            "environment" => Capability::Environment,
            "process" => Capability::Process,
            "credentials" => Capability::Credentials,
            "cache_read" => Capability::CacheRead,
            "cache_write" => Capability::CacheWrite,
            "notifications" => Capability::Notifications,
            "clipboard" => Capability::Clipboard,
            "open_url" => Capability::OpenUrl,
            other => Capability::Custom(other.to_string()),
        }
    }

    /// Convert capability to string representation.
    pub fn as_str(&self) -> &str {
        match self {
            Capability::Network => "network",
            Capability::FileRead => "file_read",
            Capability::FileWrite => "file_write",
            Capability::Environment => "environment",
            Capability::Process => "process",
            Capability::Credentials => "credentials",
            Capability::CacheRead => "cache_read",
            Capability::CacheWrite => "cache_write",
            Capability::Notifications => "notifications",
            Capability::Clipboard => "clipboard",
            Capability::OpenUrl => "open_url",
            Capability::Custom(s) => s,
        }
    }
}

/// A set of capabilities.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CapabilitySet {
    capabilities: HashSet<Capability>,
}

impl CapabilitySet {
    /// Create an empty capability set.
    pub fn new() -> Self {
        Self {
            capabilities: HashSet::new(),
        }
    }

    /// Create a capability set from a list of capability strings.
    pub fn from_strings<I, S>(strings: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let capabilities = strings
            .into_iter()
            .map(|s| Capability::parse(s.as_ref()))
            .collect();
        Self { capabilities }
    }

    /// Add a capability to the set.
    pub fn add(&mut self, cap: Capability) {
        self.capabilities.insert(cap);
    }

    /// Check if the set contains a capability.
    pub fn has(&self, cap: &Capability) -> bool {
        self.capabilities.contains(cap)
    }

    /// Check if this set is a superset of another.
    pub fn contains_all(&self, other: &CapabilitySet) -> bool {
        other.capabilities.is_subset(&self.capabilities)
    }

    /// Get all capabilities in the set.
    pub fn iter(&self) -> impl Iterator<Item = &Capability> {
        self.capabilities.iter()
    }

    /// Get the number of capabilities.
    pub fn len(&self) -> usize {
        self.capabilities.len()
    }

    /// Check if the set is empty.
    pub fn is_empty(&self) -> bool {
        self.capabilities.is_empty()
    }
}

impl FromIterator<Capability> for CapabilitySet {
    fn from_iter<T: IntoIterator<Item = Capability>>(iter: T) -> Self {
        Self {
            capabilities: iter.into_iter().collect(),
        }
    }
}
