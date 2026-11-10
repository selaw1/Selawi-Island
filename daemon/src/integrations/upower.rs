//! Battery through UPower's display device (system bus).
//!
//! Shows only changes, never the steady state:
//! - charger plugged in / unplugged, fully charged (short)
//! - battery low (UPower's own warning level, short)
//! - battery critical (persistent until the warning clears)
//!
//! Machines without a battery report "unavailable" and stay quiet.

use std::sync::Arc;

use futures_util::StreamExt;
use selawi_core::events::{
    Category, EventId, EventInput, LifecycleState, Payload, Severity, Source, SystemPayload,
};
use selawi_core::Priority;
use zbus::fdo::PropertiesProxy;
use zbus::names::InterfaceName;
use zbus::Connection;

use super::variant::{self, Props};
use super::Stop;
use crate::hub::{Hub, Status};

pub const ID: &str = "upower";
pub const NAME: &str = "Battery (UPower)";
const DESTINATION: &str = "org.freedesktop.UPower";
const DISPLAY_DEVICE: &str = "/org/freedesktop/UPower/devices/DisplayDevice";
const DEVICE_IFACE: InterfaceName<'static> =
    InterfaceName::from_static_str_unchecked("org.freedesktop.UPower.Device");
pub const POWER_EVENT: &str = "upower:power";
pub const WARNING_EVENT: &str = "upower:warning";

// UPower enums (see the UPower Device interface documentation).
const TYPE_BATTERY: u32 = 2;
const STATE_CHARGING: u32 = 1;
const STATE_FULLY_CHARGED: u32 = 4;
const STATE_PENDING_CHARGE: u32 = 5;
const WARNING_LOW: u32 = 3;
const WARNING_CRITICAL: u32 = 4;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Battery {
    pub percent: f64,
    pub state: u32,
    pub warning: u32,
}

impl Battery {
    fn on_power(&self) -> bool {
        matches!(
            self.state,
            STATE_CHARGING | STATE_FULLY_CHARGED | STATE_PENDING_CHARGE
        )
    }
}

/// `None` when the display device is not a present battery.
pub fn read(props: &Props) -> Option<Battery> {
    if variant::boolean(props, "IsPresent") != Some(true)
        || variant::u32_(props, "Type") != Some(TYPE_BATTERY)
    {
        return None;
    }
    Some(Battery {
        percent: variant::f64_(props, "Percentage")?.clamp(0.0, 100.0),
        state: variant::u32_(props, "State").unwrap_or(0),
        warning: variant::u32_(props, "WarningLevel").unwrap_or(0),
    })
}

