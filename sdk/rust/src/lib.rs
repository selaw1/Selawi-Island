//! Show your app's own events on Selawi Island.
//!
//! ```no_run
//! # async fn demo() -> zbus::Result<()> {
//! use futures_util::StreamExt;
//! use selawi_island_plugin::Island;
//!
//! let island = Island::register(r#"{"manifest_version":1,"id":"pomodoro",
//!     "name":"Pomodoro","categories":["custom"]}"#).await?;
//! if !island.allowed() {
//!     println!("Allow Pomodoro on the island (it asks), then run again");
//! }
//! island.show(r#"{"id":"timer",
//!     "category":"custom","state":"started","persistent":true,
//!     "payload":{"type":"progress","title":"Focus","percent":40},
//!     "actions":[{"id":"pause","label":"Pause"}]}"#).await?;
//! let mut pressed = island.actions().await?;
//! while let Some((event_id, action_id)) = pressed.next().await {
//!     println!("{action_id} on {event_id}");
//! }
//! # Ok(()) }
//! ```
//!
//! Events are protocol v1 event inputs (docs/EVENT_SYSTEM.md). Use your own
//! ids (no `:`); the island puts them under your plugin and fills in
//! `source` from your manifest. They disappear when the program exits.
//! See docs/PLUGINS.md.

use futures_util::{Stream, StreamExt};
use zbus::{proxy, Connection};

#[proxy(
    interface = "com.selawi.Island.Plugins1",
    default_service = "com.selawi.Island",
    default_path = "/com/selawi/Island"
)]
pub trait Plugins {
    /// Returns the plugin id and whether the user has allowed it.
    fn register(&self, manifest_json: &str) -> zbus::Result<(String, bool)>;
    fn submit(&self, event_json: &str) -> zbus::Result<()>;
    fn remove(&self, id: &str) -> zbus::Result<bool>;
    fn list(&self) -> zbus::Result<String>;
    #[zbus(signal)]
    fn action_invoked(&self, id: String, action_id: String) -> zbus::Result<()>;
    #[zbus(signal)]
    fn changed(&self, plugins_json: String) -> zbus::Result<()>;
}

/// A registered plugin. Keep it alive while your events should show: they
/// go when its connection closes.
pub struct Island {
    proxy: PluginsProxy<'static>,
    id: String,
    allowed: bool,
}

impl Island {
    /// Connects to the session bus and registers with `manifest_json`.
    pub async fn register(manifest_json: &str) -> zbus::Result<Island> {
        Self::register_on(&Connection::session().await?, manifest_json).await
    }

    /// Registers on an existing connection (one plugin per connection).
    pub async fn register_on(connection: &Connection, manifest_json: &str) -> zbus::Result<Island> {
        let proxy = PluginsProxy::new(connection).await?;
        let (id, allowed) = proxy.register(manifest_json).await?;
        Ok(Island { proxy, id, allowed })
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    /// Whether the user had allowed the plugin when it registered (see
    /// [`Island::allowed_changes`] to follow later answers).
    pub fn allowed(&self) -> bool {
        self.allowed
    }

    /// Adds or updates an event (protocol v1 event input as JSON).
    pub async fn show(&self, event_json: &str) -> zbus::Result<()> {
        self.proxy.submit(event_json).await
    }

    /// Removes one of your events at once.
    pub async fn remove(&self, id: &str) -> zbus::Result<bool> {
        self.proxy.remove(id).await
    }

    /// Button presses on your events: `(your event id, action id)`.
    pub async fn actions(&self) -> zbus::Result<impl Stream<Item = (String, String)> + Unpin> {
        Ok(self
            .proxy
            .receive_action_invoked()
            .await?
            .filter_map(|signal| {
                std::future::ready(
                    signal
                        .args()
                        .ok()
                        .map(|a| (a.id().to_string(), a.action_id().to_string())),
                )
            }))
    }

    /// Whether the plugin is allowed, each time the user changes it.
    pub async fn allowed_changes(&self) -> zbus::Result<impl Stream<Item = bool> + Unpin> {
        let id = self.id.clone();
        Ok(self
            .proxy
            .receive_changed()
            .await?
            .filter_map(move |signal| std::future::ready(allowed_in(&signal, &id))))
    }
}

/// This plugin's `allowed` in a `Changed` signal's list.
fn allowed_in(signal: &Changed, id: &str) -> Option<bool> {
    let args = signal.args().ok()?;
    let list: serde_json::Value = serde_json::from_str(args.plugins_json()).ok()?;
    list.as_array()?
        .iter()
        .find(|p| p.get("id").and_then(|v| v.as_str()) == Some(id))
        .and_then(|p| p.get("allowed")?.as_bool())
}
