//! Plugin discovery from well-known paths.
//!
//! Plugins are discovered from the following locations (in order):
//!
//! 1. `$XDG_DATA_HOME/scryforge/plugins/` (user plugins)
//! 2. `$XDG_DATA_DIRS/scryforge/plugins/` (system plugins)
//! 3. Built-in plugins directory
//!
//! Each plugin is a directory containing a `manifest.toml` file.

use crate::error::RuntimeResult;
use crate::manifest::PluginManifest;
use std::path::{Path, PathBuf};
use tracing::{debug, info, warn};

/// Information about a discovered plugin.
#[derive(Debug, Clone)]
pub struct PluginPath {
    /// Path to the plugin directory.
    pub path: PathBuf,

    /// Parsed manifest.
    pub manifest: PluginManifest,

    /// Whether the plugin is enabled.
    pub enabled: bool,
}

impl PluginPath {
    /// Get the plugin ID.
    pub fn id(&self) -> &str {
        &self.manifest.plugin.id
    }

    /// Get the plugin name.
    pub fn name(&self) -> &str {
        &self.manifest.plugin.name
    }

    /// Get the plugin version.
    pub fn version(&self) -> &str {
        &self.manifest.plugin.version
    }

    /// Get the path to the entry point file.
    pub fn entry_point_path(&self) -> PathBuf {
        self.path.join(self.manifest.entry_point())
    }

    /// Check if the entry point file exists.
    pub fn has_entry_point(&self) -> bool {
        self.entry_point_path().exists()
    }
}

/// Get the user plugins directory.
pub fn user_plugins_dir() -> Option<PathBuf> {
    directories::ProjectDirs::from("com", "raibid-labs", "scryforge")
        .map(|dirs| dirs.data_dir().join("plugins"))
}

/// Get the system plugins directories.
pub fn system_plugins_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();

    // Check XDG_DATA_DIRS
    if let Ok(data_dirs) = std::env::var("XDG_DATA_DIRS") {
        for dir in data_dirs.split(':') {
            let plugin_dir = PathBuf::from(dir).join("scryforge/plugins");
            if plugin_dir.exists() {
                dirs.push(plugin_dir);
            }
        }
    }

    // Default system locations
    let default_dirs = [
        "/usr/local/share/scryforge/plugins",
        "/usr/share/scryforge/plugins",
    ];

    for dir in default_dirs {
        let path = PathBuf::from(dir);
        if path.exists() && !dirs.contains(&path) {
            dirs.push(path);
        }
    }

    dirs
}

/// Discover all plugins from well-known paths.
pub fn discover_plugins() -> RuntimeResult<Vec<PluginPath>> {
    let mut plugins = Vec::new();
    let mut seen_ids = std::collections::HashSet::new();

    // User plugins have priority
    if let Some(user_dir) = user_plugins_dir() {
        debug!("Scanning user plugins directory: {:?}", user_dir);
        discover_in_directory(&user_dir, &mut plugins, &mut seen_ids)?;
    }

    // System plugins
    for sys_dir in system_plugins_dirs() {
        debug!("Scanning system plugins directory: {:?}", sys_dir);
        discover_in_directory(&sys_dir, &mut plugins, &mut seen_ids)?;
    }

    info!("Discovered {} plugins", plugins.len());
    Ok(plugins)
}

/// Discover plugins in a specific directory.
pub fn discover_in_directory(
    dir: &Path,
    plugins: &mut Vec<PluginPath>,
    seen_ids: &mut std::collections::HashSet<String>,
) -> RuntimeResult<()> {
    if !dir.exists() {
        return Ok(());
    }

    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(e) => {
            warn!("Failed to read plugins directory {:?}: {}", dir, e);
            return Ok(());
        }
    };

    for entry in entries.flatten() {
        let path = entry.path();

        if !path.is_dir() {
            continue;
        }

        let manifest_path = path.join("manifest.toml");
        if !manifest_path.exists() {
            debug!("Skipping {:?}: no manifest.toml", path);
            continue;
        }

        match PluginManifest::from_file(&manifest_path) {
            Ok(manifest) => {
                let id = manifest.plugin.id.clone();

                // Skip if we've already seen this plugin (user plugins take priority)
                if seen_ids.contains(&id) {
                    debug!("Skipping duplicate plugin: {}", id);
                    continue;
                }

                info!(
                    "Discovered plugin: {} v{} at {:?}",
                    manifest.plugin.name, manifest.plugin.version, path
                );

                seen_ids.insert(id);
                plugins.push(PluginPath {
                    path,
                    manifest,
                    enabled: true, // Default to enabled
                });
            }
            Err(e) => {
                warn!("Failed to load manifest from {:?}: {}", manifest_path, e);
            }
        }
    }

    Ok(())
}

/// Discover a single plugin from a path.
pub fn discover_plugin(path: &Path) -> RuntimeResult<PluginPath> {
    let manifest_path = path.join("manifest.toml");
    let manifest = PluginManifest::from_file(&manifest_path)?;

    Ok(PluginPath {
        path: path.to_path_buf(),
        manifest,
        enabled: true,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;

    fn create_test_plugin(dir: &Path, id: &str) {
        let plugin_dir = dir.join(id);
        std::fs::create_dir_all(&plugin_dir).unwrap();

        let manifest = format!(
            r#"
[plugin]
id = "{id}"
name = "Test Plugin {id}"
version = "0.1.0"
plugin_type = "provider"

capabilities = ["network"]

[provider]
id = "{id}"
has_feeds = true
"#
        );

        let manifest_path = plugin_dir.join("manifest.toml");
        let mut file = std::fs::File::create(manifest_path).unwrap();
        file.write_all(manifest.as_bytes()).unwrap();
    }

    #[test]
    fn test_discover_in_directory() {
        let temp_dir = TempDir::new().unwrap();

        create_test_plugin(temp_dir.path(), "plugin-a");
        create_test_plugin(temp_dir.path(), "plugin-b");

        let mut plugins = Vec::new();
        let mut seen = std::collections::HashSet::new();

        discover_in_directory(temp_dir.path(), &mut plugins, &mut seen).unwrap();

        assert_eq!(plugins.len(), 2);
        assert!(seen.contains("plugin-a"));
        assert!(seen.contains("plugin-b"));
    }

    #[test]
    fn test_plugin_priority() {
        let temp_dir = TempDir::new().unwrap();

        // Create two plugins with same ID
        let dir1 = temp_dir.path().join("dir1");
        let dir2 = temp_dir.path().join("dir2");
        std::fs::create_dir_all(&dir1).unwrap();
        std::fs::create_dir_all(&dir2).unwrap();

        create_test_plugin(&dir1, "same-id");
        create_test_plugin(&dir2, "same-id");

        let mut plugins = Vec::new();
        let mut seen = std::collections::HashSet::new();

        // First directory takes priority
        discover_in_directory(&dir1, &mut plugins, &mut seen).unwrap();
        discover_in_directory(&dir2, &mut plugins, &mut seen).unwrap();

        assert_eq!(plugins.len(), 1);
        assert!(plugins[0].path.starts_with(&dir1));
    }
}
