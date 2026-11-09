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
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use zbus::fdo::{DBusProxy, PropertiesProxy};
use zbus::names::InterfaceName;
use zbus::{proxy, Connection};

use super::variant::{self, Props};
use super::{id_part, Stop};
use crate::config::{MediaConfig, MediaMode};
use crate::hub::{ActionRequest, Hub, MediaPlayer, Status};

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

/// Stable key for a player, used for the preferred-player setting:
/// "org.mpris.MediaPlayer2.firefox.instance_1_23" → "firefox".
pub fn player_key(bus_name: &str) -> String {
    let suffix = bus_name.strip_prefix(BUS_PREFIX).unwrap_or(bus_name);
    id_part(suffix.split(".instance").next().unwrap_or(suffix))
}

/// The latest state of one player, as reported by its watcher.
#[derive(Debug, Clone)]
pub struct PlayerState {
    pub key: String,
    pub name: String,
    /// What it would show, or None when stopped.
    pub input: Option<EventInput>,
    pub playing: bool,
    /// Ordering stamps (monotonic counter values).
    pub playing_since: u64,
    pub updated: u64,
}

/// Which players are shown. `players` maps bus name → state.
pub fn visible(players: &HashMap<String, PlayerState>, media: &MediaConfig) -> Vec<String> {
    let showable = players.iter().filter(|(_, p)| p.input.is_some());
    match media.mode {
        MediaMode::All => showable.map(|(name, _)| name.clone()).collect(),
        MediaMode::One => {
            let preferred = media.preferred_player.as_deref();
            let candidates: Vec<(&String, &PlayerState)> = showable.collect();
            let pick = candidates
                .iter()
                .find(|(_, p)| p.playing && Some(p.key.as_str()) == preferred)
                .or_else(|| {
                    candidates
                        .iter()
                        .filter(|(_, p)| p.playing)
                        .max_by_key(|(name, p)| (p.playing_since, (*name).clone()))
                })
                .or_else(|| {
                    candidates
                        .iter()
                        .find(|(_, p)| Some(p.key.as_str()) == preferred)
                })
                .or_else(|| {
                    candidates
                        .iter()
                        .max_by_key(|(name, p)| (p.updated, (*name).clone()))
                });
            pick.map(|(name, _)| (*name).clone()).into_iter().collect()
        }
    }
}

pub async fn run(hub: Arc<Hub>) -> Result<(), Stop> {
    let conn = Connection::session().await?;
    watch(hub, conn).await
}

/// One message from a player watcher to the coordinator.
enum Report {
    State(String, Option<Box<EventInput>>, bool),
    Gone(String),
}

struct Coordinator {
    hub: Arc<Hub>,
    players: HashMap<String, PlayerState>,
    /// What each shown player last submitted.
    shown: HashMap<String, EventInput>,
    clock: u64,
}

impl Coordinator {
    fn tick(&mut self) -> u64 {
        self.clock += 1;
        self.clock
    }

    fn update(&mut self, name: String, input: Option<EventInput>, playing: bool) {
        let now = self.tick();
        let key = player_key(&name);
        let state = self
            .players
            .entry(name.clone())
            .or_insert_with(|| PlayerState {
                key,
                name: name.clone(),
                input: None,
                playing: false,
                playing_since: 0,
                updated: 0,
            });
        if playing && !state.playing {
            state.playing_since = now;
        }
        state.playing = playing;
        state.input = input;
        state.updated = now;
    }

    /// Submits what should be visible and withdraws what should not.
    async fn reconcile(&mut self) {
        let config = self.hub.config();
        let visible = visible(&self.players, &config.media);
        let hidden: Vec<String> = self
            .shown
            .keys()
            .filter(|name| !visible.contains(name))
            .cloned()
            .collect();
        for name in hidden {
            if let Some(input) = self.shown.remove(&name) {
                let still_there = self.players.get(&name).is_some_and(|p| p.input.is_some());
                if still_there {
                    self.hub.withdraw(input.id.as_str()).await;
                } else {
                    self.hub.finish(input.id.as_str()).await;
                }
            }
        }
        for name in visible {
            let Some(input) = self.players.get(&name).and_then(|p| p.input.clone()) else {
                continue;
            };
            if self.shown.get(&name) == Some(&input) {
                continue;
            }
            if let Err(e) = self.hub.submit(input.clone()).await {
                eprintln!("Media event rejected: {e}");
            }
            self.shown.insert(name, input);
        }
        let mut list: Vec<MediaPlayer> = self
            .players
            .values()
            .map(|p| MediaPlayer {
                id: p.key.clone(),
                name: p
                    .input
                    .as_ref()
                    .map_or_else(|| p.key.clone(), |i| i.source.app.clone()),
            })
            .collect();
        list.sort_by(|a, b| a.id.cmp(&b.id));
        list.dedup_by(|a, b| a.id == b.id);
        self.hub.set_media_players(list).await;
    }
}

