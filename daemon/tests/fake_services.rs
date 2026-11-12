//! Runs each built-in integration against a fake service on a private
//! session bus. Needs a bus: run with
//!   dbus-run-session -- cargo test -p selawi-islandd --test fake_services
//! Without one (plain `cargo test`) the tests print a note and pass.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use selawi_core::events::Payload;
use selawi_core::{Event, Timestamp};
use selawi_islandd::engine::Engine;
use selawi_islandd::hub::Hub;
use selawi_islandd::integrations::{bluez, mpris, upower};
use zbus::object_server::SignalEmitter;
use zbus::{interface, Connection, OwnedVariant, Variant};

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn have_bus() -> bool {
    if std::env::var_os("DBUS_SESSION_BUS_ADDRESS").is_none() {
        eprintln!("No D-Bus session bus: skipped (run under dbus-run-session)");
        return false;
    }
    true
}

fn new_hub() -> Arc<Hub> {
    Arc::new(Hub::new(Engine::new(None, Timestamp::now())))
}

async fn events(hub: &Hub) -> Vec<Event> {
    serde_json::from_str(&hub.list_json().await).unwrap_or_default()
}

/// Waits (up to 3 s) until `check` finds what it wants in the event list.
async fn wait_for<T>(
    hub: &Hub,
    what: &str,
    check: impl Fn(&[Event]) -> Option<T>,
) -> Result<T, String> {
    for _ in 0..60 {
        if let Some(found) = check(&events(hub).await) {
            return Ok(found);
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    Err(format!(
        "timed out waiting for {what}; events: {}",
        hub.list_json().await
    ))
}

fn find<'a>(events: &'a [Event], id: &str) -> Option<&'a Event> {
    events.iter().find(|e| e.id.as_str() == id)
}

fn owned(v: Variant<'_>) -> OwnedVariant {
    v.try_into_owned()
        .unwrap_or_else(|_| panic!("test value converts"))
}

// ---- MPRIS ---------------------------------------------------------------

struct FakeRoot;

#[interface(name = "org.mpris.MediaPlayer2")]
impl FakeRoot {
    #[zbus(property)]
    fn identity(&self) -> String {
        "Fake Player".into()
    }
    #[zbus(property)]
    fn desktop_entry(&self) -> String {
        "org.example.FakePlayer".into()
    }
}

struct FakePlayer {
    playing: bool,
}

#[interface(name = "org.mpris.MediaPlayer2.Player")]
impl FakePlayer {
    async fn play_pause(&mut self, #[zbus(signal_emitter)] emitter: SignalEmitter<'_>) {
        self.playing = !self.playing;
        let _ = self.playback_status_changed(&emitter).await;
    }
    fn next(&self) {}
    fn previous(&self) {}

    #[zbus(property)]
    fn playback_status(&self) -> String {
        if self.playing { "Playing" } else { "Paused" }.into()
    }
    #[zbus(property)]
    fn metadata(&self) -> HashMap<String, OwnedVariant> {
        HashMap::from([
            (
                "xesam:title".to_string(),
                owned(Variant::from("Midnight City")),
            ),
            (
                "xesam:artist".to_string(),
                owned(Variant::from(vec!["M83"])),
            ),
            ("mpris:length".to_string(), owned(Variant::I64(243_000_000))),
        ])
    }
    #[zbus(property)]
    fn position(&self) -> i64 {
        30_000_000
    }
    #[zbus(property)]
    fn can_go_next(&self) -> bool {
        true
    }
    #[zbus(property)]
    fn can_go_previous(&self) -> bool {
        true
    }
    #[zbus(property)]
    fn can_play(&self) -> bool {
        true
    }
    #[zbus(property)]
    fn can_pause(&self) -> bool {
        true
    }
    #[zbus(property)]
    fn can_control(&self) -> bool {
        true
    }
}

fn media(event: &Event) -> Option<(String, bool, Option<u64>)> {
    match &event.payload {
        Payload::Media(m) => Some((m.title.clone(), m.playing, m.position_ms)),
        _ => None,
    }
}

