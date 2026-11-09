//! Media players through MPRIS (`org.mpris.MediaPlayer2.*` on the session
//! bus): Spotify, Firefox, Chromium, VLC, mpv, Rhythmbox and others.
//!
//! One event per player, id `mpris:<player>`. Playing is persistent;
//! paused stays for `PAUSED_TTL_MS` after the last change; stopped or a
//! closed player ends the event. Previous / Play-Pause / Next buttons are
//! offered only when the player says it supports them.

use std::collections::HashMap;
use std::sync::Arc;

use futures_util::StreamExt;
use selawi_core::events::{
    Action, Category, EventId, EventInput, LifecycleState, MediaPayload, Payload, Source,
};
use tokio::task::JoinHandle;
use zbus::fdo::{DBusProxy, PropertiesProxy};
use zbus::names::InterfaceName;
use zbus::{proxy, Connection};

use super::variant::{self, Props};
use super::{id_part, Stop};
use crate::hub::{ActionRequest, Hub, Status};

pub const ID: &str = "mpris";
pub const NAME: &str = "Media players (MPRIS)";
const BUS_PREFIX: &str = "org.mpris.MediaPlayer2.";
const EVENT_PREFIX: &str = "mpris:";
const OBJECT_PATH: &str = "/org/mpris/MediaPlayer2";
const ROOT_IFACE: InterfaceName<'static> =
    InterfaceName::from_static_str_unchecked("org.mpris.MediaPlayer2");
const PLAYER_IFACE: InterfaceName<'static> =
    InterfaceName::from_static_str_unchecked("org.mpris.MediaPlayer2.Player");
/// A paused player disappears this long after its last change.
pub const PAUSED_TTL_MS: u64 = 5 * 60 * 1000;

#[proxy(
    interface = "org.mpris.MediaPlayer2.Player",
    default_path = "/org/mpris/MediaPlayer2"
)]
trait Player {
    fn play_pause(&self) -> zbus::Result<()>;
    fn next(&self) -> zbus::Result<()>;
    fn previous(&self) -> zbus::Result<()>;
    #[zbus(signal)]
    fn seeked(&self, position: i64) -> zbus::Result<()>;
}

pub fn event_id(bus_name: &str) -> String {
    format!(
        "{EVENT_PREFIX}{}",
        id_part(bus_name.strip_prefix(BUS_PREFIX).unwrap_or(bus_name))
    )
}

/// Builds the event for a player's current state, or `None` when there is
/// nothing to show (stopped, or no track information at all).
pub fn media_input(
    bus_name: &str,
    identity: Option<&str>,
    player: &Props,
    position_us: Option<i64>,
) -> Option<EventInput> {
    let playing = match variant::string(player, "PlaybackStatus").as_deref() {
        Some("Playing") => true,
        Some("Paused") => false,
        _ => return None,
    };
    let metadata: Props = player
        .get("Metadata")
        .and_then(|v| v.try_clone().ok())
        .and_then(|v| Props::try_from(v).ok())
        .unwrap_or_default();

    let app = identity.map(str::to_string).unwrap_or_else(|| {
        bus_name
            .strip_prefix(BUS_PREFIX)
            .unwrap_or(bus_name)
            .to_string()
    });
    let title = variant::string(&metadata, "xesam:title").unwrap_or_else(|| app.clone());
    let artists = variant::strings(&metadata, "xesam:artist");
    let duration_ms = variant::i64_(&metadata, "mpris:length")
        .filter(|&us| us > 0)
        .and_then(|us| u64::try_from(us / 1000).ok());
    let position_ms = position_us
        .filter(|&us| us >= 0)
        .and_then(|us| u64::try_from(us / 1000).ok())
        .map(|ms| duration_ms.map_or(ms, |d| ms.min(d)));

    let can = |key: &str| variant::boolean(player, key).unwrap_or(false);
    let can_control = variant::boolean(player, "CanControl").unwrap_or(true);
    let mut actions = Vec::new();
    if can_control && can("CanGoPrevious") {
        actions.push(Action {
            id: "previous".into(),
            label: "Previous".into(),
        });
    }
    if can_control && playing && can("CanPause") {
        actions.push(Action {
            id: "play_pause".into(),
            label: "Pause".into(),
        });
    } else if can_control && !playing && can("CanPlay") {
        actions.push(Action {
            id: "play_pause".into(),
            label: "Play".into(),
        });
    }
    if can_control && can("CanGoNext") {
        actions.push(Action {
            id: "next".into(),
            label: "Next".into(),
        });
    }

    let input = EventInput {
        id: EventId::new(event_id(bus_name)).ok()?,
        source: Source {
            app: truncate(&app, 64),
            integration: ID.into(),
        },
        category: Category::Media,
        state: LifecycleState::Updated,
        priority: None,
        persistent: playing,
        ttl_ms: (!playing).then_some(PAUSED_TTL_MS),
        group: None,
        payload: Payload::Media(MediaPayload {
            title: truncate(&title, 256),
            artist: (!artists.is_empty()).then(|| truncate(&artists.join(", "), 256)),
            album: variant::string(&metadata, "xesam:album").map(|a| truncate(&a, 256)),
            playing,
            position_ms,
            duration_ms,
        }),
        actions,
    };
    // Anything a misbehaving player sends that still fails validation is dropped.
    input.validate().ok().map(|()| input)
}

