//! selawi-island: send events to the Selawi Island daemon from a terminal
//! or script. See docs/CLI.md.

mod pacman;
mod run;

use std::io::Read;
use std::os::unix::fs::MetadataExt;
use std::os::unix::process::{CommandExt, ExitStatusExt};
use std::path::Path;
use std::process::{Child, ExitCode, Stdio};
use std::time::{Duration, Instant};

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
    /// Run a command and show it in the island: running with a timer, then
    /// Done or Failed. Example: selawi-island run -- cargo build
    Run {
        /// Title to show instead of the command's name.
        #[arg(long)]
        title: Option<String>,
        /// The command and its arguments (put `--` before it).
        #[arg(
            required = true,
            trailing_var_arg = true,
            allow_hyphen_values = true,
            value_name = "COMMAND"
        )]
        command: Vec<String>,
    },
    /// Used by the pacman hooks (runs as root).
    #[command(hide = true)]
    PacmanHook {
        #[arg(value_enum)]
        phase: HookPhase,
        #[arg(value_enum)]
        operation: HookOperation,
        /// Deliver to this process's own session bus (tests).
        #[arg(long, hide = true)]
        this_session: bool,
    },
    /// Used by pacman-hook, as the logged-in user.
    #[command(hide = true)]
    PacmanUser {
        #[arg(value_enum)]
        step: UserStep,
        #[arg(long)]
        title: Option<String>,
        #[arg(long)]
        detail: Option<String>,
    },
}

#[derive(Clone, Copy, PartialEq, Eq, ValueEnum)]
enum HookPhase {
    Pre,
    Post,
}

#[derive(Clone, Copy, ValueEnum)]
enum HookOperation {
    Install,
    Remove,
}

