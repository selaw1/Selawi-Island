//! Plugins1 end to end on a private session bus: a "Pomodoro" app
//! registers, is asked about, gets allowed, shows sealed events, receives
//! its own button presses, is rate limited and capped, and its events go
//! when it disconnects. Run under dbus-run-session (scripts/test.sh).

use std::sync::Arc;
use std::time::Duration;

use futures_util::StreamExt;
use selawi_core::{Event, Timestamp};
use selawi_islandd::dbus::OBJECT_PATH;
use selawi_islandd::engine::Engine;
use selawi_islandd::hub::Hub;
use selawi_islandd::plugins::{PluginHost, Plugins, INTERFACE};
use zbus::message::Type as MessageType;
use zbus::{Connection, MessageStream};

type TestResult = Result<(), Box<dyn std::error::Error>>;

const MANIFEST: &str = r#"{"manifest_version":1,"id":"pomodoro","name":"Pomodoro",
    "description":"Focus timer","icon":"app:org.example.Pomodoro","categories":["custom","notification"]}"#;

fn timer(local: &str, title: &str) -> String {
    format!(
        r#"{{"id":"{local}","source":{{"app":"Slack","integration":"mpris"}},"category":"custom",
            "state":"started","persistent":true,"payload":{{"type":"progress","title":"{title}"}},
            "actions":[{{"id":"pause","label":"Pause"}}]}}"#
    )
}

async fn events(hub: &Hub) -> Vec<Event> {
    serde_json::from_str(&hub.list_json().await).unwrap_or_default()
}

