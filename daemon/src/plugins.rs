//! `com.selawi.Island.Plugins1`: apps that show their own events.
//!
//! An app registers with a manifest (core `plugins` module) on its own
//! D-Bus connection. Until the user allows it (a one-time question on the
//! island, or Settings → Integrations → Apps) it shows nothing. Its events
//! are sealed: their ids live under `plugin:<id>:`, and their name, icon
//! and kind come from the manifest. Buttons pressed on them go back to that
//! app only (a signal sent to its connection), and its events disappear
//! when it disconnects. Each app has a rate limit and at most
//! `MAX_EVENTS_PER_PLUGIN` events on the island.

use std::collections::BTreeMap;
use std::sync::{Arc, OnceLock};

use futures_util::StreamExt;
use selawi_core::events::{
    Action, Category, EventId, EventInput, LifecycleState, NotificationPayload, Payload, Source,
};
use selawi_core::plugins::{self, Manifest, RateLimit, MAX_EVENTS_PER_PLUGIN};
use selawi_core::{parse_input, Priority, Timestamp};
use serde::Serialize;
use tokio::sync::Mutex;
use zbus::message::Header;
use zbus::{interface, Connection};

use crate::dbus::{IslandError, OBJECT_PATH};
use crate::hub::Hub;

pub const INTERFACE: &str = "com.selawi.Island.Plugins1";
/// Ids of the "allow this app?" questions.
const REQUEST_PREFIX: &str = "plugins:request:";
const REQUEST_TTL_MS: u64 = 60_000;

struct Registered {
    manifest: Manifest,
    /// Unique bus name of the app's connection, while it is connected.
    owner: Option<String>,
    limit: RateLimit,
}

#[derive(Serialize)]
struct PluginInfo<'a> {
    id: &'a str,
    name: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    icon: Option<&'a str>,
    categories: Vec<Category>,
    /// Allowed by the user.
    allowed: bool,
    /// The user answered (allowed or not).
    decided: bool,
    /// Connected right now.
    running: bool,
}

pub struct PluginHost {
    hub: Arc<Hub>,
    plugins: Mutex<BTreeMap<String, Registered>>,
    connection: OnceLock<Connection>,
}

impl PluginHost {
    pub fn new(hub: Arc<Hub>) -> Arc<Self> {
        Arc::new(PluginHost {
            hub,
            plugins: Mutex::new(BTreeMap::new()),
            connection: OnceLock::new(),
        })
    }

    /// Starts following disconnects, button presses and settings changes.
    pub fn start(self: &Arc<Self>, connection: Connection) {
        let _ = self.connection.set(connection);
        tokio::spawn(self.clone().follow_disconnects());
        tokio::spawn(self.clone().route_actions());
        tokio::spawn(self.clone().follow_config());
    }

    async fn plugin_of(&self, sender: &str) -> Option<String> {
        let plugins = self.plugins.lock().await;
        plugins
            .iter()
            .find(|(_, p)| p.owner.as_deref() == Some(sender))
            .map(|(id, _)| id.clone())
    }

    async fn register(
        &self,
        sender: &str,
        manifest_json: &str,
    ) -> Result<(String, bool), IslandError> {
        let manifest = plugins::parse_manifest(manifest_json)
            .map_err(|e| IslandError::InvalidPlugin(e.to_string()))?;
        let id = manifest.id.clone();
        {
            let mut plugins = self.plugins.lock().await;
            if let Some(existing) = plugins.get(&id) {
                if existing.owner.as_deref().is_some_and(|o| o != sender) {
                    return Err(IslandError::InvalidPlugin(format!(
                        "plugin {id:?} is already registered by another running app"
                    )));
                }
            }
            if plugins
                .iter()
                .any(|(other, p)| other != &id && p.owner.as_deref() == Some(sender))
            {
                return Err(IslandError::InvalidPlugin(
                    "this connection already registered another plugin".into(),
                ));
            }
            plugins.insert(
                id.clone(),
                Registered {
                    manifest: manifest.clone(),
                    owner: Some(sender.to_string()),
                    limit: RateLimit::new(Timestamp::now()),
                },
            );
        }
        let config = self.hub.config();
        let allowed = config.plugin_allowed(&id);
        if !config.plugins.contains_key(&id) {
            self.ask(&manifest).await;
        }
        self.emit_changed().await;
        Ok((id, allowed))
    }

    /// The one-time question on the island: allow this app?
    async fn ask(&self, manifest: &Manifest) {
        let Ok(id) = EventId::new(format!("{REQUEST_PREFIX}{}", manifest.id)) else {
            return;
        };
        let body = manifest
            .description
            .clone()
            .unwrap_or_else(|| "Answer any time in Settings → Integrations → Apps.".into());
        let input = EventInput {
            id,
            source: Source {
                app: manifest.name.clone(),
                integration: "plugins".into(),
                icon: manifest.icon.clone(),
            },
            category: Category::Notification,
            state: LifecycleState::Started,
            priority: Some(Priority::Notification),
            persistent: false,
            ttl_ms: Some(REQUEST_TTL_MS),
            group: None,
            payload: Payload::Notification(NotificationPayload {
                title: format!("{} wants to show things on the island", manifest.name),
                body: Some(body),
            }),
            actions: vec![
                Action {
                    id: "allow".into(),
                    label: "Allow".into(),
                },
                Action {
                    id: "deny".into(),
                    label: "Don't allow".into(),
                },
            ],
        };
        if let Err(e) = self.hub.submit(input).await {
            eprintln!("Could not ask about a plugin: {e}");
        }
    }

