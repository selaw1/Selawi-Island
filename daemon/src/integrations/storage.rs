//! USB and removable drives through UDisks2 (system bus).
//!
//! - A USB or removable drive appears → "<Vendor Model> · USB drive connected"
//! - It goes away → "<Vendor Model> · USB drive removed"
//!
//! Drives present at startup are not announced. Other USB devices
//! (keyboards, phones, hubs) are not reported by UDisks2 and are not shown.

use std::collections::HashMap;
use std::sync::Arc;

use futures_util::StreamExt;
use selawi_core::events::{
    Category, EventId, EventInput, LifecycleState, Payload, Severity, Source, SystemPayload,
};
use zbus::fdo::ObjectManagerProxy;
use zbus::Connection;

use super::variant::{self, Props};
use super::{id_part, Stop};
use crate::hub::{Hub, Status};

pub const ID: &str = "storage";
pub const NAME: &str = "USB drives (UDisks2)";
const DESTINATION: &str = "org.freedesktop.UDisks2";
const MANAGER_PATH: &str = "/org/freedesktop/UDisks2";
const DRIVE_IFACE: &str = "org.freedesktop.UDisks2.Drive";

/// Display name for a removable drive, or `None` for internal disks.
pub fn removable_drive(props: &Props) -> Option<String> {
    let usb = variant::string(props, "ConnectionBus").as_deref() == Some("usb");
    let removable = variant::boolean(props, "Removable").unwrap_or(false)
        || variant::boolean(props, "MediaRemovable").unwrap_or(false);
    if !usb && !removable {
        return None;
    }
    let vendor = variant::string(props, "Vendor").unwrap_or_default();
    let model = variant::string(props, "Model").unwrap_or_default();
    let name = format!("{} {}", vendor.trim(), model.trim())
        .trim()
        .replace('_', " ");
    let name: String = name.chars().filter(|c| !c.is_control()).take(256).collect();
    Some(if name.trim().is_empty() {
        "USB drive".into()
    } else {
        name
    })
}

pub fn drive_event(path: &str, name: &str, connected: bool) -> Option<EventInput> {
    let tail = path.rsplit('/').next().unwrap_or(path);
    let input = EventInput {
        id: EventId::new(format!("{ID}:{}", id_part(tail))).ok()?,
        source: Source {
            app: "Drives".into(),
            integration: ID.into(),
            icon: None,
        },
        category: Category::Device,
        state: LifecycleState::Started,
        priority: None,
        persistent: false,
        ttl_ms: Some(4_000),
        group: None,
        payload: Payload::System(SystemPayload {
            title: name.into(),
            detail: Some(
                if connected {
                    "USB drive connected"
                } else {
                    "USB drive removed"
                }
                .into(),
            ),
            severity: Severity::Info,
        }),
        actions: Vec::new(),
    };
    input.validate().ok().map(|()| input)
}

pub async fn run(hub: Arc<Hub>) -> Result<(), Stop> {
    let conn = Connection::system()
        .await
        .map_err(|_| Stop::Unavailable("system bus not available".into()))?;
    watch(hub, conn).await
}

/// Watches UDisks2 on `conn` (the system bus in production).
pub async fn watch(hub: Arc<Hub>, conn: Connection) -> Result<(), Stop> {
    let manager = ObjectManagerProxy::builder(&conn)
        .destination(DESTINATION)
        .path(MANAGER_PATH)
        .build()
        .await?;
    let mut added = manager.receive_interfaces_added().await?;
    let mut removed = manager.receive_interfaces_removed().await?;

    // Drives already present: remember them (to know their names on
    // removal) but announce nothing.
    let mut drives: HashMap<String, String> = HashMap::new();
    for (path, interfaces) in manager.get_managed_objects().await? {
        if let Some(props) = interfaces
            .iter()
            .find(|(i, _)| i.as_str() == DRIVE_IFACE)
            .map(|(_, p)| p)
        {
            if let Some(name) = removable_drive(props) {
                drives.insert(path.to_string(), name);
            }
        }
    }
    set_active(&hub, drives.len()).await;

    loop {
        tokio::select! {
            signal = added.next() => {
                let Some(signal) = signal else { break };
                let Ok(args) = signal.args() else { continue };
                let path = args.object_path().to_string();
                for (iface, props) in args.interfaces_and_properties() {
                    if iface.as_str() != DRIVE_IFACE {
                        continue;
                    }
                    let props: Props = props
                        .iter()
                        .filter_map(|(k, v)| v.try_to_owned().ok().map(|o| (k.to_string(), o)))
                        .collect();
                    if let Some(name) = removable_drive(&props) {
                        publish(&hub, drive_event(&path, &name, true)).await;
                        drives.insert(path.clone(), name);
                    }
                }
                set_active(&hub, drives.len()).await;
            }
            signal = removed.next() => {
                let Some(signal) = signal else { break };
                let Ok(args) = signal.args() else { continue };
                let path = args.object_path().to_string();
                if args.interfaces().iter().any(|i| i.as_str() == DRIVE_IFACE) {
                    if let Some(name) = drives.remove(&path) {
                        publish(&hub, drive_event(&path, &name, false)).await;
                    }
                }
                set_active(&hub, drives.len()).await;
            }
        }
    }
    Err(Stop::Unavailable("UDisks2 went away".into()))
}

async fn publish(hub: &Hub, input: Option<EventInput>) {
    if let Some(input) = input {
        if let Err(e) = hub.submit(input).await {
            eprintln!("Drive event rejected: {e}");
        }
    }
}

async fn set_active(hub: &Hub, count: usize) {
    let detail = match count {
        1 => "1 removable drive".to_string(),
        n => format!("{n} removable drives"),
    };
    hub.set_status(ID, NAME, Status::Active(detail)).await;
}

#[cfg(test)]
mod tests {
    use super::*;
    use zbus::Variant;

    fn props(entries: Vec<(&str, Variant<'_>)>) -> Props {
        entries
            .into_iter()
            .filter_map(|(k, v)| v.try_into_owned().ok().map(|o| (k.to_string(), o)))
            .collect()
    }

    #[test]
    fn usb_drives_are_named_and_internal_disks_ignored() {
        let usb = props(vec![
            ("ConnectionBus", Variant::from("usb")),
            ("Vendor", Variant::from("SanDisk")),
            ("Model", Variant::from("Ultra_Fit")),
        ]);
        assert_eq!(removable_drive(&usb).as_deref(), Some("SanDisk Ultra Fit"));
        let internal = props(vec![
            ("ConnectionBus", Variant::from("")),
            ("Model", Variant::from("NVMe SSD")),
        ]);
        assert_eq!(removable_drive(&internal), None);
        let card = props(vec![("MediaRemovable", Variant::from(true))]);
        assert_eq!(removable_drive(&card).as_deref(), Some("USB drive"));
    }

    #[test]
    fn events_are_valid() -> Result<(), &'static str> {
        let input = drive_event(
            "/org/freedesktop/UDisks2/drives/SanDisk_Ultra_123",
            "SanDisk Ultra",
            true,
        )
        .ok_or("no event")?;
        assert_eq!(input.id.as_str(), "storage:SanDisk_Ultra_123");
        assert!(input.validate().is_ok());
        let Payload::System(p) = &drive_event("/x/y", "Disk", false)
            .ok_or("no event")?
            .payload
        else {
            return Err("not system");
        };
        assert_eq!(p.detail.as_deref(), Some("USB drive removed"));
        Ok(())
    }
}
