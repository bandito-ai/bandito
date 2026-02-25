use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

const DEFAULT_BASE_URL: &str = "https://bandito-api.onrender.com";

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Config {
    #[serde(default)]
    pub api_key: String,
    #[serde(default = "default_base_url")]
    pub base_url: String,
    #[serde(default = "default_data_storage")]
    pub data_storage: String,
}

fn default_base_url() -> String {
    DEFAULT_BASE_URL.to_string()
}

fn default_data_storage() -> String {
    "local".to_string()
}

impl Config {
    pub fn config_dir() -> Result<PathBuf> {
        let home = dirs::home_dir().context("Could not determine home directory")?;
        Ok(home.join(".bandito"))
    }

    pub fn config_path() -> Result<PathBuf> {
        Ok(Self::config_dir()?.join("config.toml"))
    }

    pub fn load() -> Result<Config> {
        let path = Self::config_path()?;
        if !path.exists() {
            return Ok(Config::default());
        }
        let contents = fs::read_to_string(&path)
            .with_context(|| format!("Failed to read {}", path.display()))?;
        let config: Config = toml::from_str(&contents).unwrap_or_else(|e| {
            eprintln!("Warning: failed to parse config ({}), using defaults", e);
            Config::default()
        });
        Ok(config)
    }

    pub fn save(&self) -> Result<()> {
        let dir = Self::config_dir()?;
        fs::create_dir_all(&dir)?;
        let path = Self::config_path()?;

        // Write TOML manually to ensure proper escaping
        // base_url is intentionally omitted — always uses the default
        // (overridable via BANDITO_BASE_URL env var for development)
        let contents = format!(
            "api_key = {}\ndata_storage = {}\n",
            escape_toml_value(&self.api_key),
            escape_toml_value(&self.data_storage),
        );
        fs::write(&path, contents)
            .with_context(|| format!("Failed to write {}", path.display()))?;
        Ok(())
    }

    pub fn is_configured(&self) -> bool {
        !self.api_key.is_empty()
    }

    pub fn default_base_url() -> &'static str {
        DEFAULT_BASE_URL
    }
}

/// Escape a string value for TOML (matches Python SDK's _escape_toml_value)
fn escape_toml_value(s: &str) -> String {
    let escaped = s.replace('\\', "\\\\").replace('"', "\\\"");
    format!("\"{}\"", escaped)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_escape_toml_value() {
        assert_eq!(escape_toml_value("hello"), "\"hello\"");
        assert_eq!(escape_toml_value("say \"hi\""), "\"say \\\"hi\\\"\"");
        assert_eq!(escape_toml_value("a\\b"), "\"a\\\\b\"");
    }
}