fn truncate(text: &str, max: usize) -> String {
    let cleaned: String = text
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect();
    cleaned.chars().take(max).collect()
}

pub async fn run(hub: Arc<Hub>) -> Result<(), Stop> {
    let conn = Connection::session().await?;
    watch(hub, conn).await
}

/// Watches all players on `conn` until the bus connection fails.
pub async fn watch(hub: Arc<Hub>, conn: Connection) -> Result<(), Stop> {
    let dbus = DBusProxy::new(&conn).await?;
    let mut owners = dbus.receive_name_owner_changed().await?;
    let mut actions = hub.register_actions(EVENT_PREFIX).await;
    let mut players: HashMap<String, JoinHandle<()>> = HashMap::new();

    for name in dbus.list_names().await? {
        let name = name.to_string();
        if name.starts_with(BUS_PREFIX) {
            players.insert(name.clone(), spawn_player(hub.clone(), conn.clone(), name));
        }
    }
    set_active(&hub, players.len()).await;

    loop {
        tokio::select! {
            signal = owners.next() => {
                let Some(signal) = signal else { break };
                let Ok(args) = signal.args() else { continue };
                let name = args.name().to_string();
                if !name.starts_with(BUS_PREFIX) {
                    continue;
                }
                if let Some(task) = players.remove(&name) {
                    task.abort();
                }
                if args.new_owner().is_some() {
                    players.insert(name.clone(), spawn_player(hub.clone(), conn.clone(), name));
                } else {
                    hub.finish(&event_id(&name)).await;
                }
                set_active(&hub, players.len()).await;
            }
            request = actions.recv() => {
                let Some(request) = request else { break };
                handle_action(&conn, &players, &request).await;
            }
        }
    }
    for task in players.into_values() {
        task.abort();
    }
    Err(Stop::Failed("lost the session bus".into()))
}

async fn set_active(hub: &Hub, count: usize) {
    let detail = match count {
        1 => "1 player".to_string(),
        n => format!("{n} players"),
    };
    hub.set_status(ID, NAME, Status::Active(detail)).await;
}

async fn handle_action(
    conn: &Connection,
    players: &HashMap<String, JoinHandle<()>>,
    request: &ActionRequest,
) {
    let Some(name) = players.keys().find(|n| event_id(n) == request.event_id) else {
        return;
    };
    let Ok(proxy) = PlayerProxy::builder(conn)
        .destination(name.as_str())
        .build()
        .await
    else {
        return;
    };
    let result = match request.action_id.as_str() {
        "play_pause" => proxy.play_pause().await,
        "next" => proxy.next().await,
        "previous" => proxy.previous().await,
        _ => return,
    };
    if let Err(e) = result {
        eprintln!("Media action {} failed: {e}", request.action_id);
    }
}

fn spawn_player(hub: Arc<Hub>, conn: Connection, name: String) -> JoinHandle<()> {
    tokio::spawn(async move {
        if let Err(e) = watch_player(&hub, &conn, &name).await {
            eprintln!("Stopped watching a media player: {e}");
        }
    })
}

