//! selawi-islandd: the Selawi Island daemon.
//!
//! Runs the event engine and serves it on the D-Bus session bus as
//! `com.selawi.Island`. Normally started by systemd (`selawi-islandd.service`)
//! or by D-Bus activation.

mod dbus;
mod engine;

use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::Arc;
use std::time::Duration;

use selawi_core::Timestamp;
use tokio::signal::unix::{signal, SignalKind};
use tokio::sync::{Mutex, Notify};

use crate::dbus::{Core, Events, Shared, BUS_NAME, OBJECT_PATH};
use crate::engine::{default_snapshot_path, Engine};

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
    let shared = Arc::new(Shared {
        engine: Mutex::new(engine),
        wake: Notify::new(),
    });

    let connection = zbus::connection::Builder::session()
        .serve_at(OBJECT_PATH, Core)
        .serve_at(
            OBJECT_PATH,
            Events {
                shared: shared.clone(),
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
    let emitter = iface.signal_emitter().clone();

    let mut terminate = signal(SignalKind::terminate())?;
    let mut interrupt = signal(SignalKind::interrupt())?;
    tokio::select! {
        result = expire_loop(shared, emitter) => result?,
        _ = terminate.recv() => eprintln!("Stopping (SIGTERM)"),
        _ = interrupt.recv() => eprintln!("Stopping (SIGINT)"),
    }
    Ok(())
}

/// Sleeps until the next expiry (or a change), never polls.
async fn expire_loop(
    shared: Arc<Shared>,
    emitter: zbus::object_server::SignalEmitter<'static>,
) -> zbus::Result<()> {
    loop {
        let deadline = shared.engine.lock().await.next_deadline();
        match deadline {
            None => {
                shared.wake.notified().await;
                continue;
            }
            Some(deadline) => {
                let wait = deadline.0.saturating_sub(Timestamp::now().0);
                tokio::select! {
                    () = tokio::time::sleep(Duration::from_millis(wait)) => {}
                    () = shared.wake.notified() => continue,
                }
            }
        }
        let list = {
            let mut engine = shared.engine.lock().await;
            engine.tick(Timestamp::now()).then(|| engine.list_json())
        };
        if let Some(list) = list {
            Events::changed(&emitter, &list).await?;
        }
    }
}
