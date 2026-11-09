//! D-Bus interfaces on `com.selawi.Island` at `/com/selawi/Island`.
//!
//! - `com.selawi.Island.Core1`: version information.
//! - `com.selawi.Island.Events1`: submit, list, dismiss, actions.
//!
//! See docs/DBUS.md for the full contract.

use std::sync::Arc;

use selawi_core::{Timestamp, PROTOCOL_VERSION};
use tokio::sync::{Mutex, Notify};
use zbus::object_server::SignalEmitter;
use zbus::{interface, DBusError};

use crate::engine::{Engine, SubmitError};

pub const BUS_NAME: &str = "com.selawi.Island";
pub const OBJECT_PATH: &str = "/com/selawi/Island";

/// State shared by the interfaces and the expiry task.
pub struct Shared {
    pub engine: Mutex<Engine>,
    /// Wakes the expiry task whenever the next deadline may have changed.
    pub wake: Notify,
}

#[derive(Debug, DBusError)]
#[zbus(prefix = "com.selawi.Island.Error")]
pub enum IslandError {
    #[zbus(error)]
    ZBus(zbus::Error),
    /// The event JSON was malformed or failed validation.
    InvalidEvent(String),
    /// No event with that id, or it has no such action.
    UnknownAction(String),
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
    pub shared: Arc<Shared>,
}

#[interface(name = "com.selawi.Island.Events1")]
impl Events {
    /// Adds or updates one event (protocol v1 event input as JSON).
    async fn submit(
        &self,
        event_json: &str,
        #[zbus(signal_emitter)] emitter: SignalEmitter<'_>,
    ) -> Result<(), IslandError> {
        let list = {
            let mut engine = self.shared.engine.lock().await;
            match engine.submit(event_json, Timestamp::now()) {
                Ok(true) => Some(engine.list_json()),
                Ok(false) => None,
                Err(SubmitError::TooLarge) => {
                    return Err(IslandError::InvalidEvent(SubmitError::TooLarge.to_string()))
                }
                Err(e) => return Err(IslandError::InvalidEvent(e.to_string())),
            }
        };
        self.shared.wake.notify_one();
        if let Some(list) = list {
            Self::changed(&emitter, &list).await?;
        }
        Ok(())
    }

    /// All current events, most important first, as a JSON array.
    async fn list(&self) -> String {
        self.shared.engine.lock().await.list_json()
    }

    /// Removes an event because the user dismissed it. Returns whether it existed.
    async fn dismiss(
        &self,
        id: &str,
        #[zbus(signal_emitter)] emitter: SignalEmitter<'_>,
    ) -> Result<bool, IslandError> {
        let list = {
            let mut engine = self.shared.engine.lock().await;
            engine.dismiss(id).then(|| engine.list_json())
        };
        self.shared.wake.notify_one();
        match list {
            Some(list) => {
                Self::changed(&emitter, &list).await?;
                Ok(true)
            }
            None => Ok(false),
        }
    }

    /// Forwards a pressed action button to the event's source through the
    /// `ActionInvoked` signal. Only actions the event declared are accepted.
    async fn invoke_action(
        &self,
        id: &str,
        action_id: &str,
        #[zbus(signal_emitter)] emitter: SignalEmitter<'_>,
    ) -> Result<(), IslandError> {
        if !self.shared.engine.lock().await.has_action(id, action_id) {
            return Err(IslandError::UnknownAction(format!(
                "event has no action \"{action_id}\""
            )));
        }
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