/// Re-reads the player after every property change or seek and publishes it.
async fn watch_player(hub: &Hub, conn: &Connection, name: &str) -> zbus::Result<()> {
    let props = PropertiesProxy::builder(conn)
        .destination(name)
        .path(OBJECT_PATH)
        .build()
        .await?;
    let player = PlayerProxy::builder(conn).destination(name).build().await?;
    let mut changes = props.receive_properties_changed().await?;
    let mut seeks = player.receive_seeked().await?;
    let identity = props
        .get(ROOT_IFACE, "Identity")
        .await
        .ok()
        .and_then(|v| String::try_from(&*v).ok());
    let id = event_id(name);
    // (title, playing) last published, to tell a new track or a resume
    // (sent as `started`, which also brings back a dismissed event) from
    // a plain update.
    let mut last: Option<(String, bool)> = None;

    loop {
        let all = props.get_all(PLAYER_IFACE).await?;
        // Position is never announced in PropertiesChanged; ask for it.
        let position = props
            .get(PLAYER_IFACE, "Position")
            .await
            .ok()
            .and_then(|v| i64::try_from(&*v).ok());
        match media_input(name, identity.as_deref(), &all, position) {
            Some(mut input) => {
                if let Payload::Media(m) = &input.payload {
                    let now = (m.title.clone(), m.playing);
                    let fresh = match &last {
                        None => true,
                        Some((title, was_playing)) => *title != now.0 || (now.1 && !was_playing),
                    };
                    if fresh {
                        input.state = LifecycleState::Started;
                    }
                    last = Some(now);
                }
                if let Err(e) = hub.submit(input).await {
                    eprintln!("Media event rejected: {e}");
                }
            }
            None => {
                last = None;
                hub.finish(&id).await;
            }
        }
        tokio::select! {
            change = changes.next() => if change.is_none() { break },
            seek = seeks.next() => if seek.is_none() { break },
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use zbus::Variant;

    fn player(status: &str, metadata: Vec<(&str, Variant<'static>)>, caps: bool) -> Props {
        let meta: HashMap<String, Variant<'static>> = metadata
            .into_iter()
            .map(|(k, v)| (k.to_string(), v))
            .collect();
        fn put(p: &mut Props, k: &str, v: Variant<'_>) {
            if let Ok(o) = v.try_into_owned() {
                p.insert(k.to_string(), o);
            }
        }
        let mut p = Props::new();
        put(&mut p, "PlaybackStatus", Variant::from(status));
        put(&mut p, "Metadata", Variant::from(meta));
        for key in [
            "CanGoNext",
            "CanGoPrevious",
            "CanPlay",
            "CanPause",
            "CanControl",
        ] {
            put(&mut p, key, Variant::from(caps));
        }
        p
    }

    fn media(input: &EventInput) -> &MediaPayload {
        match &input.payload {
            Payload::Media(m) => m,
            other => panic!("not media: {other:?}"),
        }
    }

    #[test]
    fn playing_track_is_persistent_with_controls() {
        let p = player(
            "Playing",
            vec![
                ("xesam:title", Variant::from("Midnight City")),
                ("xesam:artist", Variant::from(vec!["M83"])),
                ("xesam:album", Variant::from("Hurry Up")),
                ("mpris:length", Variant::I64(243_000_000)),
            ],
            true,
        );
        let input = media_input(
            "org.mpris.MediaPlayer2.spotify",
            Some("Spotify"),
            &p,
            Some(61_500_000),
        );
        let Some(input) = input else {
            panic!("expected an event")
        };
        assert_eq!(input.id.as_str(), "mpris:spotify");
        assert!(input.persistent);
        assert_eq!(input.source.app, "Spotify");
        let m = media(&input);
        assert_eq!(m.title, "Midnight City");
        assert_eq!(m.artist.as_deref(), Some("M83"));
        assert_eq!(m.duration_ms, Some(243_000));
        assert_eq!(m.position_ms, Some(61_500));
        let ids: Vec<&str> = input.actions.iter().map(|a| a.id.as_str()).collect();
        assert_eq!(ids, ["previous", "play_pause", "next"]);
        assert_eq!(input.actions[1].label, "Pause");
    }

    #[test]
    fn paused_track_expires_and_offers_play() -> Result<(), &'static str> {
        let p = player("Paused", vec![("xesam:title", Variant::from("Song"))], true);
        let input = media_input("org.mpris.MediaPlayer2.vlc", None, &p, None).ok_or("no event")?;
        assert!(!input.persistent);
        assert_eq!(input.ttl_ms, Some(PAUSED_TTL_MS));
        assert_eq!(input.actions[1].label, "Play");
        assert_eq!(input.source.app, "vlc");
        Ok(())
    }

    #[test]
    fn stopped_or_unknown_status_shows_nothing() {
        let p = player(
            "Stopped",
            vec![("xesam:title", Variant::from("Song"))],
            true,
        );
        assert!(media_input("org.mpris.MediaPlayer2.x", None, &p, None).is_none());
        assert!(media_input("org.mpris.MediaPlayer2.x", None, &Props::new(), None).is_none());
    }

    #[test]
    fn no_controls_when_player_cannot() -> Result<(), &'static str> {
        let p = player("Playing", vec![], false);
        let input = media_input(
            "org.mpris.MediaPlayer2.firefox.instance_1_9",
            Some("Firefox"),
            &p,
            None,
        )
        .ok_or("no event")?;
        assert!(input.actions.is_empty());
        assert_eq!(
            media(&input).title,
            "Firefox",
            "falls back to the player name"
        );
        assert_eq!(
            media(&input).position_ms,
            None,
            "unknown position stays unknown"
        );
        Ok(())
    }

    #[test]
    fn hostile_metadata_is_cleaned() -> Result<(), &'static str> {
        let p = player(
            "Playing",
            vec![("xesam:title", Variant::from("a\u{7}b".repeat(400)))],
            true,
        );
        let input = media_input("org.mpris.MediaPlayer2.evil player!", None, &p, Some(-5))
            .ok_or("no event")?;
        assert!(input.validate().is_ok());
        assert_eq!(input.id.as_str(), "mpris:evil_player_");
        assert_eq!(media(&input).position_ms, None);
        Ok(())
    }
}
