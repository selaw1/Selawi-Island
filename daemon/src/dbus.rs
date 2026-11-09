//! D-Bus interfaces on `com.selawi.Island` at `/com/selawi/Island`.
//!
//! - `com.selawi.Island.Core1`: version information.
//! - `com.selawi.Island.Events1`: submit, list, dismiss, actions.
//! - `com.selawi.Island.Integrations1`: what each built-in integration is doing.
//! - `com.selawi.Island.Configuration1`: daemon settings (integrations, media,
//!   notifications).
//!
//! See docs/DBUS.md for the full contract.

use std::sync::Arc;

use selawi_core::PROTOCOL_VERSION;
use zbus::object_server::SignalEmitter;
use zbus::{interface, DBusError};

use crate::hub::Hub;

pub const BUS_NAME: &str = "com.selawi.Island";
pub const OBJECT_PATH: &str = "/com/selawi/Island";

#[derive(Debug, DBusError)]
#[zbus(prefix = "com.selawi.Island.Error")]
pub enum IslandError {
    #[zbus(error)]
    ZBus(zbus::Error),
    /// The event JSON was malformed or failed validation.
    InvalidEvent(String),
    /// No event with that id, or it has no such action.
    UnknownAction(String),
    /// The configuration JSON was malformed, invalid, or could not be saved.
    InvalidConfig(String),
}

pub struct Core;

#[interface(name = "com.selawi.Island.Core1")]
impl Core {
    #[zbus(property)]
    fn version(&self) -> String {
        env!("CARGO_PKG_VERSION").to_string()
    }

    #[zbus(property)]
    fn protocol_version(&self) -> u32 {
        PROTOCOL_VERSION
    }
}

pub struct Events {
    pub hub: Arc<Hub>,
}

#[interface(name = "com.selawi.Island.Events1")]
impl Events {
    /// Adds or updates one event (protocol v1 event input as JSON).
    async fn submit(&self, event_json: &str) -> Result<(), IslandError> {
        self.hub
            .submit_json(event_json)
            .await
            .map_err(|e| IslandError::InvalidEvent(e.to_string()))
    }

    /// All current events, most important first, as a JSON array.
    async fn list(&self) -> String {
        self.hub.list_json().await
    }

    /// Removes an event because the user dismissed it. Returns whether it existed.
    async fn dismiss(&self, id: &str) -> bool {
        self.hub.dismiss(id).await
    }

    /// A pressed action button. Only actions the event declared are
    /// accepted. Built-in integrations receive it directly; every action is
    /// also announced with `ActionInvoked` for external sources.
    async fn invoke_action(
        &self,
        id: &str,
        action_id: &str,
        #[zbus(signal_emitter)] emitter: SignalEmitter<'_>,
    ) -> Result<(), IslandError> {
        if !self.hub.has_action(id, action_id).await {
            return Err(IslandError::UnknownAction(format!(
                "event has no action \"{action_id}\""
            )));
        }
        self.hub.route_action(id, action_id).await;
        Self::action_invoked(&emitter, id, action_id).await?;
        Ok(())
    }

    /// The event list changed. Carries the same JSON as `List`.
    #[zbus(signal)]
    pub async fn changed(emitter: &SignalEmitter<'_>, events_json: &str) -> zbus::Result<()>;

    /// The user pressed an action button on an event.
    #[zbus(signal)]
    async fn action_invoked(
        emitter: &SignalEmitter<'_>,
        id: &str,
        action_id: &str,
    ) -> zbus::Result<()>;
}

pub struct Integrations {
    pub hub: Arc<Hub>,
}

#[interface(name = "com.selawi.Island.Integrations1")]
impl Integrations {
    /// Every built-in integration with its status, as a JSON array of
    /// `{"id", "name", "status", "detail"}`.
    async fn list(&self) -> String {
        self.hub.integrations_json().await
    }

    /// Media players currently running, as a JSON array of `{"id", "name"}`.
    /// `id` is the key used by `media.preferred_player`.
    async fn media_players(&self) -> String {
        self.hub.media_players_json().await
    }

    /// An integration's status changed. Carries the same JSON as `List`.
    #[zbus(signal)]
    pub async fn changed(emitter: &SignalEmitter<'_>, integrations_json: &str) -> zbus::Result<()>;
}

pub struct Configuration {
    pub hub: Arc<Hub>,
}

#[interface(name = "com.selawi.Island.Configuration1")]
impl Configuration {
    /// The whole configuration as JSON (see docs/INTEGRATIONS.md).
    async fn get(&self) -> String {
        self.hub.config().effective().to_json()
    }

    /// Replaces the configuration. Applied immediately and saved.
    async fn set(
        &self,
        config_json: &str,
        #[zbus(signal_emitter)] emitter: SignalEmitter<'_>,
    ) -> Result<(), IslandError> {
        let config = self
            .hub
            .set_config(config_json)
            .map_err(IslandError::InvalidConfig)?;
        Self::changed(&emitter, &config.effective().to_json()).await?;
        Ok(())
    }

    /// The configuration changed. Carries the same JSON as `Get`.
    #[zbus(signal)]
    async fn changed(emitter: &SignalEmitter<'_>, config_json: &str) -> zbus::Result<()>;
}
