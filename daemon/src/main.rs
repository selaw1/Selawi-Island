//! selawi-islandd: the Selawi Island daemon.
//!
//! Runs the event engine and serves it on the D-Bus session bus as
//! `com.selawi.Island`. Normally started by systemd (`selawi-islandd.service`)
//! or by D-Bus activation.

use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::Arc;
use std::time::Duration;

use selawi_core::Timestamp;
use selawi_islandd::config::{self, Config};
use selawi_islandd::dbus::{Configuration, Core, Events, Integrations, BUS_NAME, OBJECT_PATH};
use selawi_islandd::engine::{default_snapshot_path, Engine};
use selawi_islandd::hub::Hub;
use selawi_islandd::integrations::{
    self, bluez, health, mpris, network, notifications, storage, upower,
};
use selawi_islandd::plugins::{PluginHost, Plugins};
use tokio::signal::unix::{signal, SignalKind};

const USAGE: &str = "\
Usage: selawi-islandd [--no-persist] [--state-file PATH]

  --no-persist         Keep events in memory only (nothing written to disk)
  --state-file PATH    Where to keep persistent events across restarts
                       (default: $XDG_STATE_HOME/selawi-island/events.json)
  --version            Print the version";

struct Options {
    snapshot_path: Option<PathBuf>,
}

fn parse_args() -> Result<Option<Options>, String> {
    let mut snapshot_path = default_snapshot_path();
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--no-persist" => snapshot_path = None,
            "--state-file" => {
                let path = args.next().ok_or("--state-file needs a path")?;
                snapshot_path = Some(PathBuf::from(path));
            }
            "--version" => {
                println!("selawi-islandd {}", env!("CARGO_PKG_VERSION"));
                return Ok(None);
            }
            "-h" | "--help" => {
                println!("{USAGE}");
                return Ok(None);
            }
            other => return Err(format!("unknown argument {other:?}\n\n{USAGE}")),
        }
    }
    Ok(Some(Options { snapshot_path }))
}

fn main() -> ExitCode {
    let options = match parse_args() {
        Ok(Some(o)) => o,
        Ok(None) => return ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::from(2);
        }
    };
    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(r) => r,
        Err(e) => {
            eprintln!("Could not start: {e}");
            return ExitCode::FAILURE;
        }
    };
    match runtime.block_on(run(options)) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("selawi-islandd: {e}");
            ExitCode::FAILURE
        }
    }
}

async fn run(options: Options) -> Result<(), Box<dyn std::error::Error>> {
    let engine = Engine::new(options.snapshot_path, Timestamp::now());
    eprintln!("Starting with {} restored event(s)", engine.len());
    let config_path = config::default_path();
    let config = Config::load(config_path.as_deref());
    let hub = Arc::new(Hub::with_config(engine, config, config_path));
    let plugin_host = PluginHost::new(hub.clone());

    let connection = zbus::connection::Builder::session()
        .serve_at(OBJECT_PATH, Core)
        .serve_at(OBJECT_PATH, Events { hub: hub.clone() })
        .serve_at(OBJECT_PATH, Integrations { hub: hub.clone() })
        .serve_at(OBJECT_PATH, Configuration { hub: hub.clone() })
        .serve_at(
            OBJECT_PATH,
            Plugins {
                host: plugin_host.clone(),
            },
        )
        .name(BUS_NAME)
        .build()
        .await?;
    eprintln!("Serving {BUS_NAME} on the session bus");

    let iface = connection
        .object_server()
        .interface::<_, Events>(OBJECT_PATH)
        .await?;
    hub.set_emitter(iface.signal_emitter().clone());
    tokio::spawn(hub.clone().send_changes());
    plugin_host.start(connection.clone());

    integrations::spawn(hub.clone(), mpris::ID, mpris::NAME, mpris::run);
    integrations::spawn(hub.clone(), upower::ID, upower::NAME, upower::run);
    integrations::spawn(hub.clone(), bluez::ID, bluez::NAME, bluez::run);
    integrations::spawn(hub.clone(), network::ID, network::NAME, network::run);
    integrations::spawn(hub.clone(), storage::ID, storage::NAME, storage::run);
    integrations::spawn(hub.clone(), health::ID, health::NAME, health::run);
    integrations::spawn(
        hub.clone(),
        notifications::ID,
        notifications::NAME,
        notifications::run,
    );

    let mut terminate = signal(SignalKind::terminate())?;
    let mut interrupt = signal(SignalKind::interrupt())?;
    tokio::select! {
        () = expire_loop(hub) => {}
        _ = terminate.recv() => eprintln!("Stopping (SIGTERM)"),
        _ = interrupt.recv() => eprintln!("Stopping (SIGINT)"),
    }
    Ok(())
}

/// Sleeps until the next expiry (or a change), never polls.
async fn expire_loop(hub: Arc<Hub>) {
    loop {
        match hub.next_deadline().await {
            None => hub.wake.notified().await,
            Some(deadline) => {
                let wait = deadline.0.saturating_sub(Timestamp::now().0);
                tokio::select! {
                    () = tokio::time::sleep(Duration::from_millis(wait)) => {
                        hub.tick().await;
                    }
                    () = hub.wake.notified() => {}
                }
            }
        }
    }
}
