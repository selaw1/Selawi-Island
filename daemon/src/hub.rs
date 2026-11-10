//! The hub connects everything inside the daemon: the D-Bus interfaces,
//! the built-in integrations and the expiry task all go through it.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::OnceLock;

use selawi_core::{EventInput, Timestamp};
use serde::Serialize;
use tokio::sync::{mpsc, watch, Mutex, Notify};
use zbus::object_server::SignalEmitter;

use crate::config::Config;
use crate::engine::{Engine, SubmitError};

/// An action button pressed on an event owned by a built-in integration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActionRequest {
    pub event_id: String,
    pub action_id: String,
}

/// What an integration can do right now.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case", tag = "status", content = "detail")]
pub enum Status {
    Starting,
    /// Working. The detail says what it is watching.
    Active(String),
    /// Cannot work on this system (no battery, service not installed…).
    Unavailable(String),
    /// Turned off in the configuration.
    Disabled,
    /// Stopped by an error; it is retried.
    Failed(String),
}

/// A media player currently on the bus, for the settings window.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MediaPlayer {
    /// Stable key used by `media.preferred_player`, e.g. "spotify".
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct IntegrationInfo {
    pub id: &'static str,
    pub name: &'static str,
    #[serde(flatten)]
    pub status: Status,
}

/// Least time between two `Changed` signals.
pub const CHANGED_INTERVAL: std::time::Duration = std::time::Duration::from_millis(50);

pub struct Hub {
    engine: Mutex<Engine>,
    /// Wakes the expiry task whenever the next deadline may have changed.
    pub wake: Notify,
    emitter: OnceLock<SignalEmitter<'static>>,
    /// The event list changed and `Changed` has not been sent for it yet.
    changes: Notify,
    /// Event id prefix → the integration that owns those events.
    routes: Mutex<Vec<(String, mpsc::UnboundedSender<ActionRequest>)>>,
    integrations: Mutex<BTreeMap<&'static str, IntegrationInfo>>,
    config: watch::Sender<Config>,
    config_path: Option<PathBuf>,
    media_players: Mutex<Vec<MediaPlayer>>,
}

impl Hub {
    /// A hub with the default configuration that is never saved (tests).
    pub fn new(engine: Engine) -> Self {
        Self::with_config(engine, Config::default(), None)
    }

    /// `config_path` is where configuration changes are saved.
    pub fn with_config(engine: Engine, config: Config, config_path: Option<PathBuf>) -> Self {
        Self {
            engine: Mutex::new(engine),
            wake: Notify::new(),
            emitter: OnceLock::new(),
            changes: Notify::new(),
            routes: Mutex::new(Vec::new()),
            integrations: Mutex::new(BTreeMap::new()),
            config: watch::channel(config).0,
            config_path,
            media_players: Mutex::new(Vec::new()),
        }
    }

    pub fn config(&self) -> Config {
        self.config.borrow().clone()
    }

    /// Follows configuration changes.
    pub fn watch_config(&self) -> watch::Receiver<Config> {
        self.config.subscribe()
    }

    /// Checks, saves and applies a new configuration (JSON).
    pub fn set_config(&self, json: &str) -> Result<Config, String> {
        let config = Config::parse(json)?;
        if let Some(path) = &self.config_path {
            config
                .save(path)
                .map_err(|e| format!("could not save: {e}"))?;
        }
        self.config.send_replace(config.clone());
        Ok(config)
    }

    pub async fn set_media_players(&self, players: Vec<MediaPlayer>) {
        *self.media_players.lock().await = players;
    }

    pub async fn media_players_json(&self) -> String {
        serde_json::to_string(&*self.media_players.lock().await).unwrap_or_else(|_| "[]".into())
    }

    /// Set once the D-Bus object is registered, so changes can be announced.
    pub fn set_emitter(&self, emitter: SignalEmitter<'static>) {
        let _ = self.emitter.set(emitter);
    }

    pub async fn submit_json(&self, json: &str) -> Result<(), SubmitError> {
        // Events sent from outside for an integration that is turned off
        // (e.g. "pacman:…" from the pacman hook) are dropped quietly.
        if let Some(owner) = owning_integration(json) {
            if !self.config.borrow().enabled(owner) {
                return Ok(());
            }
        }
        let changed = self.engine.lock().await.submit(json, Timestamp::now())?;
        self.after_change(changed).await;
        Ok(())
    }

    pub async fn submit(&self, input: EventInput) -> Result<(), SubmitError> {
        let changed = self
            .engine
            .lock()
            .await
            .submit_input(input, Timestamp::now())?;
        self.after_change(changed).await;
        Ok(())
    }

    /// Removes an event silently (no "ended" moment), e.g. a media player
    /// hidden because another one is preferred. Unlike dismissing, the
    /// source may show it again with a plain update.
    pub async fn withdraw(&self, id: &str) -> bool {
        let removed = self.engine.lock().await.withdraw(id, Timestamp::now());
        self.after_change(removed).await;
        removed
    }

    /// Withdraws every active event whose id starts with `prefix`.
    pub async fn withdraw_prefix(&self, prefix: &str) {
        let removed = self
            .engine
            .lock()
            .await
            .withdraw_prefix(prefix, Timestamp::now());
        self.after_change(removed).await;
    }

    /// Ends an active event because its source went away.
    pub async fn finish(&self, id: &str) -> bool {
        let finished = self.engine.lock().await.finish(id, Timestamp::now());
        self.after_change(finished).await;
        finished
    }

