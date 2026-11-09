//! The hub connects everything inside the daemon: the D-Bus interfaces,
//! the built-in integrations and the expiry task all go through it.

use std::collections::BTreeMap;
use std::sync::OnceLock;

use selawi_core::{EventInput, Timestamp};
use serde::Serialize;
use tokio::sync::{mpsc, Mutex, Notify};
use zbus::object_server::SignalEmitter;

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

#[derive(Debug, Clone, Serialize)]
pub struct IntegrationInfo {
    pub id: &'static str,
    pub name: &'static str,
    #[serde(flatten)]
    pub status: Status,
}

pub struct Hub {
    engine: Mutex<Engine>,
    /// Wakes the expiry task whenever the next deadline may have changed.
    pub wake: Notify,
    emitter: OnceLock<SignalEmitter<'static>>,
    /// Event id prefix → the integration that owns those events.
    routes: Mutex<Vec<(String, mpsc::UnboundedSender<ActionRequest>)>>,
    integrations: Mutex<BTreeMap<&'static str, IntegrationInfo>>,
}

impl Hub {
    pub fn new(engine: Engine) -> Self {
        Self {
            engine: Mutex::new(engine),
            wake: Notify::new(),
            emitter: OnceLock::new(),
            routes: Mutex::new(Vec::new()),
            integrations: Mutex::new(BTreeMap::new()),
        }
    }

    /// Set once the D-Bus object is registered, so changes can be announced.
    pub fn set_emitter(&self, emitter: SignalEmitter<'static>) {
        let _ = self.emitter.set(emitter);
    }

    pub async fn submit_json(&self, json: &str) -> Result<(), SubmitError> {
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
        let mut integrations = self.integrations.lock().await;
        let changed = integrations.get(id).map(|i| &i.status) != Some(&status);
        if changed {
            match &status {
                Status::Failed(e) => eprintln!("Integration {id}: failed: {e}"),
                Status::Unavailable(e) => eprintln!("Integration {id}: unavailable: {e}"),
                Status::Active(_) => eprintln!("Integration {id}: active"),
                Status::Disabled | Status::Starting => {}
            }
        }
        integrations.insert(id, IntegrationInfo { id, name, status });
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
        if !changed {
            return;
        }
        let Some(emitter) = self.emitter.get() else {
            return;
        };
        let list = self.list_json().await;
        if let Err(e) = crate::dbus::Events::changed(emitter, &list).await {
            eprintln!("Could not emit Changed: {e}");
        }
    }
}
