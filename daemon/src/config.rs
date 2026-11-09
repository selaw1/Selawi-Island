//! Daemon configuration: `$XDG_CONFIG_HOME/selawi-island/daemon.json`.
//!
//! ```json
//! { "integrations": { "mpris": true, "upower": true, "bluez": false } }
//! ```
//!
//! A missing file or key means "on". An unreadable file is reported and
//! ignored, so a typo never stops the daemon.

use std::collections::HashMap;
use std::path::PathBuf;

use serde::Deserialize;

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    #[serde(default)]
    pub integrations: HashMap<String, bool>,
}

impl Config {
    pub fn load() -> Config {
        let Some(path) = default_path() else {
            return Config::default();
        };
        match std::fs::read_to_string(&path) {
            Ok(text) => serde_json::from_str(&text).unwrap_or_else(|e| {
                eprintln!("Ignoring {}: {e}", path.display());
                Config::default()
            }),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Config::default(),
            Err(e) => {
                eprintln!("Ignoring {}: {e}", path.display());
                Config::default()
            }
        }
    }

    pub fn enabled(&self, integration: &str) -> bool {
        self.integrations.get(integration).copied().unwrap_or(true)
    }
}

pub fn default_path() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))?;
    Some(base.join("selawi-island").join("daemon.json"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_keys_mean_enabled() -> Result<(), serde_json::Error> {
        let config: Config = serde_json::from_str(r#"{"integrations":{"bluez":false}}"#)?;
        assert!(!config.enabled("bluez"));
        assert!(config.enabled("mpris"));
        assert!(Config::default().enabled("anything"));
        Ok(())
    }

    #[test]
    fn unknown_top_level_keys_are_rejected() {
        assert!(serde_json::from_str::<Config>(r#"{"integration":{}}"#).is_err());
    }
}
