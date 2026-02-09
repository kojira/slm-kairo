use std::path::Path;

use crate::error::{KairoError, Result};

pub fn load_config(path: &Path) -> Result<toml::Value> {
    let content = std::fs::read_to_string(path).map_err(|e| {
        KairoError::ConfigError(format!("Failed to read config {}: {e}", path.display()))
    })?;
    let value: toml::Value = content.parse().map_err(|e| {
        KairoError::ConfigError(format!("Failed to parse config {}: {e}", path.display()))
    })?;
    Ok(value)
}
