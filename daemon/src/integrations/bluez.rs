//! Bluetooth devices through BlueZ (`org.bluez`, system bus).
//!
//! - A device connects → "Connected", with its battery if (and only if) the
//!   device reports one. Batteries often appear a moment after connecting;
//!   the event is updated if that happens within `BATTERY_GRACE`.
//! - A device disconnects → "Disconnected".
//! - A connected device's battery falls to `LOW_BATTERY` → one warning.
//!
//! Devices that are already connected when the daemon starts are not
//! announced.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use futures_util::StreamExt;
use selawi_core::events::{
    Category, DeviceKind, DevicePayload, EventId, EventInput, LifecycleState, Payload, Severity,
    Source, SystemPayload,
};
use zbus::fdo::{DBusProxy, ObjectManagerProxy};
use zbus::message::Type as MessageType;
use zbus::{Connection, MatchRule, MessageStream, OwnedVariant};

use super::variant::{self, Props};
use super::{id_part, Stop};
use crate::hub::{Hub, Status};

pub const ID: &str = "bluez";
pub const NAME: &str = "Bluetooth (BlueZ)";
const DESTINATION: &str = "org.bluez";
const DEVICE_IFACE: &str = "org.bluez.Device1";
const BATTERY_IFACE: &str = "org.bluez.Battery1";
pub const LOW_BATTERY: u8 = 15;
/// The low-battery warning re-arms once the battery is above this again.
const LOW_BATTERY_RESET: u8 = 20;
const BATTERY_GRACE: Duration = Duration::from_secs(10);

