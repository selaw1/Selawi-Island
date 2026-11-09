//! Network and VPN through NetworkManager (system bus).
//!
//! Shows changes only, never the steady state:
//! - the main connection changes → "<name> · Wi-Fi connected" (or Ethernet)
//! - no connection left → "Offline"
//! - a login page is required (captive portal) → "Sign in to network",
//!   until it is no longer needed
//! - a VPN connects or disconnects
//!
//! Nothing is announced at startup.

use std::collections::BTreeMap;
use std::sync::Arc;

use futures_util::StreamExt;
use selawi_core::events::{
    Category, EventId, EventInput, LifecycleState, Payload, Severity, Source, SystemPayload,
};
use zbus::fdo::PropertiesProxy;
use zbus::message::Type as MessageType;
use zbus::names::InterfaceName;
use zbus::{Connection, MatchRule, MessageStream, OwnedObjectPath};

use super::variant;
use super::{id_part, Stop};
use crate::hub::{Hub, Status};

pub const ID: &str = "network";
pub const NAME: &str = "Network (NetworkManager)";
const DESTINATION: &str = "org.freedesktop.NetworkManager";
const NM_PATH: &str = "/org/freedesktop/NetworkManager";
const ACTIVE_PREFIX: &str = "/org/freedesktop/NetworkManager/ActiveConnection/";
const NM_IFACE: InterfaceName<'static> =
    InterfaceName::from_static_str_unchecked("org.freedesktop.NetworkManager");
const ACTIVE_IFACE: InterfaceName<'static> =
    InterfaceName::from_static_str_unchecked("org.freedesktop.NetworkManager.Connection.Active");
pub const PRIMARY_EVENT: &str = "network:primary";
pub const PORTAL_EVENT: &str = "network:portal";

const ACTIVE_STATE_ACTIVATED: u32 = 2;
const CONNECTIVITY_PORTAL: u32 = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Wifi,
    Wired,
    Other,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Conn {
    pub name: String,
    pub kind: Kind,
}

/// What NetworkManager reports at one moment.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Snapshot {
    /// The main (non-VPN) connection, if any.
    pub primary: Option<Conn>,
    pub connectivity: u32,
    /// Activated VPNs: connection path → name.
    pub vpns: BTreeMap<String, String>,
}

pub fn kind_of(conn_type: &str) -> Kind {
    match conn_type {
        "802-11-wireless" => Kind::Wifi,
        "802-3-ethernet" => Kind::Wired,
        _ => Kind::Other,
    }
}

pub fn is_vpn(conn_type: &str, vpn_flag: bool) -> bool {
    vpn_flag || matches!(conn_type, "vpn" | "wireguard")
}

#[derive(Debug, PartialEq)]
pub enum Output {
    Show(Box<EventInput>),
    End(String),
}

/// What to show for a change from `previous` to `now`.
pub fn changes(previous: Option<&Snapshot>, now: &Snapshot) -> Vec<Output> {
    let Some(prev) = previous else {
        return Vec::new();
    };
    let mut out = Vec::new();

    if prev.primary != now.primary {
        match &now.primary {
            Some(conn) => {
                let detail = match conn.kind {
                    Kind::Wifi => "Wi-Fi connected",
                    Kind::Wired => "Ethernet connected",
                    Kind::Other => "Connected",
                };
                out.push(show(
                    PRIMARY_EVENT,
                    &conn.name,
                    detail,
                    Severity::Info,
                    4_000,
                ));
            }
            None => out.push(show(
                PRIMARY_EVENT,
                "Offline",
                "No network connection",
                Severity::Warning,
                6_000,
            )),
        }
    }

    let portal_now = now.connectivity == CONNECTIVITY_PORTAL;
    let portal_before = prev.connectivity == CONNECTIVITY_PORTAL;
    if portal_now && !portal_before {
        let name = now
            .primary
            .as_ref()
            .map_or("this network", |c| c.name.as_str());
        let mut input = event(
            PORTAL_EVENT,
            "Sign in to network",
            name,
            Severity::Warning,
            0,
        );
        input.persistent = true;
        input.ttl_ms = None;
        out.push(Output::Show(Box::new(input)));
    } else if portal_before && !portal_now {
        out.push(Output::End(PORTAL_EVENT.into()));
    }

    for (path, name) in &now.vpns {
        if !prev.vpns.contains_key(path) {
            out.push(show(
                &vpn_event_id(name),
                name,
                "VPN connected",
                Severity::Info,
                4_000,
            ));
        }
    }
    for (path, name) in &prev.vpns {
        if !now.vpns.contains_key(path) {
            out.push(show(
                &vpn_event_id(name),
                name,
                "VPN disconnected",
                Severity::Info,
                4_000,
            ));
        }
    }
    out
}

