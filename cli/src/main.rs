//! selawi-island: send events to the Selawi Island daemon from a terminal
//! or script. See docs/CLI.md.

use std::io::Read;
use std::process::ExitCode;

use clap::{Parser, Subcommand, ValueEnum};
use selawi_core::events::{
    Category, EventId, EventInput, LifecycleState, NotificationPayload, Payload, ProgressPayload,
    Source,
};
use selawi_core::{parse_input, Event, Priority, Timestamp};
use zbus::proxy;

const DEFAULT_APP: &str = "Terminal";
const INTEGRATION: &str = "cli";
/// Unfinished progress disappears if a script dies and never finishes it.
const DEFAULT_PROGRESS_TTL_SECS: u64 = 60 * 60;

#[proxy(
    interface = "com.selawi.Island.Events1",
    default_service = "com.selawi.Island",
    default_path = "/com/selawi/Island"
)]
trait Events {
    fn submit(&self, event_json: &str) -> zbus::Result<()>;
    fn list(&self) -> zbus::Result<String>;
    fn dismiss(&self, id: &str) -> zbus::Result<bool>;
}

#[proxy(
    interface = "com.selawi.Island.Core1",
    default_service = "com.selawi.Island",
    default_path = "/com/selawi/Island"
)]
trait Core {
    #[zbus(property)]
    fn version(&self) -> zbus::Result<String>;
    #[zbus(property)]
    fn protocol_version(&self) -> zbus::Result<u32>;
}

#[proxy(
    interface = "com.selawi.Island.Integrations1",
    default_service = "com.selawi.Island",
    default_path = "/com/selawi/Island"
)]
trait Integrations {
    fn list(&self) -> zbus::Result<String>;
}

#[derive(serde::Deserialize)]
struct IntegrationRow {
    id: String,
    name: String,
    status: String,
    #[serde(default)]
    detail: Option<String>,
}

#[derive(Parser)]
#[command(
    name = "selawi-island",
    version,
    about = "Send events to Selawi Island"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Show a short notification.
    Notify {
        /// Notification title.
        title: String,
        /// Second line of text.
        #[arg(long)]
        body: Option<String>,
        /// Application name shown with it.
        #[arg(long, default_value = DEFAULT_APP)]
        app: String,
        /// Seconds to keep it visible (default 8).
        #[arg(long)]
        ttl: Option<u64>,
        /// Reuse an id to replace an earlier notification.
        #[arg(long)]
        id: Option<String>,
        /// Priority tier (default: notification).
        #[arg(long, value_enum)]
        priority: Option<PriorityArg>,
    },
    /// Show and update a task with progress.
    Progress {
        #[command(subcommand)]
        action: ProgressCommand,
    },
    /// Remove an event.
    Dismiss { id: String },
    /// Print the current events as JSON.
    List,
    /// Send a raw protocol v1 event (JSON). Use "-" to read from stdin.
    Send { json: String },
    /// Check that the daemon is running.
    Status,
    /// Show the built-in integrations and what each is doing.
    Integrations,
}

#[derive(Subcommand)]
enum ProgressCommand {
    /// Start a task. Prints its id for later updates.
    Start {
        #[arg(long)]
        title: String,
        #[arg(long)]
        detail: Option<String>,
        /// 0–100. Leave out when unknown.
        #[arg(long)]
        percent: Option<f64>,
        #[arg(long, default_value = DEFAULT_APP)]
        app: String,
        /// Choose the id instead of generating one.
        #[arg(long)]
        id: Option<String>,
        /// Seconds without an update before it disappears (default 3600).
        #[arg(long)]
        ttl: Option<u64>,
    },
    /// Change a running task. Unspecified fields keep their value.
    Update {
        #[arg(long)]
        id: String,
        #[arg(long)]
        title: Option<String>,
        #[arg(long)]
        detail: Option<String>,
        #[arg(long)]
        percent: Option<f64>,
    },
    /// Finish a task. It stays visible for a few seconds, then goes.
    Finish {
        #[arg(long)]
        id: String,
        #[arg(long, value_enum, default_value_t = Status::Success)]
        status: Status,
        #[arg(long)]
        detail: Option<String>,
    },
}

#[derive(Clone, Copy, ValueEnum)]
enum Status {
    Success,
    Failure,
}

