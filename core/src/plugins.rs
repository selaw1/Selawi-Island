//! Plugins: apps that show their own events on the island through the
//! `Plugins1` D-Bus interface. This module holds the rules, with no
//! platform code:
//!
//! - the **manifest** a plugin registers with (who it is, what it shows);
//! - **sealing** each event it sends: the id is put in the plugin's own
//!   namespace (`plugin:<id>:<local id>`), and its name, icon and
//!   integration are the manifest's, so a plugin can never touch another
//!   source's events or pass itself off as one;
//! - a **rate limit** per plugin.

use serde::{Deserialize, Serialize};

use crate::events::{check_icon, Category, EventId, EventInput, Source, Timestamp};
use crate::Priority;

pub const MANIFEST_VERSION: u32 = 1;
/// `source.integration` of every plugin event.
pub const PLUGIN_INTEGRATION: &str = "plugin";
/// Event ids of plugin events start with this.
pub const PLUGIN_ID_PREFIX: &str = "plugin:";
/// Events one plugin may have on the island at a time.
pub const MAX_EVENTS_PER_PLUGIN: usize = 8;
const MAX_PLUGIN_ID_LEN: usize = 40;
const MAX_LOCAL_ID_LEN: usize = 64;
const MAX_PLUGIN_NAME_LEN: usize = 40;
const MAX_DESCRIPTION_LEN: usize = 200;

/// What a plugin says about itself when it registers.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub manifest_version: u32,
    /// Stable, unique: lowercase letters, digits, `.` and `-`
    /// ("pomodoro", "com.example.chat").
    pub id: String,
    /// Shown as the event's app name and in Settings.
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// `app:<desktop id>` or a PNG data URL, like `source.icon`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    /// The kinds of events it may show. A plugin cannot show anything else.
    pub categories: Vec<Category>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PluginError {
    InvalidManifest(String),
    /// The event is not allowed for this plugin (or malformed).
    InvalidEvent(String),
}

impl std::fmt::Display for PluginError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PluginError::InvalidManifest(why) => write!(f, "invalid manifest: {why}"),
            PluginError::InvalidEvent(why) => write!(f, "{why}"),
        }
    }
}

impl std::error::Error for PluginError {}

/// Plugin ids: what settings, event ids and D-Bus users see.
pub fn valid_plugin_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= MAX_PLUGIN_ID_LEN
        && id.starts_with(|c: char| c.is_ascii_lowercase() || c.is_ascii_digit())
        && id
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '.' || c == '-')
}

fn plain_text(value: &str, max: usize) -> bool {
    !value.trim().is_empty() && value.chars().count() <= max && !value.chars().any(char::is_control)
}

/// Reads and checks a manifest.
pub fn parse_manifest(json: &str) -> Result<Manifest, PluginError> {
    let bad = |why: &str| PluginError::InvalidManifest(why.into());
    let manifest: Manifest = serde_json::from_str(json).map_err(|e| bad(&e.to_string()))?;
    if manifest.manifest_version != MANIFEST_VERSION {
        return Err(bad("manifest_version must be 1"));
    }
    if !valid_plugin_id(&manifest.id) {
        return Err(bad("id: 1-40 lowercase letters, digits, '.' or '-'"));
    }
    if !plain_text(&manifest.name, MAX_PLUGIN_NAME_LEN) {
        return Err(bad("name: 1-40 characters of plain text"));
    }
    if let Some(d) = &manifest.description {
        if !plain_text(d, MAX_DESCRIPTION_LEN) {
            return Err(bad("description: up to 200 characters of plain text"));
        }
    }
    if let Some(icon) = &manifest.icon {
        check_icon(icon).map_err(|_| bad("icon: app:<desktop id> or a PNG data URL"))?;
    }
    if manifest.categories.is_empty() {
        return Err(bad("categories: list at least one"));
    }
    Ok(manifest)
}

/// "plugin:pomodoro:" — every event id of that plugin starts with this.
pub fn event_prefix(plugin_id: &str) -> String {
    format!("{PLUGIN_ID_PREFIX}{plugin_id}:")
}

/// The id a plugin used for an event of its own ("timer"), from the
/// stored id ("plugin:pomodoro:timer"). None if it is not that plugin's.
pub fn local_id<'a>(plugin_id: &str, event_id: &'a str) -> Option<&'a str> {
    event_id.strip_prefix(&event_prefix(plugin_id))
}

/// Makes a plugin's event safe to show: its id goes into the plugin's
/// namespace, its name, icon and integration become the manifest's, and
/// only the declared categories (and no critical priority) are accepted.
pub fn seal(manifest: &Manifest, mut input: EventInput) -> Result<EventInput, PluginError> {
    let bad = |why: String| PluginError::InvalidEvent(why);
    let local = input.id.as_str().to_string();
    if local.len() > MAX_LOCAL_ID_LEN || local.contains(':') {
        return Err(bad("id: up to 64 characters, without ':'".into()));
    }
    if !manifest.categories.contains(&input.category) {
        return Err(bad(format!(
            "category {:?} is not in the plugin's manifest",
            input.category
        )));
    }
    match input.priority {
        Some(Priority::Critical) => return Err(bad("plugins cannot use critical priority".into())),
        Some(Priority::Call) if input.category != Category::Call => {
            return Err(bad("call priority is only for call events".into()))
        }
        _ => {}
    }
    input.id = EventId::new(format!("{}{local}", event_prefix(&manifest.id)))
        .map_err(|e| bad(e.to_string()))?;
    input.source = Source {
        app: manifest.name.clone(),
        integration: PLUGIN_INTEGRATION.into(),
        icon: manifest.icon.clone(),
    };
    input.group = input
        .group
        .map(|g| format!("{}{g}", event_prefix(&manifest.id)));
    input.validate().map_err(|e| bad(e.to_string()))?;
    Ok(input)
}

