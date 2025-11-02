//! Workspace configuration management
//!
//! Manages .pmcp-config.toml for tracking server ports and settings

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

const CONFIG_FILE: &str = ".pmcp-config.toml";

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct WorkspaceConfig {
    #[serde(default)]
    pub servers: HashMap<String, ServerConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    pub port: u16,
    pub template: String,
}

impl WorkspaceConfig {
    /// Load configuration from workspace root
    pub fn load() -> Result<Self> {
        let config_path = Path::new(CONFIG_FILE);

        if !config_path.exists() {
            return Ok(Self::default());
        }

        let content =
            fs::read_to_string(config_path).context("Failed to read .pmcp-config.toml")?;

        toml::from_str(&content).context("Failed to parse .pmcp-config.toml")
    }

    /// Save configuration to workspace root
    pub fn save(&self) -> Result<()> {
        let content = toml::to_string_pretty(self).context("Failed to serialize config")?;

        fs::write(CONFIG_FILE, content).context("Failed to write .pmcp-config.toml")?;

        Ok(())
    }

    /// Get next available port (auto-increment from 3000)
    pub fn next_available_port(&self) -> u16 {
        if self.servers.is_empty() {
            return 3000;
        }

        // Find max port and add 1
        self.servers
            .values()
            .map(|s| s.port)
            .max()
            .map(|p| p + 1)
            .unwrap_or(3000)
    }

    /// Check if a port is already in use
    pub fn is_port_used(&self, port: u16) -> bool {
        self.servers.values().any(|s| s.port == port)
    }

    /// Check if server name exists
    pub fn has_server(&self, name: &str) -> bool {
        self.servers.contains_key(name)
    }

    /// Add or update server configuration
    pub fn add_server(&mut self, name: String, port: u16, template: String) {
        self.servers.insert(name, ServerConfig { port, template });
    }

    /// Remove server configuration
    pub fn remove_server(&mut self, name: &str) -> Option<ServerConfig> {
        self.servers.remove(name)
    }

    /// Get server configuration
    pub fn get_server(&self, name: &str) -> Option<&ServerConfig> {
        self.servers.get(name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_next_available_port() {
        let mut config = WorkspaceConfig::default();
        assert_eq!(config.next_available_port(), 3000);

        config.add_server("calc".to_string(), 3000, "calculator".to_string());
        assert_eq!(config.next_available_port(), 3001);

        config.add_server("explorer".to_string(), 3001, "sqlite-explorer".to_string());
        assert_eq!(config.next_available_port(), 3002);
    }

    #[test]
    fn test_is_port_used() {
        let mut config = WorkspaceConfig::default();
        assert!(!config.is_port_used(3000));

        config.add_server("calc".to_string(), 3000, "calculator".to_string());
        assert!(config.is_port_used(3000));
        assert!(!config.is_port_used(3001));
    }
}
