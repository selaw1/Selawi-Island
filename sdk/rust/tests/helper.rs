//! The Rust helper against the daemon's plugin host on a private bus
//! (run under dbus-run-session by scripts/test.sh).

use std::time::Duration;

use futures_util::StreamExt;
use selawi_core::Timestamp;
use selawi_island_plugin::Island;
use selawi_islandd::dbus::OBJECT_PATH;
use selawi_islandd::engine::Engine;
use selawi_islandd::hub::Hub;
use selawi_islandd::plugins::{PluginHost, Plugins};

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[tokio::test]
async fn register_show_and_buttons() -> TestResult {
    if std::env::var_os("DBUS_SESSION_BUS_ADDRESS").is_none() {
        eprintln!("No D-Bus session bus: skipped");
        return Ok(());
    }
    let hub = std::sync::Arc::new(Hub::new(Engine::new(None, Timestamp::now())));
    let host = PluginHost::new(hub.clone());
    let server = zbus::connection::Builder::session()
        .serve_at(OBJECT_PATH, Plugins { host: host.clone() })
        .name("com.selawi.Island")
        .build()
        .await?;
    host.start(server.clone());
    tokio::time::sleep(Duration::from_millis(100)).await;

    let island = Island::register(
        r#"{"manifest_version":1,"id":"countdown","name":"Focus timer","categories":["custom"]}"#,
    )
    .await?;
    assert_eq!(island.id(), "countdown");
    assert!(!island.allowed());

    let mut answers = island.allowed_changes().await?;
    hub.set_config(r#"{"plugins":{"countdown":true}}"#)?;
    let answer = tokio::time::timeout(Duration::from_secs(3), answers.next()).await?;
    assert_eq!(answer, Some(true), "the app learns it was allowed");

    island
        .show(
            r#"{"id":"focus","category":"custom","state":"started",
                "persistent":true,"payload":{"type":"progress","title":"Focus"},
                "actions":[{"id":"stop","label":"Stop"}]}"#,
        )
        .await?;
    assert!(hub.is_active("plugin:countdown:focus").await);

    let mut pressed = island.actions().await?;
    assert!(hub.route_action("plugin:countdown:focus", "stop").await);
    let press = tokio::time::timeout(Duration::from_secs(3), pressed.next()).await?;
    assert_eq!(press, Some(("focus".to_string(), "stop".to_string())));
    assert!(island.remove("focus").await?);
    Ok(())
}