async fn wait_until<F: Fn(&[Event]) -> bool>(
    hub: &Hub,
    what: &str,
    check: F,
) -> Result<(), String> {
    for _ in 0..60 {
        if check(&events(hub).await) {
            return Ok(());
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    Err(format!(
        "timed out waiting for {what}; events: {}",
        hub.list_json().await
    ))
}

struct Island {
    hub: Arc<Hub>,
    name: String,
    _server: Connection,
}

async fn island() -> Result<Island, Box<dyn std::error::Error>> {
    let hub = Arc::new(Hub::new(Engine::new(None, Timestamp::now())));
    let host = PluginHost::new(hub.clone());
    let server = zbus::connection::Builder::session()
        .serve_at(OBJECT_PATH, Plugins { host: host.clone() })
        .build()
        .await?;
    host.start(server.clone());
    let name = server.unique_name().ok_or("no unique name")?.to_string();
    tokio::time::sleep(Duration::from_millis(100)).await;
    Ok(Island {
        hub,
        name,
        _server: server,
    })
}

async fn call<B>(
    client: &Connection,
    island: &Island,
    method: &str,
    body: &B,
) -> zbus::Result<zbus::Message>
where
    B: serde::Serialize + zbus::zvariant::DynamicType,
{
    client
        .call_method(
            Some(island.name.as_str()),
            OBJECT_PATH,
            Some(INTERFACE),
            method,
            body,
        )
        .await
}

fn error_name(result: zbus::Result<zbus::Message>) -> String {
    match result {
        Err(zbus::Error::MethodError(name, _, _)) => name.to_string(),
        Err(e) => format!("other error: {e}"),
        Ok(_) => "ok".into(),
    }
}

#[tokio::test]
async fn a_plugin_from_registration_to_disconnect() -> TestResult {
    if std::env::var_os("DBUS_SESSION_BUS_ADDRESS").is_none() {
        eprintln!("No D-Bus session bus: skipped");
        return Ok(());
    }
    let island = island().await?;
    let hub = &island.hub;
    let app = Connection::session().await?;

    // Not registered yet: nothing goes through.
    assert_eq!(
        error_name(call(&app, &island, "Submit", &(timer("t", "x"),)).await),
        "com.selawi.Island.Error.NotRegistered"
    );

    // Register: not allowed yet, and the island asks.
    let reply = call(&app, &island, "Register", &(MANIFEST,)).await?;
    let (id, allowed): (String, bool) = reply.body().deserialize()?;
    assert_eq!((id.as_str(), allowed), ("pomodoro", false));
    wait_until(hub, "the question", |ev| {
        ev.iter()
            .any(|e| e.id.as_str() == "plugins:request:pomodoro" && e.actions.len() == 2)
    })
    .await?;
    assert_eq!(
        error_name(call(&app, &island, "Submit", &(timer("t", "x"),)).await),
        "com.selawi.Island.Error.NotAllowed",
        "nothing shows before the user allows it"
    );

    // The user presses Allow on the island.
    assert!(hub.route_action("plugins:request:pomodoro", "allow").await);
    wait_until(hub, "the question to go", |ev| {
        !ev.iter()
            .any(|e| e.id.as_str().starts_with("plugins:request:") && !e.state.is_terminal())
    })
    .await?;
    assert!(hub.config().plugin_allowed("pomodoro"));

    // Sealed: own namespace, the manifest's name and icon.
    call(&app, &island, "Submit", &(timer("timer", "Focus"),)).await?;
    let list = events(hub).await;
    let shown = list
        .iter()
        .find(|e| e.id.as_str() == "plugin:pomodoro:timer")
        .ok_or("event not shown")?;
    assert_eq!(shown.source.app, "Pomodoro", "cannot pose as Slack");
    assert_eq!(shown.source.integration, "plugin");
    assert_eq!(
        shown.source.icon.as_deref(),
        Some("app:org.example.Pomodoro")
    );

    // Not declared in the manifest: refused.
    let call_event = r#"{"id":"c","source":{"app":"x","integration":"x"},"category":"call","state":"started",
        "payload":{"type":"call","title":"Fake","confidence":"confirmed"}}"#;
    assert_eq!(
        error_name(call(&app, &island, "Submit", &(call_event,)).await),
        "com.selawi.Island.Error.InvalidEvent"
    );
    // The plugin namespace cannot be used from Events1.Submit.
    assert!(hub
        .submit_json(&timer("plugin:pomodoro:fake", "x"))
        .await
        .is_err());

    // A button on its event comes back to it, and only to it.
    let mut incoming = MessageStream::from(&app);
    assert!(hub.route_action("plugin:pomodoro:timer", "pause").await);
    let pressed = tokio::time::timeout(Duration::from_secs(3), async {
        while let Some(Ok(message)) = incoming.next().await {
            let header = message.header();
            if header.message_type() == MessageType::Signal
                && header.member().map(|m| m.as_str()) == Some("ActionInvoked")
            {
                return message.body().deserialize::<(String, String)>().ok();
            }
        }
        None
    })
    .await?;
    assert_eq!(pressed, Some(("timer".into(), "pause".into())));

    // Another app cannot take its id.
    let impostor = Connection::session().await?;
    assert_eq!(
        error_name(call(&impostor, &island, "Register", &(MANIFEST,)).await),
        "com.selawi.Island.Error.InvalidPlugin"
    );

    // At most 8 events at a time.
    for i in 0..7 {
        call(&app, &island, "Submit", &(timer(&format!("t{i}"), "x"),)).await?;
    }
    assert_eq!(
        error_name(call(&app, &island, "Submit", &(timer("t9", "x"),)).await),
        "com.selawi.Island.Error.InvalidEvent",
        "a ninth event is refused"
    );
    let reply = call(&app, &island, "Remove", &("t0",)).await?;
    assert!(reply.body().deserialize::<bool>()?);

    // Too fast: rate limited.
    let mut limited = false;
    for _ in 0..60 {
        if error_name(call(&app, &island, "Submit", &(timer("timer", "Focus"),)).await)
            == "com.selawi.Island.Error.RateLimited"
        {
            limited = true;
            break;
        }
    }
    assert!(limited, "60 updates at once are rate limited");

    // Settings → List shows it running and allowed.
    let reply = call(&app, &island, "List", &()).await?;
    let list: String = reply.body().deserialize()?;
    assert!(
        list.contains(r#""id":"pomodoro""#)
            && list.contains(r#""allowed":true"#)
            && list.contains(r#""running":true"#)
    );

    // The app quits: its events go.
    drop(incoming);
    app.graceful_shutdown().await;
    wait_until(hub, "its events to go", |ev| {
        !ev.iter()
            .any(|e| e.id.as_str().starts_with("plugin:pomodoro:") && !e.state.is_terminal())
    })
    .await?;

    // The user turns it off later: nothing more from it.
    let again = Connection::session().await?;
    call(&again, &island, "Register", &(MANIFEST,)).await?;
    call(&again, &island, "Submit", &(timer("timer", "Focus"),)).await?;
    hub.set_config(r#"{"plugins":{"pomodoro":false}}"#)?;
    wait_until(hub, "events of a turned-off plugin to go", |ev| {
        !ev.iter()
            .any(|e| e.id.as_str().starts_with("plugin:pomodoro:") && !e.state.is_terminal())
    })
    .await?;
    assert_eq!(
        error_name(call(&again, &island, "Submit", &(timer("timer", "Focus"),)).await),
        "com.selawi.Island.Error.NotAllowed"
    );
    Ok(())
}