#[tokio::test]
async fn mpris_follows_a_player_and_controls_it() -> TestResult {
    if !have_bus() {
        return Ok(());
    }
    let hub = new_hub();
    let watcher = tokio::spawn(mpris::watch(hub.clone(), Connection::session().await?));
    tokio::time::sleep(Duration::from_millis(200)).await;

    // A player appears after the integration started.
    let player = zbus::connection::Builder::session()
        .serve_at("/org/mpris/MediaPlayer2", FakeRoot)
        .serve_at("/org/mpris/MediaPlayer2", FakePlayer { playing: true })
        .name("org.mpris.MediaPlayer2.fakeplayer")
        .build()
        .await?;

    let id = "mpris:fakeplayer";
    let (title, playing, position) =
        wait_for(&hub, "playing track", |ev| find(ev, id).and_then(media)).await?;
    assert_eq!(title, "Midnight City");
    assert!(playing);
    assert_eq!(position, Some(30_000));
    assert_eq!(
        find(&events(&hub).await, id)
            .map(|e| e.source.app.clone())
            .as_deref(),
        Some("Fake Player")
    );
    assert_eq!(
        find(&events(&hub).await, id)
            .and_then(|e| e.source.icon.clone())
            .as_deref(),
        Some("app:org.example.FakePlayer"),
        "the player's installed app gives the logo"
    );

    // Pressing Pause in the island reaches the player.
    assert!(hub.route_action(id, "play_pause").await);
    wait_for(&hub, "paused", |ev| {
        find(ev, id).and_then(media).filter(|m| !m.1)
    })
    .await?;

    // Closing the player ends the event.
    drop(player);
    wait_for(&hub, "ended", |ev| {
        find(ev, id).filter(|e| e.state.is_terminal()).map(|_| ())
    })
    .await?;
    watcher.abort();
    Ok(())
}

// ---- UPower --------------------------------------------------------------

struct FakeBattery {
    state: u32,
    warning: u32,
}

#[interface(name = "org.freedesktop.UPower.Device")]
impl FakeBattery {
    #[zbus(property)]
    fn is_present(&self) -> bool {
        true
    }
    #[zbus(property, name = "Type")]
    fn kind(&self) -> u32 {
        2
    }
    #[zbus(property)]
    fn percentage(&self) -> f64 {
        12.0
    }
    #[zbus(property)]
    fn state(&self) -> u32 {
        self.state
    }
    #[zbus(property)]
    fn warning_level(&self) -> u32 {
        self.warning
    }
}

fn system_title(event: &Event) -> Option<String> {
    match &event.payload {
        Payload::System(p) => Some(p.title.clone()),
        _ => None,
    }
}

#[tokio::test]
async fn upower_reports_charger_and_low_battery() -> TestResult {
    if !have_bus() {
        return Ok(());
    }
    const PATH: &str = "/org/freedesktop/UPower/devices/DisplayDevice";
    let service = zbus::connection::Builder::session()
        .serve_at(
            PATH,
            FakeBattery {
                state: 2,
                warning: 1,
            },
        )
        .name("org.freedesktop.UPower")
        .build()
        .await?;
    let hub = new_hub();
    let watcher = tokio::spawn(upower::watch(hub.clone(), Connection::session().await?));
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert!(events(&hub).await.is_empty(), "nothing at startup");
    assert!(hub.integrations_json().await.contains("battery at 12%"));

    let iface = service
        .object_server()
        .interface::<_, FakeBattery>(PATH)
        .await?;
    iface.get_mut().await.state = 1;
    iface
        .get()
        .await
        .state_changed(iface.signal_emitter())
        .await?;
    let title = wait_for(&hub, "charging", |ev| {
        find(ev, upower::POWER_EVENT).and_then(system_title)
    })
    .await?;
    assert_eq!(title, "Charging");

    {
        let mut battery = iface.get_mut().await;
        battery.state = 2;
        battery.warning = 3;
    }
    iface
        .get()
        .await
        .warning_level_changed(iface.signal_emitter())
        .await?;
    let title = wait_for(&hub, "low battery", |ev| {
        find(ev, upower::WARNING_EVENT).and_then(system_title)
    })
    .await?;
    assert_eq!(title, "Battery low");
    watcher.abort();
    Ok(())
}

// ---- BlueZ ---------------------------------------------------------------

struct FakeDevice {
    connected: bool,
}

#[interface(name = "org.bluez.Device1")]
impl FakeDevice {
    #[zbus(property)]
    fn alias(&self) -> String {
        "Galaxy Buds2 Pro".into()
    }
    #[zbus(property)]
    fn icon(&self) -> String {
        "audio-headset".into()
    }
    #[zbus(property)]
    fn connected(&self) -> bool {
        self.connected
    }
}