    async fn submit(&self, sender: &str, event_json: &str) -> Result<(), IslandError> {
        let id = self.plugin_of(sender).await.ok_or_else(|| {
            IslandError::NotRegistered("call Register with a manifest first".into())
        })?;
        if !self.hub.config().plugin_allowed(&id) {
            return Err(IslandError::NotAllowed(format!(
                "the user has not allowed {id:?} (Settings → Integrations → Apps)"
            )));
        }
        let manifest = {
            let mut plugins = self.plugins.lock().await;
            let Some(plugin) = plugins.get_mut(&id) else {
                return Err(IslandError::NotRegistered("not registered".into()));
            };
            if !plugin.limit.allow(Timestamp::now()) {
                return Err(IslandError::RateLimited(
                    "too many events: at most 10 a second (short bursts of 30)".into(),
                ));
            }
            plugin.manifest.clone()
        };
        // `source` is always the manifest's, so the plugin may leave it out.
        let mut value: serde_json::Value = serde_json::from_str(event_json)
            .map_err(|e| IslandError::InvalidEvent(e.to_string()))?;
        if let Some(object) = value.as_object_mut() {
            object.insert(
                "source".into(),
                serde_json::json!({"app": manifest.name, "integration": plugins::PLUGIN_INTEGRATION}),
            );
        }
        let input = parse_input(&value.to_string())
            .map_err(|e| IslandError::InvalidEvent(e.to_string()))?;
        let input = plugins::seal(&manifest, input)
            .map_err(|e| IslandError::InvalidEvent(e.to_string()))?;
        let prefix = plugins::event_prefix(&id);
        if !self.hub.is_active(input.id.as_str()).await
            && self.hub.count_active(&prefix).await >= MAX_EVENTS_PER_PLUGIN
        {
            return Err(IslandError::InvalidEvent(format!(
                "at most {MAX_EVENTS_PER_PLUGIN} events at a time: end one first"
            )));
        }
        self.hub
            .submit(input)
            .await
            .map_err(|e| IslandError::InvalidEvent(e.to_string()))
    }

    /// Removes one of the app's own events (by the id it used).
    async fn remove(&self, sender: &str, local_id: &str) -> Result<bool, IslandError> {
        let id = self.plugin_of(sender).await.ok_or_else(|| {
            IslandError::NotRegistered("call Register with a manifest first".into())
        })?;
        Ok(self
            .hub
            .withdraw(&format!("{}{local_id}", plugins::event_prefix(&id)))
            .await)
    }

    pub async fn list_json(&self) -> String {
        let config = self.hub.config();
        let plugins = self.plugins.lock().await;
        let mut list: Vec<PluginInfo<'_>> = plugins
            .iter()
            .map(|(id, p)| PluginInfo {
                id,
                name: &p.manifest.name,
                description: p.manifest.description.as_deref(),
                icon: p.manifest.icon.as_deref(),
                categories: p.manifest.categories.clone(),
                allowed: config.plugin_allowed(id),
                decided: config.plugins.contains_key(id),
                running: p.owner.is_some(),
            })
            .collect();
        // Apps answered in an earlier session that have not started since.
        for (id, allowed) in &config.plugins {
            if !plugins.contains_key(id) {
                list.push(PluginInfo {
                    id,
                    name: id,
                    description: None,
                    icon: None,
                    categories: Vec::new(),
                    allowed: *allowed,
                    decided: true,
                    running: false,
                });
            }
        }
        serde_json::to_string(&list).unwrap_or_else(|_| "[]".into())
    }

    async fn emit_changed(&self) {
        let Some(connection) = self.connection.get() else {
            return;
        };
        let json = self.list_json().await;
        if let Err(e) = connection
            .emit_signal(None::<&str>, OBJECT_PATH, INTERFACE, "Changed", &(json,))
            .await
        {
            eprintln!("Could not emit Plugins Changed: {e}");
        }
    }

    /// An app's connection closed: its events go, it shows as not running.
    async fn disconnected(&self, unique_name: &str) {
        let gone: Vec<String> = {
            let mut plugins = self.plugins.lock().await;
            plugins
                .iter_mut()
                .filter(|(_, p)| p.owner.as_deref() == Some(unique_name))
                .map(|(id, p)| {
                    p.owner = None;
                    id.clone()
                })
                .collect()
        };
        if gone.is_empty() {
            return;
        }
        for id in gone {
            self.hub.withdraw_prefix(&plugins::event_prefix(&id)).await;
        }
        self.emit_changed().await;
    }