#[derive(Debug, PartialEq)]
pub enum Output {
    Show(Box<EventInput>),
    End(&'static str),
}

/// What to show for a change from `previous` to `now`. With no previous
/// reading (daemon start) only warnings are shown, never charger events.
pub fn changes(previous: Option<Battery>, now: Battery) -> Vec<Output> {
    let mut out = Vec::new();
    let pct = format!("{:.0}%", now.percent);

    if let Some(prev) = previous {
        if prev.on_power() != now.on_power() {
            let title = if now.state == STATE_CHARGING {
                "Charging"
            } else if now.on_power() {
                "Plugged in"
            } else {
                "On battery"
            };
            out.push(Output::Show(Box::new(event(
                POWER_EVENT,
                title,
                &pct,
                Severity::Info,
                None,
                4_000,
            ))));
        } else if prev.state != STATE_FULLY_CHARGED && now.state == STATE_FULLY_CHARGED {
            out.push(Output::Show(Box::new(event(
                POWER_EVENT,
                "Fully charged",
                &pct,
                Severity::Info,
                None,
                6_000,
            ))));
        }
    }

    let was = previous.map_or(0, |p| p.warning);
    if now.warning >= WARNING_CRITICAL && !now.on_power() {
        // Persistent and refreshed as the level drops.
        let mut critical = event(
            WARNING_EVENT,
            "Battery critical",
            &format!("{pct} remaining — plug in now"),
            Severity::Critical,
            Some(Priority::Critical),
            0,
        );
        critical.persistent = true;
        critical.ttl_ms = None;
        out.push(Output::Show(Box::new(critical)));
    } else if now.warning == WARNING_LOW && was < WARNING_LOW && !now.on_power() {
        out.push(Output::Show(Box::new(event(
            WARNING_EVENT,
            "Battery low",
            &format!("{pct} remaining"),
            Severity::Warning,
            Some(Priority::Device),
            15_000,
        ))));
    } else if was >= WARNING_LOW && (now.warning < WARNING_LOW || now.on_power()) {
        out.push(Output::End(WARNING_EVENT));
    }
    out
}

fn event(
    id: &str,
    title: &str,
    detail: &str,
    severity: Severity,
    priority: Option<Priority>,
    ttl_ms: u64,
) -> EventInput {
    EventInput {
        id: EventId::new(id).unwrap_or_else(|_| unreachable!("constant ids are valid")),
        source: Source {
            app: "Power".into(),
            integration: ID.into(),
            icon: None,
        },
        category: Category::System,
        state: LifecycleState::Started,
        priority,
        persistent: false,
        ttl_ms: (ttl_ms > 0).then_some(ttl_ms),
        group: None,
        payload: Payload::System(SystemPayload {
            title: title.into(),
            detail: Some(detail.into()),
            severity,
        }),
        actions: Vec::new(),
    }
}

pub async fn run(hub: Arc<Hub>) -> Result<(), Stop> {
    let conn = Connection::system()
        .await
        .map_err(|_| Stop::Unavailable("system bus not available".into()))?;
    watch(hub, conn).await
}

/// Watches the display device on `conn` (the system bus in production).
pub async fn watch(hub: Arc<Hub>, conn: Connection) -> Result<(), Stop> {
    let props = PropertiesProxy::builder(&conn)
        .destination(DESTINATION)
        .path(DISPLAY_DEVICE)
        .build()
        .await?;
    let mut updates = props.receive_properties_changed().await?;
    let mut previous: Option<Battery> = None;

    loop {
        let all = props.get_all(DEVICE_IFACE).await?;
        match read(&all) {
            Some(now) => {
                hub.set_status(
                    ID,
                    NAME,
                    Status::Active(format!("battery at {:.0}%", now.percent)),
                )
                .await;
                if previous != Some(now) {
                    for output in changes(previous, now) {
                        match output {
                            Output::Show(input) => {
                                if let Err(e) = hub.submit(*input).await {
                                    eprintln!("Battery event rejected: {e}");
                                }
                            }
                            Output::End(id) => {
                                hub.finish(id).await;
                            }
                        }
                    }
                }
                previous = Some(now);
            }
            None => {
                hub.set_status(ID, NAME, Status::Unavailable("no battery".into()))
                    .await;
                previous = None;
            }
        }
        if updates.next().await.is_none() {
            return Err(Stop::Unavailable("UPower went away".into()));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DISCHARGING: u32 = 2;

    fn bat(percent: f64, state: u32, warning: u32) -> Battery {
        Battery {
            percent,
            state,
            warning,
        }
    }

    fn titles(out: &[Output]) -> Vec<String> {
        out.iter()
            .map(|o| match o {
                Output::Show(i) => match &i.payload {
                    Payload::System(p) => p.title.clone(),
                    _ => "?".into(),
                },
                Output::End(id) => format!("end {id}"),
            })
            .collect()
    }

    #[test]
    fn nothing_at_startup_unless_warning() {
        assert!(changes(None, bat(80.0, DISCHARGING, 1)).is_empty());
        assert!(changes(None, bat(80.0, STATE_CHARGING, 1)).is_empty());
        assert_eq!(
            titles(&changes(None, bat(9.0, DISCHARGING, WARNING_LOW))),
            ["Battery low"]
        );
    }

    #[test]
    fn plug_and_unplug() {
        let unplugged = bat(50.0, DISCHARGING, 1);
        let charging = bat(50.0, STATE_CHARGING, 1);
        assert_eq!(titles(&changes(Some(unplugged), charging)), ["Charging"]);
        assert_eq!(titles(&changes(Some(charging), unplugged)), ["On battery"]);
        assert_eq!(
            titles(&changes(Some(charging), bat(100.0, STATE_FULLY_CHARGED, 1))),
            ["Fully charged"]
        );
        assert!(
            changes(Some(charging), bat(51.0, STATE_CHARGING, 1)).is_empty(),
            "percent ticks are quiet"
        );
    }

    #[test]
    fn low_then_critical_then_charger_clears_it() {
        let low = bat(10.0, DISCHARGING, WARNING_LOW);
        let critical = bat(4.0, DISCHARGING, WARNING_CRITICAL);
        assert_eq!(
            titles(&changes(Some(bat(11.0, DISCHARGING, 1)), low)),
            ["Battery low"]
        );
        assert!(
            changes(Some(low), bat(9.0, DISCHARGING, WARNING_LOW)).is_empty(),
            "low shown once"
        );
        let out = changes(Some(low), critical);
        assert_eq!(titles(&out), ["Battery critical"]);
        let Output::Show(input) = &out[0] else {
            panic!("expected Show")
        };
        assert!(input.persistent);
        assert_eq!(input.priority, Some(Priority::Critical));
        assert!(input.validate().is_ok());
        assert_eq!(
            titles(&changes(
                Some(critical),
                bat(4.0, STATE_CHARGING, WARNING_CRITICAL)
            )),
            ["Charging", format!("end {WARNING_EVENT}").as_str()]
        );
    }

    #[test]
    fn read_requires_a_present_battery() {
        use zbus::Variant;
        let mut p = Props::new();
        for (k, v) in [
            ("IsPresent", Variant::from(true)),
            ("Type", Variant::from(TYPE_BATTERY)),
            ("Percentage", Variant::from(42.4)),
            ("State", Variant::from(DISCHARGING)),
            ("WarningLevel", Variant::from(1u32)),
        ] {
            if let Ok(o) = v.try_into_owned() {
                p.insert(k.into(), o);
            }
        }
        assert_eq!(read(&p), Some(bat(42.4, DISCHARGING, 1)));
        if let Ok(o) = Variant::from(false).try_into_owned() {
            p.insert("IsPresent".into(), o);
        }
        assert_eq!(read(&p), None);
    }
}