#[derive(Debug, Default)]
struct Tracked {
    device: Props,
    battery: Option<Props>,
    connected_at: Option<Instant>,
    low_warned: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Device {
    pub name: String,
    pub kind: Option<DeviceKind>,
    pub connected: bool,
    pub battery: Option<u8>,
}

pub fn device(device: &Props, battery: Option<&Props>) -> Device {
    let name = variant::string(device, "Alias")
        .or_else(|| variant::string(device, "Name"))
        .or_else(|| variant::string(device, "Address"))
        .unwrap_or_else(|| "Bluetooth device".into());
    Device {
        name: name.chars().filter(|c| !c.is_control()).take(256).collect(),
        kind: variant::string(device, "Icon")
            .as_deref()
            .map(kind_from_icon),
        connected: variant::boolean(device, "Connected").unwrap_or(false),
        battery: battery
            .and_then(|b| variant::u8_(b, "Percentage"))
            .filter(|&p| p <= 100),
    }
}

fn kind_from_icon(icon: &str) -> DeviceKind {
    match icon {
        "audio-headphones" | "audio-headset" => DeviceKind::Headphones,
        "audio-card" | "audio-speakers" => DeviceKind::Speaker,
        "input-keyboard" => DeviceKind::Keyboard,
        "input-mouse" | "input-tablet" => DeviceKind::Mouse,
        "phone" => DeviceKind::Phone,
        _ => DeviceKind::Other,
    }
}

pub fn event_id(path: &str) -> String {
    let tail = path.rsplit('/').next().unwrap_or(path);
    format!("bluez:{}", id_part(tail))
}

/// "Connected" / "Disconnected" event for a device.
pub fn connection_event(path: &str, d: &Device) -> Option<EventInput> {
    let input = EventInput {
        id: EventId::new(event_id(path)).ok()?,
        source: Source {
            app: "Bluetooth".into(),
            integration: ID.into(),
            icon: None,
        },
        category: Category::Device,
        state: LifecycleState::Started,
        priority: None,
        persistent: false,
        ttl_ms: Some(if d.connected { 5_000 } else { 4_000 }),
        group: None,
        payload: Payload::Device(DevicePayload {
            name: d.name.clone(),
            device_kind: d.kind,
            connected: d.connected,
            battery_percent: if d.connected { d.battery } else { None },
        }),
        actions: Vec::new(),
    };
    input.validate().ok().map(|()| input)
}

pub fn low_battery_event(path: &str, d: &Device) -> Option<EventInput> {
    let level = d.battery?;
    let input = EventInput {
        id: EventId::new(format!("{}:battery", event_id(path))).ok()?,
        source: Source {
            app: "Bluetooth".into(),
            integration: ID.into(),
            icon: None,
        },
        category: Category::Device,
        state: LifecycleState::Started,
        priority: None,
        persistent: false,
        ttl_ms: Some(10_000),
        group: None,
        payload: Payload::System(SystemPayload {
            title: format!("{} battery low", d.name)
                .chars()
                .take(256)
                .collect(),
            detail: Some(format!("{level}% remaining")),
            severity: Severity::Warning,
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

/// Watches BlueZ on `conn` (the system bus in production).
pub async fn watch(hub: Arc<Hub>, conn: Connection) -> Result<(), Stop> {
    let dbus = DBusProxy::new(&conn).await?;
    let mut owner = dbus.receive_name_owner_changed().await?;
    let manager = ObjectManagerProxy::builder(&conn)
        .destination(DESTINATION)
        .path("/")
        .build()
        .await?;
    let mut added = manager.receive_interfaces_added().await?;
    let mut removed = manager.receive_interfaces_removed().await?;
    let rule = MatchRule::builder()
        .msg_type(MessageType::Signal)
        .sender(DESTINATION)
        .interface("org.freedesktop.DBus.Properties")
        .member("PropertiesChanged")
        .path_namespace("/org/bluez")
        .build()?;
    // Room for the device chatter (e.g. signal strength while Bluetooth
    // settings scan) that arrives while all devices are read below: a full
    // queue would hold back that reply.
    let mut changed =
        MessageStream::for_match_rule(rule, &conn, std::num::NonZeroUsize::new(4096)).await?;

    let mut devices: HashMap<String, Tracked> = HashMap::new();
    for (path, interfaces) in manager.get_managed_objects().await? {
        let mut tracked = Tracked::default();
        for (iface, props) in interfaces {
            match iface.as_str() {
                DEVICE_IFACE => tracked.device = props,
                BATTERY_IFACE => tracked.battery = Some(props),
                _ => {}
            }
        }
        if !tracked.device.is_empty() {
            devices.insert(path.to_string(), tracked);
        }
    }
    set_active(&hub, &devices).await;

    loop {
        tokio::select! {
            signal = owner.next() => {
                let Some(signal) = signal else { break };
                if let Ok(args) = signal.args() {
                    if args.name().as_str() == DESTINATION && args.new_owner().is_none() {
                        return Err(Stop::Unavailable("bluetoothd stopped".into()));
                    }
                }
            }
            signal = added.next() => {
                let Some(signal) = signal else { break };
                let Ok(args) = signal.args() else { continue };
                let path = args.object_path().to_string();
                for (iface, props) in args.interfaces_and_properties() {
                    let props: Props = props
                        .iter()
                        .filter_map(|(k, v)| v.try_to_owned().ok().map(|o| (k.to_string(), o)))
                        .collect();
                    match iface.as_str() {
                        DEVICE_IFACE => {
                            let connected = variant::boolean(&props, "Connected").unwrap_or(false);
                            let tracked = devices.entry(path.clone()).or_default();
                            tracked.device = props;
                            if connected {
                                tracked.connected_at = Some(Instant::now());
                                publish(&hub, connection_event(&path, &snapshot(tracked))).await;
                            }
                        }
                        BATTERY_IFACE => {
                            if let Some(tracked) = devices.get_mut(&path) {
                                tracked.battery = Some(props);
                                battery_changed(&hub, &path, tracked).await;
                            }
                        }
                        _ => {}
                    }
                }
                set_active(&hub, &devices).await;
            }
            signal = removed.next() => {
                let Some(signal) = signal else { break };
                let Ok(args) = signal.args() else { continue };
                let path = args.object_path().to_string();
                for iface in args.interfaces().iter() {
                    match iface.as_str() {
                        DEVICE_IFACE => { devices.remove(&path); }
                        BATTERY_IFACE => {
                            if let Some(t) = devices.get_mut(&path) { t.battery = None; }
                        }
                        _ => {}
                    }
                }
                set_active(&hub, &devices).await;
            }
            message = changed.next() => {
                let Some(Ok(message)) = message else { break };
                let Some(path) = message.header().path().map(|p| p.to_string()) else { continue };
                let Ok((iface, props, _invalidated)) =
                    message.body().deserialize::<(String, HashMap<String, OwnedVariant>, Vec<String>)>()
                else { continue };
                let Some(tracked) = devices.get_mut(&path) else { continue };
                match iface.as_str() {
                    DEVICE_IFACE => {
                        let was = variant::boolean(&tracked.device, "Connected").unwrap_or(false);
                        tracked.device.extend(props);
                        let now = variant::boolean(&tracked.device, "Connected").unwrap_or(false);
                        if was != now {
                            tracked.connected_at = now.then(Instant::now);
                            tracked.low_warned = false;
                            publish(&hub, connection_event(&path, &snapshot(tracked))).await;
                        }
                    }
                    BATTERY_IFACE => {
                        tracked.battery.get_or_insert_with(Props::new).extend(props);
                        battery_changed(&hub, &path, tracked).await;
                    }
                    _ => {}
                }
            }
        }
    }
    Err(Stop::Failed("lost the system bus".into()))
}

fn snapshot(tracked: &Tracked) -> Device {
    device(&tracked.device, tracked.battery.as_ref())
}

async fn battery_changed(hub: &Hub, path: &str, tracked: &mut Tracked) {
    let d = snapshot(tracked);
    if !d.connected {
        return;
    }
    // Battery arrived just after connecting: add it to the "Connected" event.
    if tracked
        .connected_at
        .is_some_and(|t| t.elapsed() < BATTERY_GRACE)
    {
        publish(hub, connection_event(path, &d)).await;
    }
    match d.battery {
        Some(level) if level <= LOW_BATTERY && !tracked.low_warned => {
            tracked.low_warned = true;
            publish(hub, low_battery_event(path, &d)).await;
        }
        Some(level) if level >= LOW_BATTERY_RESET => tracked.low_warned = false,
        _ => {}
    }
}

async fn publish(hub: &Hub, input: Option<EventInput>) {
    if let Some(input) = input {
        if let Err(e) = hub.submit(input).await {
            eprintln!("Bluetooth event rejected: {e}");
        }
    }
}

async fn set_active(hub: &Hub, devices: &HashMap<String, Tracked>) {
    let connected = devices.values().filter(|t| snapshot(t).connected).count();
    hub.set_status(
        ID,
        NAME,
        Status::Active(format!("{} known, {connected} connected", devices.len())),
    )
    .await;
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
    fn reads_device_and_battery() {
        let dev = props(vec![
            ("Alias", Variant::from("Galaxy Buds2 Pro")),
            ("Icon", Variant::from("audio-headset")),
            ("Connected", Variant::from(true)),
        ]);
        let bat = props(vec![("Percentage", Variant::U8(82))]);
        let d = device(&dev, Some(&bat));
        assert_eq!(d.name, "Galaxy Buds2 Pro");
        assert_eq!(d.kind, Some(DeviceKind::Headphones));
        assert_eq!(d.battery, Some(82));
        assert_eq!(
            device(&dev, None).battery,
            None,
            "no battery interface, no battery"
        );
    }

    #[test]
    fn disconnect_never_shows_a_battery() -> Result<(), &'static str> {
        let d = Device {
            name: "Buds".into(),
            kind: None,
            connected: false,
            battery: Some(50),
        };
        let input = connection_event("/org/bluez/hci0/dev_AA_BB", &d).ok_or("no event")?;
        assert_eq!(input.id.as_str(), "bluez:dev_AA_BB");
        let Payload::Device(p) = &input.payload else {
            return Err("not a device payload");
        };
        assert_eq!(p.battery_percent, None);
        assert!(!p.connected);
        Ok(())
    }

    #[test]
    fn names_fall_back_and_are_cleaned() {
        let d = device(&props(vec![("Address", Variant::from("AA:BB"))]), None);
        assert_eq!(d.name, "AA:BB");
        assert_eq!(device(&Props::new(), None).name, "Bluetooth device");
        let d = device(&props(vec![("Alias", Variant::from("x\u{7}y"))]), None);
        assert_eq!(d.name, "xy");
    }

    #[test]
    fn low_battery_event_is_valid() -> Result<(), &'static str> {
        let d = Device {
            name: "Keyboard".into(),
            kind: Some(DeviceKind::Keyboard),
            connected: true,
            battery: Some(9),
        };
        let input = low_battery_event("/org/bluez/hci0/dev_1", &d).ok_or("no event")?;
        assert_eq!(input.id.as_str(), "bluez:dev_1:battery");
        assert!(input.validate().is_ok());
        Ok(())
    }
}