struct FakeBluezBattery;

#[interface(name = "org.bluez.Battery1")]
impl FakeBluezBattery {
    #[zbus(property)]
    fn percentage(&self) -> u8 {
        82
    }
}

fn device(event: &Event) -> Option<(bool, Option<u8>)> {
    match &event.payload {
        Payload::Device(d) => Some((d.connected, d.battery_percent)),
        _ => None,
    }
}

#[tokio::test]
async fn bluez_reports_connect_battery_and_disconnect() -> TestResult {
    if !have_bus() {
        return Ok(());
    }
    const DEV: &str = "/org/bluez/hci0/dev_AA_BB_CC";
    let service = zbus::connection::Builder::session()
        .serve_at(DEV, FakeDevice { connected: false })
        .serve_at("/", zbus::fdo::ObjectManager)
        .name("org.bluez")
        .build()
        .await?;
    let hub = new_hub();
    let watcher = tokio::spawn(bluez::watch(hub.clone(), Connection::session().await?));
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert!(hub
        .integrations_json()
        .await
        .contains("1 known, 0 connected"));

    let id = "bluez:dev_AA_BB_CC";
    let iface = service
        .object_server()
        .interface::<_, FakeDevice>(DEV)
        .await?;
    iface.get_mut().await.connected = true;
    iface
        .get()
        .await
        .connected_changed(iface.signal_emitter())
        .await?;
    let (connected, battery) =
        wait_for(&hub, "connected", |ev| find(ev, id).and_then(device)).await?;
    assert!(connected);
    assert_eq!(battery, None, "no battery reported yet");

    // The battery shows up a moment later and is added to the event.
    service.object_server().at(DEV, FakeBluezBattery).await?;
    wait_for(&hub, "battery", |ev| {
        find(ev, id).and_then(device).filter(|d| d.1 == Some(82))
    })
    .await?;

    iface.get_mut().await.connected = false;
    iface
        .get()
        .await
        .connected_changed(iface.signal_emitter())
        .await?;
    let (connected, battery) = wait_for(&hub, "disconnected", |ev| {
        find(ev, id).and_then(device).filter(|d| !d.0)
    })
    .await?;
    assert!(!connected);
    assert_eq!(battery, None, "a disconnected device shows no battery");
    watcher.abort();
    Ok(())
}

#[tokio::test]
async fn dismissed_media_comes_back_on_resume() -> TestResult {
    if !have_bus() {
        return Ok(());
    }
    let hub = new_hub();
    let player = zbus::connection::Builder::session()
        .serve_at("/org/mpris/MediaPlayer2", FakeRoot)
        .serve_at("/org/mpris/MediaPlayer2", FakePlayer { playing: true })
        .name("org.mpris.MediaPlayer2.resumetest")
        .build()
        .await?;
    let watcher = tokio::spawn(mpris::watch(hub.clone(), Connection::session().await?));
    let id = "mpris:resumetest";
    wait_for(&hub, "playing", |ev| find(ev, id).map(|_| ())).await?;
    assert!(hub.dismiss(id).await);

    // Pause: still dismissed. Play again: back.
    hub.route_action(id, "play_pause").await;
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert!(
        find(&events(&hub).await, id).is_none(),
        "pausing does not bring it back"
    );
    // The event is gone, so press play on the player directly.
    let iface = player
        .object_server()
        .interface::<_, FakePlayer>("/org/mpris/MediaPlayer2")
        .await?;
    iface.get_mut().await.playing = true;
    iface
        .get()
        .await
        .playback_status_changed(iface.signal_emitter())
        .await?;
    wait_for(&hub, "back after resume", |ev| {
        find(ev, id).and_then(media).filter(|m| m.1)
    })
    .await?;
    watcher.abort();
    Ok(())
}

// ---- Notifications --------------------------------------------------------

struct FakeNotificationServer;

#[interface(name = "org.freedesktop.Notifications")]
impl FakeNotificationServer {
    #[allow(clippy::too_many_arguments)]
    fn notify(
        &self,
        _app_name: &str,
        _replaces_id: u32,
        _app_icon: &str,
        _summary: &str,
        _body: &str,
        _actions: Vec<String>,
        _hints: HashMap<String, OwnedVariant>,
        _expire_timeout: i32,
    ) -> u32 {
        1
    }
}

