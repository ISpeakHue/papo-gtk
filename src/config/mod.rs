//! Persistent application configuration stored in `~/.config/papo-gtk/config.toml`.

use anyhow::{Context, Result};
use dirs::config_dir;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// User-editable app config that survives restarts.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AppConfig {
    /// Last server URL the user connected to.
    pub server_url: Option<String>,
    /// Last logged-in username (to pre-fill the login form).
    pub last_username: Option<String>,
    /// GTK colour scheme override ("dark" | "light" | "system").
    pub theme: Option<String>,
    /// Resume a valid session from the desktop keyring on the next launch.
    pub remember_session: Option<bool>,
}

impl AppConfig {
    /// XDG config path: `~/.config/papo-gtk/config.toml`.
    pub fn path() -> PathBuf {
        config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("papo-gtk")
            .join("config.toml")
    }

    /// Load config from disk; returns `Default::default()` if the file does
    /// not exist or cannot be parsed.
    pub fn load() -> Self {
        let p = Self::path();
        std::fs::read_to_string(&p)
            .ok()
            .and_then(|s| toml::from_str(&s).ok())
            .unwrap_or_default()
    }

    /// Persist config to disk, creating parent directories if needed.
    pub fn save(&self) -> Result<()> {
        let p = Self::path();
        if let Some(parent) = p.parent() {
            std::fs::create_dir_all(parent).context("creating config dir")?;
        }
        let toml_str = toml::to_string_pretty(self).context("serialising config")?;
        std::fs::write(&p, toml_str).context("writing config")?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_app_config_serialize_deserialize() {
        let config = AppConfig {
            server_url: Some("http://localhost:8080".into()),
            last_username: Some("testuser".into()),
            theme: Some("dark".into()),
            remember_session:Some(true),
        };

        let serialized = toml::to_string_pretty(&config).expect("serialization failed");
        let deserialized: AppConfig = toml::from_str(&serialized).expect("deserialization failed");

        assert_eq!(deserialized.server_url.as_deref(), Some("http://localhost:8080"));
        assert_eq!(deserialized.last_username.as_deref(), Some("testuser"));
        assert_eq!(deserialized.theme.as_deref(), Some("dark"));
    }

    #[test]
    fn test_app_config_defaults() {
        let default_config = AppConfig::default();
        assert!(default_config.server_url.is_none());
        assert!(default_config.last_username.is_none());
        assert!(default_config.theme.is_none());
    }
}