/// Token bucket: `RATE_PER_SEC` events a second on average, bursts up to
/// `BURST`.
#[derive(Debug, Clone)]
pub struct RateLimit {
    tokens: f64,
    last: Timestamp,
}

const RATE_PER_SEC: f64 = 10.0;
const BURST: f64 = 30.0;

impl RateLimit {
    pub fn new(now: Timestamp) -> Self {
        RateLimit {
            tokens: BURST,
            last: now,
        }
    }

    /// Whether one more event may go through now.
    pub fn allow(&mut self, now: Timestamp) -> bool {
        let elapsed = now.0.saturating_sub(self.last.0) as f64 / 1000.0;
        self.last = now;
        self.tokens = (self.tokens + elapsed * RATE_PER_SEC).min(BURST);
        if self.tokens >= 1.0 {
            self.tokens -= 1.0;
            true
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse_input;

    fn manifest() -> Result<Manifest, PluginError> {
        parse_manifest(
            r#"{"manifest_version":1,"id":"pomodoro","name":"Pomodoro",
                "description":"Focus timer","icon":"app:org.example.Pomodoro",
                "categories":["custom","notification"]}"#,
        )
    }

    fn event(json: &str) -> Result<EventInput, String> {
        parse_input(json).map_err(|e| e.to_string())
    }

    #[test]
    fn manifests_are_checked() -> Result<(), PluginError> {
        let m = manifest()?;
        assert_eq!(m.name, "Pomodoro");
        let base = r#""manifest_version":1,"name":"X","categories":["custom"]"#;
        for bad in [
            format!(r#"{{{base},"id":"Has Space"}}"#),
            format!(r#"{{{base},"id":"../x"}}"#),
            format!(r#"{{{base},"id":"ok","icon":"https://x/y.png"}}"#),
            format!(r#"{{{base},"id":"ok","extra":1}}"#),
            r#"{"manifest_version":2,"id":"ok","name":"X","categories":["custom"]}"#.into(),
            r#"{"manifest_version":1,"id":"ok","name":"X","categories":[]}"#.into(),
            r#"{"manifest_version":1,"id":"ok","name":"","categories":["custom"]}"#.into(),
        ] {
            assert!(parse_manifest(&bad).is_err(), "accepted: {bad}");
        }
        assert!(parse_manifest(&format!(r#"{{{base},"id":"com.example.chat"}}"#)).is_ok());
        Ok(())
    }

    #[test]
    fn events_are_sealed() -> Result<(), String> {
        let m = manifest().map_err(|e| e.to_string())?;
        let input = event(
            r#"{"id":"timer","source":{"app":"Slack","integration":"mpris","icon":"app:spotify"},
                "category":"custom","state":"started","group":"focus",
                "payload":{"type":"progress","title":"Focus","percent":10}}"#,
        )?;
        let sealed = seal(&m, input).map_err(|e| e.to_string())?;
        assert_eq!(sealed.id.as_str(), "plugin:pomodoro:timer");
        assert_eq!(sealed.source.app, "Pomodoro", "cannot pose as Slack");
        assert_eq!(sealed.source.integration, "plugin");
        assert_eq!(
            sealed.source.icon.as_deref(),
            Some("app:org.example.Pomodoro")
        );
        assert_eq!(sealed.group.as_deref(), Some("plugin:pomodoro:focus"));
        assert_eq!(local_id("pomodoro", sealed.id.as_str()), Some("timer"));
        assert_eq!(local_id("other", sealed.id.as_str()), None);
        Ok(())
    }

    #[test]
    fn only_what_the_manifest_declares() -> Result<(), String> {
        let m = manifest().map_err(|e| e.to_string())?;
        let call = event(
            r#"{"id":"c","source":{"app":"x","integration":"x"},"category":"call","state":"started",
                "payload":{"type":"call","title":"Fake call","confidence":"confirmed"}}"#,
        )?;
        assert!(seal(&m, call).is_err(), "call not declared");
        let critical = event(
            r#"{"id":"n","source":{"app":"x","integration":"x"},"category":"notification",
                "priority":"critical","state":"started","payload":{"type":"notification","title":"!"}}"#,
        )?;
        assert!(seal(&m, critical).is_err(), "no critical priority");
        let other = event(
            r#"{"id":"mpris:spotify","source":{"app":"x","integration":"x"},"category":"custom",
                "state":"started","payload":{"type":"progress","title":"x"}}"#,
        )?;
        assert!(seal(&m, other).is_err(), "no ':' in a plugin's own ids");
        Ok(())
    }

    #[test]
    fn rate_limit() {
        let mut limit = RateLimit::new(Timestamp(0));
        let burst = (0..100).filter(|_| limit.allow(Timestamp(0))).count();
        assert_eq!(burst, 30);
        assert!(!limit.allow(Timestamp(50)), "still empty after 50 ms");
        assert!(limit.allow(Timestamp(200)), "refills over time");
        // After the burst is used up: about 10 a second.
        let mut steady = RateLimit::new(Timestamp(0));
        while steady.allow(Timestamp(0)) {}
        let in_one_second = (1..=20).filter(|i| steady.allow(Timestamp(i * 50))).count();
        assert!(
            (9..=11).contains(&in_one_second),
            "{in_one_second} in one second"
        );
    }
}
