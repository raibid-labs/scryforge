//! Error types for the Fusabi runtime.

use thiserror::Error;

/// Errors that can occur in the Fusabi runtime.
#[derive(Error, Debug)]
pub enum RuntimeError {
    /// Plugin not found at the specified path.
    #[error("Plugin not found: {0}")]
    PluginNotFound(String),

    /// Failed to parse plugin manifest.
    #[error("Invalid manifest: {0}")]
    InvalidManifest(String),

    /// Failed to load bytecode.
    #[error("Bytecode error: {0}")]
    BytecodeError(String),

    /// Plugin requested a capability it doesn't have.
    #[error("Missing capability: {0}")]
    MissingCapability(String),

    /// Plugin is disabled.
    #[error("Plugin is disabled: {0}")]
    PluginDisabled(String),

    /// Plugin failed to initialize.
    #[error("Plugin initialization failed: {0}")]
    InitializationFailed(String),

    /// Plugin execution failed.
    #[error("Execution error: {0}")]
    ExecutionError(String),

    /// IO error.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// JSON serialization/deserialization error.
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// TOML parsing error.
    #[error("TOML error: {0}")]
    Toml(#[from] toml::de::Error),
}

/// Result type for runtime operations.
pub type RuntimeResult<T> = std::result::Result<T, RuntimeError>;
