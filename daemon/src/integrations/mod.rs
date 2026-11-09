//! Built-in Linux integrations. Each one watches an OS service over D-Bus
//! and turns its signals into events. None of them poll.
//!
//! An integration is an async function that runs until its source goes
//! away or fails; the runner then marks it and tries again later.

pub mod bluez;
pub mod mpris;
pub mod upower;
pub mod variant;

use std::future::Future;
use std::sync::Arc;
use std::time::Duration;

use crate::config::Config;
use crate::hub::{Hub, Status};

/// Why an integration stopped.
#[derive(Debug)]
pub enum Stop {
    /// The source does not exist on this system. Checked again later.
    Unavailable(String),
    /// Something went wrong. Retried.
    Failed(String),
}

impl From<zbus::Error> for Stop {
    fn from(e: zbus::Error) -> Self {
        let text = e.to_string();
        if text.contains("ServiceUnknown") || text.contains("NameHasNoOwner") {
            Stop::Unavailable("service is not running".into())
        } else {
            Stop::Failed(text)
        }
    }
}

impl From<zbus::fdo::Error> for Stop {
    fn from(e: zbus::fdo::Error) -> Self {
        Stop::from(zbus::Error::from(e))
    }
}

const RETRY_AFTER: Duration = Duration::from_secs(30);

/// Runs `run` forever (unless disabled), restarting it after failures.
pub fn spawn<F, Fut>(hub: Arc<Hub>, config: &Config, id: &'static str, name: &'static str, run: F)
where
    F: Fn(Arc<Hub>) -> Fut + Send + 'static,
    Fut: Future<Output = Result<(), Stop>> + Send + 'static,
{
    let enabled = config.enabled(id);
    tokio::spawn(async move {
        if !enabled {
            hub.set_status(id, name, Status::Disabled).await;
            return;
        }
        loop {
            hub.set_status(id, name, Status::Starting).await;
            let status = match run(hub.clone()).await {
                Ok(()) => Status::Unavailable("source went away".into()),
                Err(Stop::Unavailable(why)) => Status::Unavailable(why),
                Err(Stop::Failed(why)) => Status::Failed(why),
            };
            hub.set_status(id, name, status).await;
            tokio::time::sleep(RETRY_AFTER).await;
        }
    });
}

/// Makes any string usable inside an event id (`A-Z a-z 0-9 . _ : -`).
pub fn id_part(raw: &str) -> String {
    let cleaned: String = raw
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-') {
                c
            } else {
                '_'
            }
        })
        .take(100)
        .collect();
    if cleaned.is_empty() {
        "unknown".into()
    } else {
        cleaned
    }
}

#[cfg(test)]
mod tests {
    use super::id_part;

    #[test]
    fn id_part_is_always_valid() {
        assert_eq!(id_part("firefox.instance_1_23"), "firefox.instance_1_23");
        assert_eq!(
            id_part("/org/bluez/hci0/dev_AA:BB"),
            "_org_bluez_hci0_dev_AA_BB"
        );
        assert_eq!(id_part(""), "unknown");
        assert_eq!(id_part(&"x".repeat(500)).len(), 100);
    }
}
