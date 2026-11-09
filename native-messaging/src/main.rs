//! selawi-island-browser-host: started by Chrome/Firefox for the Selawi
//! Island extension (native messaging). It forwards checked events to the
//! daemon over D-Bus and sends button presses back to the extension.
//!
//! It never runs commands; it only relays events that pass
//! `protocol::validate_event`. When the browser disconnects, every event
//! it still shows is ended.

mod protocol;

use std::collections::HashMap;
use std::process::ExitCode;

use futures_util::StreamExt;
use selawi_core::{EventInput, LifecycleState};
use tokio::io::{stdin, stdout, Stdout};
use tokio::sync::mpsc;
use zbus::proxy;

use crate::protocol::{
    read_message, validate_event, write_message, FromBrowser, RateLimit, ToBrowser, ID_PREFIX,
};

#[proxy(
    interface = "com.selawi.Island.Events1",
    default_service = "com.selawi.Island",
    default_path = "/com/selawi/Island"
)]
trait Events {
    fn submit(&self, event_json: &str) -> zbus::Result<()>;
    #[zbus(signal)]
    fn action_invoked(&self, id: &str, action_id: &str) -> zbus::Result<()>;
}

fn main() -> ExitCode {
    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(r) => r,
        Err(e) => {
            eprintln!("selawi-island-browser-host: {e}");
            return ExitCode::FAILURE;
        }
    };
    match runtime.block_on(run()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            // stderr ends up in the browser's log.
            eprintln!("selawi-island-browser-host: {e}");
            ExitCode::FAILURE
        }
    }
}

async fn run() -> Result<(), Box<dyn std::error::Error>> {
    let connection = zbus::Connection::session().await?;
    let events = EventsProxy::new(&connection).await?;
    let mut actions = events.receive_action_invoked().await?;

    // Messages from the browser are read on their own task.
    let (tx, mut from_browser) = mpsc::channel::<Vec<u8>>(64);
    tokio::spawn(async move {
        let mut input = stdin();
        loop {
            match read_message(&mut input).await {
                Ok(Some(body)) => {
                    if tx.send(body).await.is_err() {
                        break;
                    }
                }
                Ok(None) => break,
                Err(e) => {
                    eprintln!("selawi-island-browser-host: {e}");
                    break;
                }
            }
        }
    });

    let mut out = stdout();
    let mut shown: HashMap<String, EventInput> = HashMap::new();
    let mut limit = RateLimit::new(50, 20);

    loop {
        tokio::select! {
            message = from_browser.recv() => {
                let Some(body) = message else { break };
                if !limit.allow() {
                    eprintln!("selawi-island-browser-host: too many messages, dropped one");
                    continue;
                }
                handle(&events, &mut shown, &body).await;
            }
            signal = actions.next() => {
                let Some(signal) = signal else { break };
                let Ok(args) = signal.args() else { continue };
                let id = args.id().to_string();
                if id.starts_with(ID_PREFIX) && shown.contains_key(&id) {
                    let action = ToBrowser::Action { id, action: args.action_id().to_string() };
                    send(&mut out, &action).await?;
                }
            }
        }
    }

    // The browser went away: end what it was showing.
    for (_, mut input) in shown {
        input.state = LifecycleState::Ended;
        input.actions.clear();
        if let Ok(json) = serde_json::to_string(&input) {
            let _ = events.submit(&json).await;
        }
    }
    Ok(())
}

async fn handle(events: &EventsProxy<'_>, shown: &mut HashMap<String, EventInput>, body: &[u8]) {
    let message: FromBrowser = match serde_json::from_slice(body) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("selawi-island-browser-host: refused message: {e}");
            return;
        }
    };
    match message {
        FromBrowser::Hello { .. } => {}
        FromBrowser::Event { event } => {
            let input = match validate_event(&event) {
                Ok(input) => input,
                Err(e) => {
                    eprintln!("selawi-island-browser-host: refused event: {e}");
                    return;
                }
            };
            let Ok(json) = serde_json::to_string(&input) else {
                return;
            };
            if let Err(e) = events.submit(&json).await {
                eprintln!("selawi-island-browser-host: daemon refused event: {e}");
                return;
            }
            if input.state.is_terminal() {
                shown.remove(input.id.as_str());
            } else {
                shown.insert(input.id.to_string(), input);
            }
        }
    }
}

async fn send(out: &mut Stdout, message: &ToBrowser) -> std::io::Result<()> {
    let body = serde_json::to_vec(message).map_err(std::io::Error::other)?;
    write_message(out, &body).await
}
