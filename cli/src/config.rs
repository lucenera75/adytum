use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::{fs, path::PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Config {
    pub daemon_url: Option<String>,
    pub bundle_template: Option<String>,
    pub registry_address: Option<String>,
}

impl Config {
    pub fn path() -> PathBuf {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".adytum")
            .join("config.toml")
    }

    pub fn load() -> Result<Self> {
        let path = Self::path();
        if !path.exists() {
            return Ok(Self::default());
        }
        let contents = fs::read_to_string(&path)
            .with_context(|| format!("Failed to read config from {}", path.display()))?;
        toml::from_str(&contents).context("Failed to parse config.toml")
    }

    pub fn save(&self) -> Result<()> {
        let path = Self::path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let contents = toml::to_string_pretty(self)?;
        fs::write(&path, contents)
            .with_context(|| format!("Failed to write config to {}", path.display()))
    }

    pub fn daemon_url(&self, override_url: Option<&str>) -> String {
        override_url
            .map(str::to_string)
            .or_else(|| self.daemon_url.clone())
            .unwrap_or_else(|| "http://localhost:5100/json_rpc".to_string())
    }
}