    async fn follow_disconnects(self: Arc<Self>) {
        let Some(connection) = self.connection.get().cloned() else {
            return;
        };
        let Ok(bus) = zbus::fdo::DBusProxy::new(&connection).await else {
            eprintln!("Plugins: cannot watch disconnects");
            return;
        };
        let Ok(mut stream) = bus.receive_name_owner_changed_with_args(&[(2, "")]).await else {
            eprintln!("Plugins: cannot watch disconnects");
            return;
        };
        while let Some(signal) = stream.next().await {
            if let Ok(args) = signal.args() {
                let name = args.name().as_str();
                if name.starts_with(':') {
                    self.disconnected(name).await;
                }
            }
        }
    }

    /// Buttons: on an app's event → that app only; on a question → the
    /// user's answer.
    async fn route_actions(self: Arc<Self>) {
        let mut events = self.hub.register_actions(plugins::PLUGIN_ID_PREFIX).await;
        let mut requests = self.hub.register_actions(REQUEST_PREFIX).await;
        loop {
            tokio::select! {
                Some(request) = events.recv() => self.deliver(&request.event_id, &request.action_id).await,
                Some(request) = requests.recv() => self.answer(&request.event_id, &request.action_id).await,
                else => return,
            }
        }
    }

    async fn deliver(&self, event_id: &str, action_id: &str) {
        let target = {
            let plugins = self.plugins.lock().await;
            plugins.iter().find_map(|(id, p)| {
                let local = plugins::local_id(id, event_id)?;
                Some((p.owner.clone()?, local.to_string()))
            })
        };
        let (Some(connection), Some((owner, local))) = (self.connection.get(), target) else {
            return;
        };
        if let Err(e) = connection
            .emit_signal(
                Some(owner.as_str()),
                OBJECT_PATH,
                INTERFACE,
                "ActionInvoked",
                &(local, action_id.to_string()),
            )
            .await
        {
            eprintln!("Could not deliver a plugin action: {e}");
        }
    }

    async fn answer(&self, event_id: &str, action_id: &str) {
        let Some(id) = event_id.strip_prefix(REQUEST_PREFIX) else {
            return;
        };
        let mut config = self.hub.config();
        config.plugins.insert(id.to_string(), action_id == "allow");
        match serde_json::to_string(&config) {
            Ok(json) => {
                if let Err(e) = self.hub.set_config(&json) {
                    eprintln!("Could not save the plugin answer: {e}");
                }
            }
            Err(e) => eprintln!("Could not save the plugin answer: {e}"),
        }
    }

    /// Settings changed: hide the events of apps no longer allowed, drop
    /// questions that were answered, and tell the settings window.
    async fn follow_config(self: Arc<Self>) {
        let mut config = self.hub.watch_config();
        while config.changed().await.is_ok() {
            let current = config.borrow_and_update().clone();
            let ids: Vec<String> = self.plugins.lock().await.keys().cloned().collect();
            for id in ids {
                if !current.plugin_allowed(&id) {
                    self.hub.withdraw_prefix(&plugins::event_prefix(&id)).await;
                }
                if current.plugins.contains_key(&id) {
                    self.hub.withdraw(&format!("{REQUEST_PREFIX}{id}")).await;
                }
            }
            self.emit_changed().await;
        }
    }
}

pub struct Plugins {
    pub host: Arc<PluginHost>,
}

fn sender(header: &Header<'_>) -> Result<String, IslandError> {
    header
        .sender()
        .map(|s| s.to_string())
        .ok_or_else(|| IslandError::NotRegistered("no sender".into()))
}

#[interface(name = "com.selawi.Island.Plugins1")]
impl Plugins {
    /// Registers the calling connection as the plugin described by the
    /// manifest. Returns its id and whether the user has allowed it.
    async fn register(
        &self,
        manifest_json: &str,
        #[zbus(header)] header: Header<'_>,
    ) -> Result<(String, bool), IslandError> {
        self.host.register(&sender(&header)?, manifest_json).await
    }

    /// Adds or updates one of the plugin's events (protocol v1 event input
    /// as JSON; its `id` is the plugin's own, `source` is replaced).
    async fn submit(
        &self,
        event_json: &str,
        #[zbus(header)] header: Header<'_>,
    ) -> Result<(), IslandError> {
        self.host.submit(&sender(&header)?, event_json).await
    }

    /// Removes one of the plugin's events at once. Returns whether it existed.
    async fn remove(
        &self,
        id: &str,
        #[zbus(header)] header: Header<'_>,
    ) -> Result<bool, IslandError> {
        self.host.remove(&sender(&header)?, id).await
    }

    /// Every known plugin, as JSON (for the settings window).
    async fn list(&self) -> String {
        self.host.list_json().await
    }

    /// The plugin list or an answer changed. Carries the same JSON as `List`.
    #[zbus(signal)]
    async fn changed(
        emitter: &zbus::object_server::SignalEmitter<'_>,
        plugins_json: &str,
    ) -> zbus::Result<()>;

    /// A button was pressed on one of the plugin's events. Sent only to
    /// that plugin's connection; `id` is the plugin's own event id.
    #[zbus(signal)]
    async fn action_invoked(
        emitter: &zbus::object_server::SignalEmitter<'_>,
        id: &str,
        action_id: &str,
    ) -> zbus::Result<()>;
}