fn vpn_event_id(name: &str) -> String {
    format!("network:vpn:{}", id_part(name))
}

fn show(id: &str, title: &str, detail: &str, severity: Severity, ttl_ms: u64) -> Output {
    Output::Show(Box::new(event(id, title, detail, severity, ttl_ms)))
}

fn event(id: &str, title: &str, detail: &str, severity: Severity, ttl_ms: u64) -> EventInput {
    let clean = |s: &str| -> String { s.chars().filter(|c| !c.is_control()).take(256).collect() };
    let title = clean(title);
    EventInput {
        id: EventId::new(id).unwrap_or_else(|_| unreachable!("ids are built from id_part")),
        source: Source {
            app: "Network".into(),
            integration: ID.into(),
        },
        category: Category::System,
        state: LifecycleState::Started,
        priority: None,
        persistent: false,
        ttl_ms: (ttl_ms > 0).then_some(ttl_ms),
        group: None,
        payload: Payload::System(SystemPayload {
            title: if title.trim().is_empty() {
                "Network".into()
            } else {
                title
            },
            detail: Some(clean(detail)),
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

/// Watches NetworkManager on `conn` (the system bus in production).
pub async fn watch(hub: Arc<Hub>, conn: Connection) -> Result<(), Stop> {
    let nm = PropertiesProxy::builder(&conn)
        .destination(DESTINATION)
        .path(NM_PATH)
        .build()
        .await?;
    let rule = MatchRule::builder()
        .msg_type(MessageType::Signal)
        .sender(DESTINATION)
        .interface("org.freedesktop.DBus.Properties")
        .member("PropertiesChanged")
        .path_namespace(NM_PATH)
        .build()?;
    let mut signals = MessageStream::for_match_rule(rule, &conn, None).await?;
    let mut previous: Option<Snapshot> = None;

    loop {
        let now = snapshot(&conn, &nm).await?;
        let detail = match &now.primary {
            Some(c) => format!("on {}", c.name),
            None => "offline".into(),
        };
        hub.set_status(ID, NAME, Status::Active(detail)).await;
        for output in changes(previous.as_ref(), &now) {
            match output {
                Output::Show(input) => {
                    if let Err(e) = hub.submit(*input).await {
                        eprintln!("Network event rejected: {e}");
                    }
                }
                Output::End(id) => {
                    hub.finish(&id).await;
                }
            }
        }
        previous = Some(now);

        // Wait for a change on NetworkManager itself or an active
        // connection. Device and access-point chatter (signal strength…)
        // is ignored.
        loop {
            let Some(Ok(message)) = signals.next().await else {
                return Err(Stop::Unavailable("NetworkManager went away".into()));
            };
            let header = message.header();
            let path = header.path().map(|p| p.as_str()).unwrap_or_default();
            if path == NM_PATH || path.starts_with(ACTIVE_PREFIX) {
                break;
            }
        }
    }
}

async fn snapshot(conn: &Connection, nm: &PropertiesProxy<'_>) -> Result<Snapshot, Stop> {
    let props = nm.get_all(NM_IFACE).await?;
    let connectivity = variant::u32_(&props, "Connectivity").unwrap_or(0);
    let primary_path = props
        .get("PrimaryConnection")
        .and_then(|v| OwnedObjectPath::try_from(v.try_clone().ok()?).ok())
        .map(|p| p.to_string());
    let active: Vec<String> = props
        .get("ActiveConnections")
        .and_then(|v| v.try_clone().ok())
        .and_then(|v| Vec::<OwnedObjectPath>::try_from(v).ok())
        .unwrap_or_default()
        .into_iter()
        .map(|p| p.to_string())
        .collect();

    let mut vpns = BTreeMap::new();
    let mut primary: Option<Conn> = None;
    let mut fallback: Option<Conn> = None;
    for path in active {
        let Ok(proxy) = PropertiesProxy::builder(conn)
            .destination(DESTINATION)
            .path(path.as_str())
            .build()
            .await
        else {
            continue;
        };
        // The connection may disappear between the two calls.
        let Ok(a) = proxy.get_all(ACTIVE_IFACE).await else {
            continue;
        };
        let name = variant::string(&a, "Id").unwrap_or_else(|| "Network".into());
        let conn_type = variant::string(&a, "Type").unwrap_or_default();
        let activated = variant::u32_(&a, "State") == Some(ACTIVE_STATE_ACTIVATED);
        if !activated {
            continue;
        }
        if is_vpn(&conn_type, variant::boolean(&a, "Vpn").unwrap_or(false)) {
            vpns.insert(path, name);
            continue;
        }
        let kind = kind_of(&conn_type);
        if kind == Kind::Other {
            // Bridges, loopback, tunnels: never the "main" connection to show.
            continue;
        }
        let c = Conn { name, kind };
        if primary_path.as_deref() == Some(path.as_str()) {
            primary = Some(c);
        } else if fallback.is_none() {
            fallback = Some(c);
        }
    }
    // When a VPN is the primary connection, the underlying network is
    // still the one to show.
    Ok(Snapshot {
        primary: primary.or(fallback),
        connectivity,
        vpns,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wifi(name: &str) -> Option<Conn> {
        Some(Conn {
            name: name.into(),
            kind: Kind::Wifi,
        })
    }

    fn snap(primary: Option<Conn>, connectivity: u32, vpns: &[(&str, &str)]) -> Snapshot {
        Snapshot {
            primary,
            connectivity,
            vpns: vpns
                .iter()
                .map(|(p, n)| (p.to_string(), n.to_string()))
                .collect(),
        }
    }

    fn describe(out: &[Output]) -> Vec<String> {
        out.iter()
            .map(|o| match o {
                Output::Show(i) => match &i.payload {
                    Payload::System(p) => {
                        format!("{} · {}", p.title, p.detail.clone().unwrap_or_default())
                    }
                    _ => "?".into(),
                },
                Output::End(id) => format!("end {id}"),
            })
            .collect()
    }

    #[test]
    fn quiet_at_startup() {
        assert!(changes(None, &snap(wifi("Home"), 4, &[("/a/1", "Work VPN")])).is_empty());
    }

    #[test]
    fn switching_and_losing_networks() {
        let home = snap(wifi("Home"), 4, &[]);
        let office = snap(
            Some(Conn {
                name: "Office LAN".into(),
                kind: Kind::Wired,
            }),
            4,
            &[],
        );
        let offline = snap(None, 1, &[]);
        assert_eq!(
            describe(&changes(Some(&home), &office)),
            ["Office LAN · Ethernet connected"]
        );
        assert_eq!(
            describe(&changes(Some(&office), &offline)),
            ["Offline · No network connection"]
        );
        assert_eq!(
            describe(&changes(Some(&offline), &home)),
            ["Home · Wi-Fi connected"]
        );
        assert!(changes(Some(&home), &home).is_empty());
    }

    #[test]
    fn captive_portal_until_signed_in() {
        let cafe = snap(wifi("Cafe"), 4, &[]);
        let portal = snap(wifi("Cafe"), 2, &[]);
        let out = changes(Some(&cafe), &portal);
        assert_eq!(describe(&out), ["Sign in to network · Cafe"]);
        let Output::Show(input) = &out[0] else {
            panic!("expected Show")
        };
        assert!(input.persistent);
        assert!(input.validate().is_ok());
        assert_eq!(
            describe(&changes(Some(&portal), &cafe)),
            [format!("end {PORTAL_EVENT}")]
        );
    }

    #[test]
    fn vpn_up_and_down() {
        let plain = snap(wifi("Home"), 4, &[]);
        let vpn = snap(wifi("Home"), 4, &[("/a/7", "Work VPN")]);
        let out = changes(Some(&plain), &vpn);
        assert_eq!(describe(&out), ["Work VPN · VPN connected"]);
        let Output::Show(input) = &out[0] else {
            panic!("expected Show")
        };
        assert_eq!(input.id.as_str(), "network:vpn:Work_VPN");
        assert_eq!(
            describe(&changes(Some(&vpn), &plain)),
            ["Work VPN · VPN disconnected"]
        );
    }

    #[test]
    fn connection_types() {
        assert_eq!(kind_of("802-11-wireless"), Kind::Wifi);
        assert_eq!(kind_of("802-3-ethernet"), Kind::Wired);
        assert_eq!(kind_of("bridge"), Kind::Other);
        assert!(is_vpn("wireguard", false));
        assert!(is_vpn("vpn", false));
        assert!(is_vpn("tun", true));
        assert!(!is_vpn("802-11-wireless", false));
    }
}