    pub async fn dismiss(&self, id: &str) -> bool {
        let removed = self.engine.lock().await.dismiss(id);
        self.after_change(removed).await;
        removed
    }

    pub async fn list_json(&self) -> String {
        self.engine.lock().await.list_json()
    }

    pub async fn len(&self) -> usize {
        self.engine.lock().await.len()
    }

    pub async fn is_empty(&self) -> bool {
        self.len().await == 0
    }

    /// Whether an event with this id exists and is still active.
    pub async fn is_active(&self, id: &str) -> bool {
        self.engine
            .lock()
            .await
            .get(id)
            .is_some_and(|e| !e.state.is_terminal())
    }

    pub async fn has_action(&self, id: &str, action_id: &str) -> bool {
        self.engine.lock().await.has_action(id, action_id)
    }

    /// Hands a pressed button to the integration that owns the event.
    /// Returns false if no built-in integration owns it.
    pub async fn route_action(&self, id: &str, action_id: &str) -> bool {
        let routes = self.routes.lock().await;
        let Some((_, sender)) = routes
            .iter()
            .find(|(prefix, _)| id.starts_with(prefix.as_str()))
        else {
            return false;
        };
        sender
            .send(ActionRequest {
                event_id: id.to_string(),
                action_id: action_id.to_string(),
            })
            .is_ok()
    }

    /// Claims all events whose id starts with `prefix`. Returns the receiver
    /// for their action buttons.
    pub async fn register_actions(&self, prefix: &str) -> mpsc::UnboundedReceiver<ActionRequest> {
        let (tx, rx) = mpsc::unbounded_channel();
        let mut routes = self.routes.lock().await;
        routes.retain(|(p, _)| p != prefix);
        routes.push((prefix.to_string(), tx));
        rx
    }

    pub async fn set_status(&self, id: &'static str, name: &'static str, status: Status) {
        let json = {
            let mut integrations = self.integrations.lock().await;
            let previous = integrations.get(id).map(|i| i.status.clone());
            if previous.as_ref() == Some(&status) {
                return;
            }
            let kind_changed = previous.as_ref().map(std::mem::discriminant)
                != Some(std::mem::discriminant(&status));
            if kind_changed {
                match &status {
                    Status::Failed(e) => eprintln!("Integration {id}: failed: {e}"),
                    Status::Unavailable(e) => eprintln!("Integration {id}: unavailable: {e}"),
                    Status::Active(_) => eprintln!("Integration {id}: active"),
                    Status::Disabled => eprintln!("Integration {id}: disabled"),
                    Status::Starting => {}
                }
            }
            integrations.insert(id, IntegrationInfo { id, name, status });
            let list: Vec<&IntegrationInfo> = integrations.values().collect();
            serde_json::to_string(&list).unwrap_or_else(|_| "[]".into())
        };
        if let Some(emitter) = self.emitter.get() {
            if let Err(e) = crate::dbus::Integrations::changed(emitter, &json).await {
                eprintln!("Could not emit Integrations Changed: {e}");
            }
        }
    }

    pub async fn integrations_json(&self) -> String {
        let integrations = self.integrations.lock().await;
        let list: Vec<&IntegrationInfo> = integrations.values().collect();
        serde_json::to_string(&list).unwrap_or_else(|_| "[]".into())
    }

    /// Expires due events. Returns whether anything changed.
    pub async fn tick(&self) -> bool {
        let changed = self.engine.lock().await.tick(Timestamp::now());
        self.after_change(changed).await;
        changed
    }

    pub async fn next_deadline(&self) -> Option<Timestamp> {
        self.engine.lock().await.next_deadline()
    }

    async fn after_change(&self, changed: bool) {
        self.wake.notify_one();
        if changed {
            self.changes.notify_one();
        }
    }

    /// Sends `Changed` with the current list: right away after a quiet
    /// moment, then at most once per `CHANGED_INTERVAL` while changes keep
    /// coming (each send carries the latest list), so a source that floods
    /// the daemon cannot make the island redraw thousands of times a second.
    pub async fn send_changes(self: std::sync::Arc<Self>) {
        loop {
            self.changes.notified().await;
            if let Some(emitter) = self.emitter.get() {
                let list = self.list_json().await;
                if let Err(e) = crate::dbus::Events::changed(emitter, &list).await {
                    eprintln!("Could not emit Changed: {e}");
                }
            }
            tokio::time::sleep(CHANGED_INTERVAL).await;
        }
    }
}

/// The known integration an event id belongs to ("pacman:transaction" →
/// "pacman"), if any.
fn owning_integration(json: &str) -> Option<&'static str> {
    let value: serde_json::Value = serde_json::from_str(json).ok()?;
    let prefix = value.get("id")?.as_str()?.split(':').next()?;
    crate::config::KNOWN_INTEGRATIONS
        .into_iter()
        .find(|known| *known == prefix)
}

#[cfg(test)]
mod tests {
    use super::owning_integration;

    #[test]
    fn events_from_outside_belong_to_known_integrations_only() {
        assert_eq!(
            owning_integration(r#"{"id":"pacman:transaction"}"#),
            Some("pacman")
        );
        assert_eq!(
            owning_integration(r#"{"id":"mpris:spotify"}"#),
            Some("mpris")
        );
        assert_eq!(owning_integration(r#"{"id":"cli-run-1-2"}"#), None);
        assert_eq!(owning_integration(r#"{"id":"browser:call:1"}"#), None);
        assert_eq!(owning_integration("not json"), None);
    }
}