#[derive(Clone, Copy, ValueEnum)]
enum PriorityArg {
    Critical,
    Call,
    Device,
    Notification,
    Media,
    Transfer,
    Reminder,
    Metric,
}

impl From<PriorityArg> for Priority {
    fn from(p: PriorityArg) -> Priority {
        match p {
            PriorityArg::Critical => Priority::Critical,
            PriorityArg::Call => Priority::Call,
            PriorityArg::Device => Priority::Device,
            PriorityArg::Notification => Priority::Notification,
            PriorityArg::Media => Priority::Media,
            PriorityArg::Transfer => Priority::Transfer,
            PriorityArg::Reminder => Priority::Reminder,
            PriorityArg::Metric => Priority::Metric,
        }
    }
}

type Result<T> = std::result::Result<T, String>;

fn main() -> ExitCode {
    let cli = Cli::parse();
    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(r) => r,
        Err(e) => {
            eprintln!("selawi-island: {e}");
            return ExitCode::FAILURE;
        }
    };
    match runtime.block_on(run(cli.command)) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("selawi-island: {e}");
            ExitCode::FAILURE
        }
    }
}

async fn run(command: Command) -> Result<()> {
    let connection = zbus::Connection::session()
        .await
        .map_err(|e| format!("no D-Bus session bus: {e}"))?;
    let events = EventsProxy::new(&connection).await.map_err(dbus_error)?;

    match command {
        Command::Notify {
            title,
            body,
            app,
            ttl,
            id,
            priority,
        } => {
            let input = EventInput {
                id: id_or_new(id, "notify")?,
                source: source(app),
                category: Category::Notification,
                state: LifecycleState::Started,
                priority: priority.map(Priority::from),
                persistent: false,
                ttl_ms: ttl.map(secs_to_ms),
                group: None,
                payload: Payload::Notification(NotificationPayload { title, body }),
                actions: Vec::new(),
            };
            submit(&events, &input).await
        }
        Command::Progress { action } => progress(&events, action).await,
        Command::Dismiss { id } => {
            if events.dismiss(&id).await.map_err(dbus_error)? {
                Ok(())
            } else {
                Err(format!("no event with id {id:?}"))
            }
        }
        Command::List => {
            println!("{}", events.list().await.map_err(dbus_error)?);
            Ok(())
        }
        Command::Send { json } => {
            let json = if json == "-" {
                let mut buf = String::new();
                std::io::stdin()
                    .read_to_string(&mut buf)
                    .map_err(|e| e.to_string())?;
                buf
            } else {
                json
            };
            // Validate locally first for a clearer message.
            parse_input(&json).map_err(|e| e.to_string())?;
            events.submit(&json).await.map_err(dbus_error)
        }
        Command::Integrations => {
            let proxy = IntegrationsProxy::new(&connection)
                .await
                .map_err(dbus_error)?;
            let rows: Vec<IntegrationRow> =
                serde_json::from_str(&proxy.list().await.map_err(dbus_error)?)
                    .map_err(|e| e.to_string())?;
            for row in rows {
                let detail = row.detail.map(|d| format!(" — {d}")).unwrap_or_default();
                println!(
                    "{:<8} {:<26} {}{detail}",
                    row.id,
                    row.name,
                    row.status.replace('_', " ")
                );
            }
            Ok(())
        }
        Command::Status => {
            let core = CoreProxy::new(&connection).await.map_err(dbus_error)?;
            let version = core.version().await.map_err(dbus_error)?;
            let protocol = core.protocol_version().await.map_err(dbus_error)?;
            let list: Vec<Event> = serde_json::from_str(&events.list().await.map_err(dbus_error)?)
                .map_err(|e| e.to_string())?;
            println!(
                "Daemon running: version {version}, protocol {protocol}, {} event(s)",
                list.len()
            );
            Ok(())
        }
    }
}