#[zbus::proxy(
    interface = "org.freedesktop.Notifications",
    default_service = "org.freedesktop.Notifications",
    default_path = "/org/freedesktop/Notifications"
)]
trait Notifications {
    #[allow(clippy::too_many_arguments)]
    fn notify(
        &self,
        app_name: &str,
        replaces_id: u32,
        app_icon: &str,
        summary: &str,
        body: &str,
        actions: &[&str],
        hints: HashMap<&str, Variant<'_>>,
        expire_timeout: i32,
    ) -> zbus::Result<u32>;
}

#[tokio::test]
async fn notifications_are_observed_without_replacing_the_server() -> TestResult {
    if !have_bus() {
        return Ok(());
    }
    let _server = zbus::connection::Builder::session()
        .serve_at("/org/freedesktop/Notifications", FakeNotificationServer)
        .name("org.freedesktop.Notifications")
        .build()
        .await?;
    let hub = new_hub();
    let monitor = zbus::connection::Builder::session().build().await?;
    let watcher = tokio::spawn(selawi_islandd::integrations::notifications::watch(
        hub.clone(),
        monitor,
    ));
    tokio::time::sleep(Duration::from_millis(300)).await;

    let client = Connection::session().await?;
    let proxy = NotificationsProxy::new(&client).await?;
    let id = proxy
        .notify(
            "Slack",
            0,
            "",
            "Alex",
            "<b>Lunch</b> &amp; coffee?",
            &[],
            HashMap::new(),
            -1,
        )
        .await?;
    assert_eq!(id, 1, "the real server still answers");

    let (title, body) = wait_for(&hub, "notification", |ev| {
        ev.iter().find_map(|e| match &e.payload {
            Payload::Notification(p) if e.id.as_str().starts_with("notifications:slack:") => {
                Some((p.title.clone(), p.body.clone()))
            }
            _ => None,
        })
    })
    .await?;
    assert_eq!(title, "Alex");
    assert_eq!(body.as_deref(), Some("Lunch & coffee?"));

    watcher.abort();
    Ok(())
}

// ---- Media mode and live configuration ------------------------------------

async fn fake_player(suffix: &str, playing: bool) -> zbus::Result<Connection> {
    zbus::connection::Builder::session()
        .serve_at("/org/mpris/MediaPlayer2", FakeRoot)
        .serve_at("/org/mpris/MediaPlayer2", FakePlayer { playing })
        .name(format!("org.mpris.MediaPlayer2.{suffix}"))
        .build()
        .await
}

fn media_ids(events: &[Event]) -> Vec<String> {
    let mut ids: Vec<String> = events
        .iter()
        .filter(|e| e.id.as_str().starts_with("mpris:modetest") && !e.state.is_terminal())
        .map(|e| e.id.to_string())
        .collect();
    ids.sort();
    ids
}