/// Watches all players on `conn` until the bus connection fails.
pub async fn watch(hub: Arc<Hub>, conn: Connection) -> Result<(), Stop> {
    let dbus = DBusProxy::new(&conn).await?;
    let mut owners = dbus.receive_name_owner_changed().await?;
    let mut actions = hub.register_actions(EVENT_PREFIX).await;
    let mut config = hub.watch_config();
    let (reports_tx, mut reports) = mpsc::unbounded_channel::<Report>();
    let mut tasks: HashMap<String, JoinHandle<()>> = HashMap::new();
    let mut coordinator = Coordinator {
        hub: hub.clone(),
        players: HashMap::new(),
        shown: HashMap::new(),
        clock: 0,
    };

    for name in dbus.list_names().await? {
        let name = name.to_string();
        if name.starts_with(BUS_PREFIX) {
            tasks.insert(
                name.clone(),
                spawn_player(conn.clone(), name, reports_tx.clone()),
            );
        }
    }
    set_active(&hub, tasks.len()).await;

    loop {
        tokio::select! {
            signal = owners.next() => {
                let Some(signal) = signal else { break };
                let Ok(args) = signal.args() else { continue };
                let name = args.name().to_string();
                if !name.starts_with(BUS_PREFIX) {
                    continue;
                }
                if let Some(task) = tasks.remove(&name) {
                    task.abort();
                }
                if args.new_owner().is_some() {
                    tasks.insert(name.clone(), spawn_player(conn.clone(), name, reports_tx.clone()));
                } else {
                    coordinator.players.remove(&name);
                    coordinator.reconcile().await;
                }
                set_active(&hub, tasks.len()).await;
            }
            report = reports.recv() => {
                match report {
                    Some(Report::State(name, input, playing)) => coordinator.update(name, input.map(|b| *b), playing),
                    Some(Report::Gone(name)) => { coordinator.players.remove(&name); }
                    None => break,
                }
                coordinator.reconcile().await;
            }
            changed = config.changed() => {
                if changed.is_err() { break }
                coordinator.reconcile().await;
            }
            request = actions.recv() => {
                let Some(request) = request else { break };
                handle_action(&conn, &tasks, &request).await;
            }
        }
    }
    for task in tasks.into_values() {
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

fn spawn_player(
    conn: Connection,
    name: String,
    reports: mpsc::UnboundedSender<Report>,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        if let Err(e) = watch_player(&conn, &name, &reports).await {
            eprintln!("Stopped watching a media player: {e}");
        }
        let _ = reports.send(Report::Gone(name));
    })
}

/// Re-reads the player after every property change or seek and reports it.
async fn watch_player(
    conn: &Connection,
    name: &str,
    reports: &mpsc::UnboundedSender<Report>,
) -> zbus::Result<()> {
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
    // (title, playing) last reported, to tell a new track or a resume
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
        let mut input = media_input(name, identity.as_deref(), &all, position);
        let mut playing = false;
        match input.as_mut() {
            Some(input) => {
                if let Payload::Media(m) = &input.payload {
                    playing = m.playing;
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
            }
            None => last = None,
        }
        if reports
            .send(Report::State(
                name.to_string(),
                input.map(Box::new),
                playing,
            ))
            .is_err()
        {
            break;
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

    fn state(key: &str, playing: bool, playing_since: u64, updated: u64) -> PlayerState {
        let p = player(if playing { "Playing" } else { "Paused" }, vec![], true);
        PlayerState {
            key: key.into(),
            name: key.into(),
            input: media_input(&format!("org.mpris.MediaPlayer2.{key}"), None, &p, None),
            playing,
            playing_since,
            updated,
        }
    }

    fn shown(players: Vec<PlayerState>, mode: MediaMode, preferred: Option<&str>) -> Vec<String> {
        let map: HashMap<String, PlayerState> =
            players.into_iter().map(|p| (p.key.clone(), p)).collect();
        let media = MediaConfig {
            mode,
            preferred_player: preferred.map(str::to_string),
        };
        let mut v = visible(&map, &media);
        v.sort();
        v
    }

    #[test]
    fn player_keys_are_stable_across_browser_instances() {
        assert_eq!(
            player_key("org.mpris.MediaPlayer2.firefox.instance_1_23"),
            "firefox"
        );
        assert_eq!(
            player_key("org.mpris.MediaPlayer2.chromium.instance4512"),
            "chromium"
        );
        assert_eq!(player_key("org.mpris.MediaPlayer2.spotify"), "spotify");
    }

    #[test]
    fn all_mode_shows_every_player() {
        let v = shown(
            vec![state("spotify", true, 1, 1), state("firefox", false, 0, 2)],
            MediaMode::All,
            None,
        );
        assert_eq!(v, ["firefox", "spotify"]);
    }

    #[test]
    fn one_mode_prefers_the_chosen_player_while_it_plays() {
        let players = || vec![state("spotify", true, 1, 1), state("firefox", true, 5, 5)];
        assert_eq!(
            shown(players(), MediaMode::One, Some("spotify")),
            ["spotify"]
        );
        assert_eq!(
            shown(players(), MediaMode::One, None),
            ["firefox"],
            "newest playing wins"
        );
    }

    #[test]
    fn one_mode_falls_back_when_preferred_is_paused() {
        let players = vec![state("spotify", false, 1, 9), state("firefox", true, 5, 5)];
        assert_eq!(shown(players, MediaMode::One, Some("spotify")), ["firefox"]);
        let all_paused = vec![state("spotify", false, 1, 3), state("firefox", false, 5, 5)];
        assert_eq!(
            shown(all_paused, MediaMode::One, Some("spotify")),
            ["spotify"]
        );
    }

    #[test]
    fn stopped_players_are_never_shown() {
        let mut stopped = state("vlc", true, 9, 9);
        stopped.input = None;
        assert!(shown(vec![stopped], MediaMode::One, Some("vlc")).is_empty());
    }
}
