//! Daemon configuration: `$XDG_CONFIG_HOME/selawi-island/daemon.json`.
//!
//! ```json
//! {
//!   "integrations": { "bluez": false },
//!   "media": { "mode": "one", "preferred_player": "spotify" },
//!   "notifications": { "show_body": true }
//! }
//! ```
//!
//! Every key is optional; a missing integration means "on". The settings
//! window changes it through `com.selawi.Island.Configuration1`, and changes
//! apply immediately. An invalid file is reported and ignored, so a typo
//! never stops the daemon.

use std::collections::BTreeMap;
use std::fs;
use std::io::Write;
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// Integration ids the configuration may mention.
pub const KNOWN_INTEGRATIONS: [&str; 8] = [
    "mpris",
    "upower",
    "bluez",
    "notifications",
    "network",
    "storage",
    "health",
    // Sent by the pacman hooks (selawi-island pacman-hook), not run here.
    "pacman",
];

/// Integrations that are off unless turned on: they poll instead of
/// listening to signals.
const OFF_BY_DEFAULT: [&str; 1] = ["health"];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MediaMode {
    /// Every active player has its own event.
    #[default]
    All,
    /// Only one player is shown: the preferred one while it plays,
    /// otherwise the one that started playing most recently.
    One,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct MediaConfig {
    pub mode: MediaMode,
    /// Stable player key, e.g. "spotify", "firefox", "vlc".
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preferred_player: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct NotificationConfig {
    /// Show the notification text, not only its title.
    pub show_body: bool,
}

impl Default for NotificationConfig {
    fn default() -> Self {
        Self { show_body: true }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct Config {
    pub integrations: BTreeMap<String, bool>,
    pub media: MediaConfig,
    pub notifications: NotificationConfig,
    /// Plugin id → allowed to show events. A plugin that is not listed
    /// has not been answered yet and shows nothing.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub plugins: BTreeMap<String, bool>,
}

impl Config {
    pub fn enabled(&self, integration: &str) -> bool {
        self.integrations
            .get(integration)
            .copied()
            .unwrap_or(!OFF_BY_DEFAULT.contains(&integration))
    }

    /// Whether the user allowed this plugin.
    pub fn plugin_allowed(&self, plugin: &str) -> bool {
        self.plugins.get(plugin).copied().unwrap_or(false)
    }

    /// The configuration with every known integration's on/off state
    /// spelled out, as shown to the settings window.
    pub fn effective(&self) -> Config {
        let mut config = self.clone();
        for id in KNOWN_INTEGRATIONS {
            config.integrations.insert(id.to_string(), self.enabled(id));
        }
        config
    }

    /// Parses and checks a configuration (from the file or from D-Bus).
    pub fn parse(json: &str) -> Result<Config, String> {
        let config: Config = serde_json::from_str(json).map_err(|e| e.to_string())?;
        for key in config.integrations.keys() {
            if !KNOWN_INTEGRATIONS.contains(&key.as_str()) {
                return Err(format!("unknown integration {key:?}"));
            }
        }
        for key in config.plugins.keys() {
            if !selawi_core::plugins::valid_plugin_id(key) {
                return Err(format!("invalid plugin id {key:?}"));
            }
        }
        if let Some(player) = &config.media.preferred_player {
            let valid = !player.is_empty()
                && player.len() <= 100
                && player
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'));
            if !valid {
                return Err("media.preferred_player is not a valid player key".into());
            }
        }
        Ok(config)
    }

    pub fn load(path: Option<&Path>) -> Config {
        let Some(path) = path else {
            return Config::default();
        };
        match fs::read_to_string(path) {
            Ok(text) => Config::parse(&text).unwrap_or_else(|e| {
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

    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_else(|_| "{}".into())
    }

    /// Writes atomically, readable by the user only.
    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        if let Some(dir) = path.parent() {
            fs::DirBuilder::new()
                .recursive(true)
                .mode(0o700)
                .create(dir)?;
        }
        let json = serde_json::to_string_pretty(self).map_err(std::io::Error::other)?;
        let tmp = path.with_extension("json.tmp");
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(&tmp)?;
        file.write_all(json.as_bytes())?;
        file.write_all(b"\n")?;
        file.sync_all()?;
        fs::rename(&tmp, path)
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
    fn plugins_are_off_until_allowed() -> Result<(), String> {
        let config = Config::parse(r#"{"plugins":{"pomodoro":true,"spam":false}}"#)?;
        assert!(config.plugin_allowed("pomodoro"));
        assert!(!config.plugin_allowed("spam"));
        assert!(!config.plugin_allowed("never-asked"));
        assert!(Config::parse(r#"{"plugins":{"Bad Id":true}}"#).is_err());
        Ok(())
    }

    #[test]
    fn missing_keys_mean_defaults() -> Result<(), String> {
        let config = Config::parse(r#"{"integrations":{"bluez":false}}"#)?;
        assert!(!config.enabled("bluez"));
        assert!(config.enabled("mpris"));
        assert!(!config.enabled("health"), "polling integrations are opt-in");
        assert_eq!(
            config.effective().integrations.len(),
            KNOWN_INTEGRATIONS.len()
        );
        assert_eq!(config.effective().integrations.get("bluez"), Some(&false));
        assert_eq!(config.media.mode, MediaMode::All);
        assert!(config.notifications.show_body);
        assert_eq!(Config::parse("{}")?, Config::default());
        Ok(())
    }

    #[test]
    fn full_config_round_trips() -> Result<(), String> {
        let json = r#"{"integrations":{"notifications":false},
            "media":{"mode":"one","preferred_player":"spotify"},
            "notifications":{"show_body":false}}"#;
        let config = Config::parse(json)?;
        assert_eq!(config.media.mode, MediaMode::One);
        assert_eq!(config.media.preferred_player.as_deref(), Some("spotify"));
        assert_eq!(Config::parse(&config.to_json())?, config);
        Ok(())
    }

    #[test]
    fn rejects_typos_and_bad_values() {
        assert!(Config::parse(r#"{"integration":{}}"#).is_err());
        assert!(Config::parse(r#"{"integrations":{"blueztooth":false}}"#).is_err());
        assert!(Config::parse(r#"{"media":{"mode":"some"}}"#).is_err());
        assert!(Config::parse(r#"{"media":{"preferred_player":"rm -rf /"}}"#).is_err());
    }

    #[test]
    fn saves_privately() -> Result<(), Box<dyn std::error::Error>> {
        let dir = std::env::temp_dir().join(format!("selawi-config-{}", std::process::id()));
        let path = dir.join("daemon.json");
        let mut config = Config::default();
        config.integrations.insert("bluez".into(), false);
        config.save(&path)?;
        let mode = std::os::unix::fs::PermissionsExt::mode(&fs::metadata(&path)?.permissions());
        assert_eq!(mode & 0o777, 0o600);
        assert_eq!(Config::load(Some(&path)), config);
        fs::remove_dir_all(dir)?;
        Ok(())
    }
}