#[tokio::test]
async fn one_player_mode_follows_the_preferred_player() -> TestResult {
    if !have_bus() {
        return Ok(());
    }
    let _a = fake_player("modetesta", true).await?;
    let _b = fake_player("modetestb", true).await?;
    let hub = new_hub();
    let watcher = tokio::spawn(mpris::watch(hub.clone(), Connection::session().await?));

    wait_for(&hub, "both players", |ev| {
        (media_ids(ev).len() == 2).then_some(())
    })
    .await?;
    let players = hub.media_players_json().await;
    assert!(players.contains("\"id\":\"modetesta\"") && players.contains("\"id\":\"modetestb\""));

    hub.set_config(r#"{"media":{"mode":"one","preferred_player":"modetesta"}}"#)?;
    wait_for(&hub, "only the preferred player", |ev| {
        (media_ids(ev) == ["mpris:modetesta"]).then_some(())
    })
    .await?;

    hub.set_config(r#"{"media":{"mode":"one","preferred_player":"modetestb"}}"#)?;
    wait_for(&hub, "switched to the other player", |ev| {
        (media_ids(ev) == ["mpris:modetestb"]).then_some(())
    })
    .await?;

    hub.set_config(r#"{"media":{"mode":"all"}}"#)?;
    wait_for(&hub, "both again", |ev| {
        (media_ids(ev).len() == 2).then_some(())
    })
    .await?;
    watcher.abort();
    Ok(())
}

#[tokio::test]
async fn turning_an_integration_off_removes_its_events_live() -> TestResult {
    if !have_bus() {
        return Ok(());
    }
    let _player = fake_player("livetoggle", true).await?;
    let hub = new_hub();
    selawi_islandd::integrations::spawn(hub.clone(), mpris::ID, mpris::NAME, mpris::run);
    let id = "mpris:livetoggle";
    wait_for(&hub, "player shown", |ev| find(ev, id).map(|_| ())).await?;

    hub.set_config(r#"{"integrations":{"mpris":false}}"#)?;
    wait_for(&hub, "removed", |ev| find(ev, id).is_none().then_some(())).await?;
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert!(hub
        .integrations_json()
        .await
        .contains("\"status\":\"disabled\""));

    hub.set_config("{}")?;
    wait_for(&hub, "shown again", |ev| find(ev, id).map(|_| ())).await?;
    Ok(())
}

// ---- NetworkManager -------------------------------------------------------

use zbus::OwnedObjectPath;

struct FakeNm {
    primary: String,
    active: Vec<String>,
    connectivity: u32,
}

#[interface(name = "org.freedesktop.NetworkManager")]
impl FakeNm {
    #[zbus(property)]
    fn state(&self) -> u32 {
        70
    }
    #[zbus(property)]
    fn connectivity(&self) -> u32 {
        self.connectivity
    }
    #[zbus(property)]
    fn primary_connection(&self) -> OwnedObjectPath {
        OwnedObjectPath::try_from(self.primary.clone()).unwrap_or_else(|_| panic!("valid path"))
    }
    #[zbus(property)]
    fn active_connections(&self) -> Vec<OwnedObjectPath> {
        self.active
            .iter()
            .filter_map(|p| OwnedObjectPath::try_from(p.clone()).ok())
            .collect()
    }
}

struct FakeActive {
    id: &'static str,
    kind: &'static str,
    vpn: bool,
}

#[interface(name = "org.freedesktop.NetworkManager.Connection.Active")]
impl FakeActive {
    #[zbus(property)]
    fn id(&self) -> String {
        self.id.into()
    }
    #[zbus(property, name = "Type")]
    fn kind(&self) -> String {
        self.kind.into()
    }
    #[zbus(property)]
    fn vpn(&self) -> bool {
        self.vpn
    }
    #[zbus(property)]
    fn state(&self) -> u32 {
        2
    }
}

fn system_line(event: &Event) -> Option<String> {
    match &event.payload {
        Payload::System(p) => Some(format!(
            "{} · {}",
            p.title,
            p.detail.clone().unwrap_or_default()
        )),
        _ => None,
    }
}

#[tokio::test]
async fn network_reports_vpn_switches_portal_and_offline() -> TestResult {
    if !have_bus() {
        return Ok(());
    }
    const NM: &str = "/org/freedesktop/NetworkManager";
    const WIFI: &str = "/org/freedesktop/NetworkManager/ActiveConnection/1";
    const VPN: &str = "/org/freedesktop/NetworkManager/ActiveConnection/2";
    const LAN: &str = "/org/freedesktop/NetworkManager/ActiveConnection/3";
    let service = zbus::connection::Builder::session()
        .serve_at(
            NM,
            FakeNm {
                primary: WIFI.into(),
                active: vec![WIFI.into()],
                connectivity: 4,
            },
        )
        .serve_at(
            WIFI,
            FakeActive {
                id: "Home",
                kind: "802-11-wireless",
                vpn: false,
            },
        )
        .name("org.freedesktop.NetworkManager")
        .build()
        .await?;
    let hub = new_hub();
    let watcher = tokio::spawn(selawi_islandd::integrations::network::watch(
        hub.clone(),
        Connection::session().await?,
    ));
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert!(events(&hub).await.is_empty(), "nothing at startup");
    assert!(hub.integrations_json().await.contains("on Home"));

    let server = service.object_server();
    let nm = server.interface::<_, FakeNm>(NM).await?;

    // A VPN comes up.
    server
        .at(
            VPN,
            FakeActive {
                id: "Work VPN",
                kind: "vpn",
                vpn: true,
            },
        )
        .await?;
    nm.get_mut().await.active = vec![WIFI.into(), VPN.into()];
    nm.get()
        .await
        .active_connections_changed(nm.signal_emitter())
        .await?;
    let line = wait_for(&hub, "vpn up", |ev| {
        find(ev, "network:vpn:Work_VPN").and_then(system_line)
    })
    .await?;
    assert_eq!(line, "Work VPN · VPN connected");

    // Switch from Wi-Fi to Ethernet.
    server
        .at(
            LAN,
            FakeActive {
                id: "Office LAN",
                kind: "802-3-ethernet",
                vpn: false,
            },
        )
        .await?;
    {
        let mut n = nm.get_mut().await;
        n.primary = LAN.into();
        n.active = vec![LAN.into(), VPN.into()];
    }
    nm.get()
        .await
        .primary_connection_changed(nm.signal_emitter())
        .await?;
    let line = wait_for(&hub, "ethernet", |ev| {
        find(ev, "network:primary")
            .and_then(system_line)
            .filter(|l| l.contains("Office"))
    })
    .await?;
    assert_eq!(line, "Office LAN · Ethernet connected");

    // A login page appears, then everything disconnects.
    nm.get_mut().await.connectivity = 2;
    nm.get()
        .await
        .connectivity_changed(nm.signal_emitter())
        .await?;
    let line = wait_for(&hub, "portal", |ev| {
        find(ev, "network:portal").and_then(system_line)
    })
    .await?;
    assert_eq!(line, "Sign in to network · Office LAN");

    {
        let mut n = nm.get_mut().await;
        n.primary = "/".into();
        n.active = vec![];
        n.connectivity = 1;
    }
    nm.get()
        .await
        .active_connections_changed(nm.signal_emitter())
        .await?;
    let line = wait_for(&hub, "offline", |ev| {
        find(ev, "network:primary")
            .and_then(system_line)
            .filter(|l| l.starts_with("Offline"))
    })
    .await?;
    assert_eq!(line, "Offline · No network connection");
    let ev = events(&hub).await;
    assert!(
        find(&ev, "network:portal").is_none_or(|e| e.state.is_terminal()),
        "portal cleared"
    );
    assert_eq!(
        find(&ev, "network:vpn:Work_VPN")
            .and_then(system_line)
            .as_deref(),
        Some("Work VPN · VPN disconnected")
    );
    watcher.abort();
    Ok(())
}

// ---- UDisks2 --------------------------------------------------------------

struct FakeDrive {
    vendor: &'static str,
    model: &'static str,
    bus: &'static str,
}

#[interface(name = "org.freedesktop.UDisks2.Drive")]
impl FakeDrive {
    #[zbus(property)]
    fn vendor(&self) -> String {
        self.vendor.into()
    }
    #[zbus(property)]
    fn model(&self) -> String {
        self.model.into()
    }
    #[zbus(property)]
    fn connection_bus(&self) -> String {
        self.bus.into()
    }
    #[zbus(property)]
    fn removable(&self) -> bool {
        self.bus == "usb"
    }
}

#[tokio::test]
async fn storage_reports_usb_drives_only() -> TestResult {
    if !have_bus() {
        return Ok(());
    }
    const ROOT: &str = "/org/freedesktop/UDisks2";
    const NVME: &str = "/org/freedesktop/UDisks2/drives/Samsung_SSD";
    const STICK: &str = "/org/freedesktop/UDisks2/drives/SanDisk_Ultra_42";
    let service = zbus::connection::Builder::session()
        .serve_at(
            NVME,
            FakeDrive {
                vendor: "",
                model: "Samsung SSD 980",
                bus: "",
            },
        )
        .serve_at(ROOT, zbus::fdo::ObjectManager)
        .name("org.freedesktop.UDisks2")
        .build()
        .await?;
    let hub = new_hub();
    let watcher = tokio::spawn(selawi_islandd::integrations::storage::watch(
        hub.clone(),
        Connection::session().await?,
    ));
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert!(events(&hub).await.is_empty());
    assert!(
        hub.integrations_json().await.contains("0 removable drives"),
        "internal disk ignored"
    );

    service
        .object_server()
        .at(
            STICK,
            FakeDrive {
                vendor: "SanDisk",
                model: "Ultra",
                bus: "usb",
            },
        )
        .await?;
    let id = "storage:SanDisk_Ultra_42";
    let line = wait_for(&hub, "plugged in", |ev| find(ev, id).and_then(system_line)).await?;
    assert_eq!(line, "SanDisk Ultra · USB drive connected");

    service
        .object_server()
        .remove::<FakeDrive, _>(STICK)
        .await?;
    let line = wait_for(&hub, "removed", |ev| {
        find(ev, id)
            .and_then(system_line)
            .filter(|l| l.ends_with("removed"))
    })
    .await?;
    assert_eq!(line, "SanDisk Ultra · USB drive removed");
    watcher.abort();
    Ok(())
}

// ---- System warnings -------------------------------------------------------

#[tokio::test]
async fn health_warns_about_low_memory_from_fake_proc() -> TestResult {
    let root = std::env::temp_dir().join(format!("selawi-health-it-{}", std::process::id()));
    std::fs::create_dir_all(root.join("proc"))?;
    std::fs::write(
        root.join("proc/meminfo"),
        "MemTotal: 8000000 kB\nMemAvailable: 400000 kB\n",
    )?;
    let hub = new_hub();
    let task_root = root.clone();
    let watcher = tokio::spawn({
        let hub = hub.clone();
        async move { selawi_islandd::integrations::health::watch(hub, &task_root).await }
    });
    let line = wait_for(&hub, "memory warning", |ev| {
        find(ev, "health:memory").and_then(system_line)
    })
    .await?;
    assert!(line.starts_with("Memory low · 5% free"), "{line}");
    assert!(hub
        .integrations_json()
        .await
        .contains("checks memory every 15 s"));
    watcher.abort();
    std::fs::remove_dir_all(root)?;
    Ok(())
}

/// Turning Wi-Fi on makes NetworkManager announce every access point it
/// scans. That flood must not stop the integration from seeing the
/// connection come back (it used to fill the queue and hang).
#[tokio::test]
async fn wifi_coming_back_survives_a_scan_flood() -> TestResult {
    if !have_bus() {
        return Ok(());
    }
    const NM: &str = "/org/freedesktop/NetworkManager";
    const WIFI: &str = "/org/freedesktop/NetworkManager/ActiveConnection/7";
    let service = zbus::connection::Builder::session()
        .serve_at(
            NM,
            FakeNm {
                primary: WIFI.into(),
                active: vec![WIFI.into()],
                connectivity: 4,
            },
        )
        .serve_at(
            WIFI,
            FakeActive {
                id: "Home",
                kind: "802-11-wireless",
                vpn: false,
            },
        )
        .name("org.freedesktop.NetworkManager")
        .build()
        .await?;
    let hub = new_hub();
    let watcher = tokio::spawn(selawi_islandd::integrations::network::watch(
        hub.clone(),
        Connection::session().await?,
    ));
    tokio::time::sleep(Duration::from_millis(300)).await;
    let nm = service.object_server().interface::<_, FakeNm>(NM).await?;

    // Wi-Fi off.
    {
        let mut n = nm.get_mut().await;
        n.primary = "/".into();
        n.active = Vec::new();
    }
    nm.get()
        .await
        .active_connections_changed(nm.signal_emitter())
        .await?;
    nm.get()
        .await
        .primary_connection_changed(nm.signal_emitter())
        .await?;
    wait_for(&hub, "offline", |ev| {
        find(ev, "network:primary")
            .and_then(system_line)
            .filter(|l| l.starts_with("Offline"))
    })
    .await?;

    // Wi-Fi on: the connection comes back in the middle of a scan flood.
    for i in 0..400 {
        let mut changed: HashMap<&str, Variant<'_>> = HashMap::new();
        changed.insert("Strength", Variant::U8((i % 100) as u8));
        service
            .emit_signal(
                None::<&str>,
                format!("{NM}/AccessPoint/{i}").as_str(),
                "org.freedesktop.DBus.Properties",
                "PropertiesChanged",
                &(
                    "org.freedesktop.NetworkManager.AccessPoint",
                    changed,
                    Vec::<String>::new(),
                ),
            )
            .await?;
        if i == 50 {
            {
                let mut n = nm.get_mut().await;
                n.primary = WIFI.into();
                n.active = vec![WIFI.into()];
            }
            nm.get()
                .await
                .active_connections_changed(nm.signal_emitter())
                .await?;
            nm.get()
                .await
                .primary_connection_changed(nm.signal_emitter())
                .await?;
        }
    }
    let line = wait_for(&hub, "Wi-Fi back", |ev| {
        find(ev, "network:primary")
            .and_then(system_line)
            .filter(|l| l.contains("Wi-Fi connected"))
    })
    .await?;
    assert_eq!(line, "Home · Wi-Fi connected");
    assert!(hub.integrations_json().await.contains("on Home"));
    watcher.abort();
    Ok(())
}