async fn progress(events: &EventsProxy<'_>, action: ProgressCommand) -> Result<()> {
    match action {
        ProgressCommand::Start {
            title,
            detail,
            percent,
            app,
            id,
            ttl,
        } => {
            let id = id_or_new(id, "task")?;
            let input = progress_input(
                id.clone(),
                source(app),
                LifecycleState::Started,
                ProgressPayload {
                    title,
                    detail,
                    percent: check_percent(percent)?,
                },
                ttl.unwrap_or(DEFAULT_PROGRESS_TTL_SECS),
            );
            submit(events, &input).await?;
            println!("{id}");
            Ok(())
        }
        ProgressCommand::Update {
            id,
            title,
            detail,
            percent,
        } => {
            let (current, mut payload) = current_progress(events, &id).await?;
            if let Some(title) = title {
                payload.title = title;
            }
            if detail.is_some() {
                payload.detail = detail;
            }
            if percent.is_some() {
                payload.percent = check_percent(percent)?;
            }
            // Keep the ttl chosen at start: it is expires_at - updated_at.
            let ttl = current
                .expires_at
                .map(|e| e.0.saturating_sub(current.updated_at.0).div_ceil(1000))
                .filter(|&secs| secs > 0)
                .unwrap_or(DEFAULT_PROGRESS_TTL_SECS);
            let input = progress_input(
                current.id,
                current.source,
                LifecycleState::Updated,
                payload,
                ttl,
            );
            submit(events, &input).await
        }
        ProgressCommand::Finish { id, status, detail } => {
            let (current, mut payload) = current_progress(events, &id).await?;
            let state = match status {
                Status::Success => {
                    payload.percent = Some(100.0);
                    LifecycleState::Ended
                }
                Status::Failure => LifecycleState::Failed,
            };
            if detail.is_some() {
                payload.detail = detail;
            }
            let input = progress_input(
                current.id,
                current.source,
                state,
                payload,
                DEFAULT_PROGRESS_TTL_SECS,
            );
            submit(events, &input).await
        }
    }
}

fn progress_input(
    id: EventId,
    source: Source,
    state: LifecycleState,
    payload: ProgressPayload,
    ttl_secs: u64,
) -> EventInput {
    EventInput {
        id,
        source,
        category: Category::Transfer,
        state,
        priority: None,
        persistent: false,
        ttl_ms: Some(secs_to_ms(ttl_secs)),
        group: None,
        payload: Payload::Progress(payload),
        actions: Vec::new(),
    }
}

/// The CLI is stateless, so updates start from what the daemon has.
async fn current_progress(events: &EventsProxy<'_>, id: &str) -> Result<(Event, ProgressPayload)> {
    let list: Vec<Event> = serde_json::from_str(&events.list().await.map_err(dbus_error)?)
        .map_err(|e| e.to_string())?;
    let event = list
        .into_iter()
        .find(|e| e.id.as_str() == id)
        .ok_or_else(|| {
            format!("no running task with id {id:?} (it may have finished or expired)")
        })?;
    if event.state.is_terminal() {
        return Err(format!("task {id:?} has already finished"));
    }
    match &event.payload {
        Payload::Progress(p) => {
            let payload = p.clone();
            Ok((event, payload))
        }
        _ => Err(format!("event {id:?} is not a progress task")),
    }
}

async fn submit(events: &EventsProxy<'_>, input: &EventInput) -> Result<()> {
    input.validate().map_err(|e| e.to_string())?;
    let json = serde_json::to_string(input).map_err(|e| e.to_string())?;
    events.submit(&json).await.map_err(dbus_error)
}

fn source(app: String) -> Source {
    Source {
        app,
        integration: INTEGRATION.into(),
    }
}

fn id_or_new(id: Option<String>, prefix: &str) -> Result<EventId> {
    let id =
        id.unwrap_or_else(|| format!("cli-{prefix}-{}-{}", std::process::id(), Timestamp::now().0));
    EventId::new(id).map_err(|e| e.to_string())
}

fn check_percent(percent: Option<f64>) -> Result<Option<f64>> {
    match percent {
        Some(p) if !(0.0..=100.0).contains(&p) => Err("--percent must be between 0 and 100".into()),
        other => Ok(other),
    }
}

fn secs_to_ms(secs: u64) -> u64 {
    secs.saturating_mul(1000)
}

fn dbus_error(e: zbus::Error) -> String {
    let text = e.to_string();
    // zbus reports this as a method or fdo error depending on the path taken.
    if text.contains("org.freedesktop.DBus.Error.ServiceUnknown")
        || text.contains("org.freedesktop.DBus.Error.NameHasNoOwner")
    {
        return "the Selawi Island daemon is not running. Start it with:\n  \
                systemctl --user start selawi-islandd"
            .into();
    }
    if let zbus::Error::MethodError(_, Some(message), _) = &e {
        return message.clone();
    }
    text
}