#[derive(Clone, Copy, ValueEnum)]
enum UserStep {
    Start,
    Finish,
    Watch,
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
    match cli.command {
        Command::Run { title, command } => return run_command(title, command),
        // Never fails: an install must not depend on the island.
        Command::PacmanHook {
            phase,
            operation,
            this_session,
        } => {
            pacman_hook(phase, operation, this_session);
            return ExitCode::SUCCESS;
        }
        Command::PacmanUser {
            step,
            title,
            detail,
        } => {
            pacman_user(step, title, detail);
            return ExitCode::SUCCESS;
        }
        _ => {}
    }
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
        Command::Run { .. } | Command::PacmanHook { .. } | Command::PacmanUser { .. } => {
            Err("handled before connecting".into())
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
        icon: None,
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

// ---- selawi-island run ------------------------------------------------------

/// Runs are long; one that is never finished (terminal closed) goes after this.
const RUN_TTL_SECS: u64 = 12 * 60 * 60;

fn run_command(title: Option<String>, command: Vec<String>) -> ExitCode {
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
    let Some((program, args)) = command.split_first() else {
        return ExitCode::FAILURE;
    };
    let title = title.unwrap_or_else(|| run::command_title(&command));
    let started = Instant::now();
    let mut child = match std::process::Command::new(program).args(args).spawn() {
        Ok(child) => child,
        Err(e) => {
            eprintln!("selawi-island: can't run {program}: {e}");
            return ExitCode::from(127);
        }
    };

    runtime.block_on(async move {
        let island = Island::connect().await;
        let id = id_or_new(None, "run").ok();
        let input = |state, detail: String, percent| {
            id.clone().map(|id| EventInput {
                id,
                source: source(DEFAULT_APP.into()),
                category: Category::Custom,
                state,
                priority: None,
                persistent: false,
                ttl_ms: Some(secs_to_ms(RUN_TTL_SECS)),
                group: None,
                payload: Payload::Progress(ProgressPayload {
                    title: title.clone(),
                    detail: Some(detail),
                    percent,
                }),
                actions: Vec::new(),
            })
        };
        island
            .send(input(LifecycleState::Started, "Running".into(), None))
            .await;

        // Ctrl+C reaches the command itself; keep waiting so its end is shown.
        let wait = tokio::task::spawn_blocking(move || child.wait());
        tokio::pin!(wait);
        let status = loop {
            tokio::select! {
                status = &mut wait => break status,
                _ = tokio::signal::ctrl_c() => {}
            }
        };
        let outcome = match status {
            Ok(Ok(status)) => match (status.code(), status.signal()) {
                (Some(code), _) => run::Outcome::Exited(code),
                (None, Some(signal)) => run::Outcome::Signal(signal),
                (None, None) => run::Outcome::Exited(1),
            },
            _ => run::Outcome::Exited(1),
        };
        let (state, percent) = if outcome.success() {
            (LifecycleState::Ended, Some(100.0))
        } else {
            (LifecycleState::Failed, None)
        };
        island
            .send(input(state, outcome.detail(started.elapsed()), percent))
            .await;
        ExitCode::from(outcome.exit_code())
    })
}

/// The daemon, if it is running. Sending never fails the caller: a run
/// or an install works the same without the island.
struct Island {
    events: Option<EventsProxy<'static>>,
}

impl Island {
    async fn connect() -> Island {
        let events = async {
            let connection = zbus::Connection::session().await.ok()?;
            let bus = zbus::fdo::DBusProxy::new(&connection).await.ok()?;
            let name = zbus::names::BusName::try_from("com.selawi.Island").ok()?;
            // Do not start the daemon just for this.
            if !bus.name_has_owner(name).await.ok()? {
                return None;
            }
            EventsProxy::new(&connection).await.ok()
        }
        .await;
        Island { events }
    }

    async fn send(&self, input: Option<EventInput>) {
        if let (Some(events), Some(input)) = (&self.events, input) {
            let _ = submit(events, &input).await;
        }
    }

    async fn current(&self, id: &str) -> Option<Event> {
        let events = self.events.as_ref()?;
        let list: Vec<Event> = serde_json::from_str(&events.list().await.ok()?).ok()?;
        list.into_iter()
            .find(|e| e.id.as_str() == id && !e.state.is_terminal())
    }
}

// ---- pacman hooks -------------------------------------------------------------

/// Packages being installed or removed take at most this long to show.
const PACMAN_TTL_SECS: u64 = 6 * 60 * 60;
const DELIVER_WAIT: Duration = Duration::from_millis(1500);

fn pacman_hook(phase: HookPhase, operation: HookOperation, this_session: bool) {
    let mut args: Vec<String> = Vec::new();
    if phase == HookPhase::Pre {
        let mut stdin = String::new();
        if std::io::stdin().read_to_string(&mut stdin).is_err() {
            return;
        }
        let targets = pacman::clean_targets(&stdin);
        if targets.is_empty() {
            return;
        }
        let installed = std::fs::read_dir(pacman::LOCAL_DB).ok().map(|dir| {
            let entries: Vec<String> = dir
                .filter_map(|e| e.ok()?.file_name().into_string().ok())
                .collect();
            pacman::installed_names(entries.iter().map(String::as_str))
        });
        let operation = match operation {
            HookOperation::Install => pacman::Operation::Install,
            HookOperation::Remove => pacman::Operation::Remove,
        };
        let (title, detail) = pacman::summary(operation, &targets, installed.as_ref());
        args.extend([
            "pacman-user".into(),
            "start".into(),
            "--title".into(),
            title,
        ]);
        if let Some(detail) = detail {
            args.extend(["--detail".into(), detail]);
        }
    } else {
        args.extend(["pacman-user".into(), "finish".into()]);
    }
    let Ok(program) = std::env::current_exe() else {
        return;
    };
    let sessions = if this_session {
        vec![Session { uid: None, gid: 0 }]
    } else {
        user_sessions()
    };
    for session in sessions {
        if let Some(mut child) = as_user(&program, &args, session) {
            wait_briefly(&mut child);
        }
        if phase == HookPhase::Pre {
            // Notices a transaction that stops without finishing. Detached:
            // pacman does not wait for it.
            let watch = ["pacman-user".to_string(), "watch".to_string()];
            let _ = as_user(&program, &watch, session);
        }
    }
}

/// A logged-in user's session bus. None for `uid` means "this process's own
/// bus" (when not running as root, e.g. in tests).
#[derive(Clone, Copy)]
struct Session {
    uid: Option<u32>,
    gid: u32,
}

fn running_as_root() -> bool {
    std::fs::metadata("/proc/self").is_ok_and(|m| m.uid() == 0)
}

fn user_sessions() -> Vec<Session> {
    if !running_as_root() {
        return vec![Session { uid: None, gid: 0 }];
    }
    let Ok(dir) = std::fs::read_dir("/run/user") else {
        return Vec::new();
    };
    dir.filter_map(|entry| {
        let entry = entry.ok()?;
        let uid: u32 = entry.file_name().to_str()?.parse().ok()?;
        let home = entry.metadata().ok()?;
        let bus = std::fs::metadata(entry.path().join("bus")).ok()?;
        (uid >= pacman::FIRST_USER_UID && home.uid() == uid && bus.uid() == uid).then_some(
            Session {
                uid: Some(uid),
                gid: home.gid(),
            },
        )
    })
    .collect()
}

/// Starts this program as the session's user, with nothing but the path to
/// that user's bus in its environment.
fn as_user(program: &Path, args: &[String], session: Session) -> Option<Child> {
    let mut command = std::process::Command::new(program);
    command
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .process_group(0);
    if let Some(uid) = session.uid {
        command
            .env_clear()
            .env(
                "DBUS_SESSION_BUS_ADDRESS",
                format!("unix:path=/run/user/{uid}/bus"),
            )
            .current_dir("/")
            .gid(session.gid)
            .uid(uid);
    }
    command.spawn().ok()
}

fn wait_briefly(child: &mut Child) {
    let until = Instant::now() + DELIVER_WAIT;
    while Instant::now() < until {
        if let Ok(Some(_)) = child.try_wait() {
            return;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    let _ = child.kill();
    let _ = child.wait();
}

fn pacman_user(step: UserStep, title: Option<String>, detail: Option<String>) {
    let Ok(runtime) = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    else {
        return;
    };
    match step {
        UserStep::Start => runtime.block_on(async {
            let island = Island::connect().await;
            let title = title.unwrap_or_else(|| "Updating packages".into());
            island
                .send(pacman_input(LifecycleState::Started, title, detail, None))
                .await;
        }),
        UserStep::Finish => runtime.block_on(async {
            let island = Island::connect().await;
            if let Some((title, detail)) = island
                .current(pacman::EVENT_ID)
                .await
                .and_then(progress_text)
            {
                let input = pacman_input(
                    LifecycleState::Ended,
                    pacman::finished_title(&title),
                    detail,
                    Some(100.0),
                );
                island.send(input).await;
            }
        }),
        UserStep::Watch => pacman_watch(&runtime),
    }
}

/// Waits for pacman to let go of its database lock. If the transaction
/// it saw starting has not finished by then, it stopped: say so.
fn pacman_watch(runtime: &tokio::runtime::Runtime) {
    let lock = Path::new(pacman::LOCK_FILE);
    if !lock.exists() {
        return;
    }
    // Which event this watch is about (another transaction may follow).
    let mut watched = None;
    for _ in 0..20 {
        watched =
            runtime.block_on(async { Island::connect().await.current(pacman::EVENT_ID).await });
        if watched.is_some() {
            break;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    let Some(watched) = watched else {
        return;
    };
    let give_up = Instant::now() + Duration::from_secs(PACMAN_TTL_SECS);
    while lock.exists() {
        if Instant::now() > give_up {
            return;
        }
        std::thread::sleep(Duration::from_secs(1));
    }
    std::thread::sleep(Duration::from_secs(2));
    runtime.block_on(async {
        let island = Island::connect().await;
        let Some(current) = island.current(pacman::EVENT_ID).await else {
            return;
        };
        if current.created_at != watched.created_at {
            return;
        }
        if let Some((title, _)) = progress_text(current) {
            let input = pacman_input(
                LifecycleState::Failed,
                title,
                Some("Did not finish".into()),
                None,
            );
            island.send(input).await;
        }
    });
}

fn progress_text(event: Event) -> Option<(String, Option<String>)> {
    match event.payload {
        Payload::Progress(p) => Some((p.title, p.detail)),
        _ => None,
    }
}

fn pacman_input(
    state: LifecycleState,
    title: String,
    detail: Option<String>,
    percent: Option<f64>,
) -> Option<EventInput> {
    Some(EventInput {
        id: EventId::new(pacman::EVENT_ID).ok()?,
        source: Source {
            app: pacman::APP.into(),
            integration: pacman::INTEGRATION.into(),
            icon: None,
        },
        category: Category::System,
        state,
        priority: None,
        persistent: false,
        ttl_ms: Some(secs_to_ms(PACMAN_TTL_SECS)),
        group: None,
        payload: Payload::Progress(ProgressPayload {
            title,
            detail,
            percent,
        }),
        actions: Vec::new(),
    })
}
