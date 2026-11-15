//! Claude Code in the island: what each Claude Code hook means for the
//! bubble. Pure functions; main.rs reads the hook's input and talks to
//! the daemon. See docs/CLAUDE_CODE.md.
//!
//! Only what Claude Code reports is shown: the project folder, the kind of
//! tool and a file's name. Never a command line, a prompt or file contents.

use serde_json::Value;

pub const INTEGRATION: &str = "agents";
pub const APP: &str = "Claude Code";
const ID_PREFIX: &str = "agents:claude:";
/// A bubble that hears nothing for this long goes by itself (Claude Code
/// closed without saying, or a hook that never fired).
pub const WORKING_TTL_SECS: u64 = 15 * 60;
pub const WAITING_TTL_SECS: u64 = 60 * 60;
/// How long the island waits for your Allow / Deny. After that the buttons
/// go and Claude Code's own prompt decides. Must stay below the hook's
/// timeout in Claude Code's settings (agent_hooks.rs).
pub const ANSWER_WAIT_SECS: u64 = 110;
/// The longest request text shown.
const MAX_REQUEST: usize = 80;
const MAX_TEXT: usize = 60;

/// What a hook means for the session's bubble.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Step {
    /// Working, with what it is doing right now.
    Working(String),
    /// Stopped until you answer (a notification; the request itself came
    /// with PermissionRequest).
    Waiting(String),
    /// Claude Code asks approval for one tool call. `request` says what it
    /// wants; `answerable` is whether Allow / Deny can answer it (questions
    /// and plans need more than a yes).
    Approval { request: String, answerable: bool },
    /// The turn finished.
    Finished,
    /// The turn ended with an error.
    Failed(String),
    /// Nothing is running any more without a finished turn (the session
    /// closed, or you rejected a tool and Claude Code sits idle): remove an
    /// unfinished bubble.
    Closed,
    /// Nothing to show.
    Nothing,
}

/// The island's id for a Claude Code session, from its session id.
pub fn event_id(session_id: &str) -> Option<String> {
    let clean: String = session_id
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_' || *c == '.')
        .take(100)
        .collect();
    (!clean.is_empty()).then(|| format!("{ID_PREFIX}{clean}"))
}

/// The bubble's title: the project folder Claude Code works in.
pub fn project_title(cwd: Option<&str>) -> String {
    cwd.map(file_name)
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| APP.to_string())
}

/// The last part of a path, cleaned and shortened.
pub fn file_name(path: &str) -> String {
    let name = path.trim_end_matches('/').rsplit('/').next().unwrap_or("");
    clip(name)
}

fn clip(text: &str) -> String {
    text.chars()
        .filter(|c| !c.is_control())
        .take(MAX_TEXT)
        .collect::<String>()
        .trim()
        .to_string()
}

fn text<'a>(input: &'a Value, key: &str) -> Option<&'a str> {
    input.get(key).and_then(Value::as_str)
}

/// What a tool call is, in a few words. Commands, searches and web
/// addresses are never shown, only their kind.
pub fn describe_tool(name: &str, input: &Value) -> String {
    let file = || {
        text(input, "file_path")
            .or_else(|| text(input, "notebook_path"))
            .map(file_name)
            .filter(|f| !f.is_empty())
    };
    match name {
        "Edit" | "MultiEdit" | "NotebookEdit" => match file() {
            Some(f) => format!("Editing {f}"),
            None => "Editing a file".into(),
        },
        "Write" => match file() {
            Some(f) => format!("Writing {f}"),
            None => "Writing a file".into(),
        },
        "Read" => match file() {
            Some(f) => format!("Reading {f}"),
            None => "Reading a file".into(),
        },
        "Glob" | "Grep" | "LS" => "Searching the code".into(),
        "Bash" | "BashOutput" | "KillShell" => "Running a command".into(),
        "WebFetch" => "Reading a web page".into(),
        "WebSearch" => "Searching the web".into(),
        "Task" | "Agent" => "Working with a helper agent".into(),
        "AskUserQuestion" => "Asking you a question".into(),
        "ExitPlanMode" => "Presenting a plan".into(),
        "TodoWrite" => "Planning".into(),
        other => match other.strip_prefix("mcp__") {
            Some(rest) => {
                let server = clip(rest.split("__").next().unwrap_or(""));
                if server.is_empty() {
                    "Using a tool".into()
                } else {
                    format!("Using {server}")
                }
            }
            None => {
                let tool = clip(other);
                if tool.is_empty() {
                    "Working".into()
                } else {
                    format!("Using {tool}")
                }
            }
        },
    }
}

/// Exactly what a tool call asks approval for, in one line: the command,
/// the file, the site. Shown only while it waits for you, and only when
/// that is turned on (Settings → Integrations → Claude Code).
pub fn describe_request(name: &str, input: &Value, cwd: Option<&str>) -> String {
    let path = |key: &str| text(input, key).map(|p| shown_path(p, cwd));
    let line = match name {
        "Bash" => text(input, "command").map(|c| {
            let first = c
                .lines()
                .map(str::trim)
                .find(|l| !l.is_empty())
                .unwrap_or("");
            format!("Run: {first}")
        }),
        "Edit" | "MultiEdit" => path("file_path").map(|p| format!("Edit: {p}")),
        "Write" => path("file_path").map(|p| format!("Write: {p}")),
        "NotebookEdit" => path("notebook_path").map(|p| format!("Edit: {p}")),
        "Read" => path("file_path").map(|p| format!("Read: {p}")),
        "WebFetch" => text(input, "url").map(|u| format!("Fetch: {}", host_of(u))),
        "WebSearch" => text(input, "query").map(|q| format!("Search: {q}")),
        "AskUserQuestion" => input
            .get("questions")
            .and_then(Value::as_array)
            .and_then(|qs| qs.first())
            .and_then(|q| text(q, "question"))
            .map(str::to_string),
        "ExitPlanMode" => Some("Review the plan".into()),
        "Task" | "Agent" => Some("Start a helper agent".into()),
        other => other.strip_prefix("mcp__").map(|rest| {
            let mut parts = rest.splitn(2, "__");
            let server = parts.next().unwrap_or("");
            match parts.next() {
                Some(tool) => format!("{server}: {tool}"),
                None => server.to_string(),
            }
        }),
    };
    let line = clip_request(&line.unwrap_or_default());
    if !line.is_empty() {
        return line;
    }
    let tool = clip(name);
    if tool.is_empty() {
        "Needs your approval".into()
    } else {
        format!("Use {tool}")
    }
}

/// Whether Allow / Deny can answer the request. A question needs your
/// answer and a plan your review, so those stay in Claude Code.
pub fn can_answer(tool: &str) -> bool {
    !matches!(tool, "AskUserQuestion" | "ExitPlanMode")
}

/// What the hook prints to answer the request (Claude Code's
/// PermissionRequest decision).
pub fn decision_json(allow: bool) -> String {
    let decision = if allow {
        serde_json::json!({"behavior": "allow"})
    } else {
        serde_json::json!({"behavior": "deny", "message": "Denied from Selawi Island."})
    };
    serde_json::json!({
        "hookSpecificOutput": {"hookEventName": "PermissionRequest", "decision": decision}
    })
    .to_string()
}

/// A file inside the project is shown from the project folder
/// ("src/main.rs"), any other by its name.
fn shown_path(path: &str, cwd: Option<&str>) -> String {
    let inside = cwd
        .map(|c| c.trim_end_matches('/'))
        .filter(|c| !c.is_empty())
        .and_then(|c| path.strip_prefix(c))
        .and_then(|rest| rest.strip_prefix('/'));
    match inside {
        Some(rest) if !rest.is_empty() => rest.to_string(),
        _ => file_name(path),
    }
}

/// "https://docs.example.com/a/b" → "docs.example.com" (never a password
/// or the path).
fn host_of(url: &str) -> String {
    let rest = url.split_once("://").map_or(url, |(_, r)| r);
    let host = rest.split(['/', '?', '#']).next().unwrap_or("");
    host.rsplit('@').next().unwrap_or(host).to_string()
}

/// One line, at most MAX_REQUEST characters.
fn clip_request(text: &str) -> String {
    let clean: String = text
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect();
    let clean = clean.split_whitespace().collect::<Vec<_>>().join(" ");
    if clean.chars().count() > MAX_REQUEST {
        let cut: String = clean.chars().take(MAX_REQUEST - 1).collect();
        format!("{}…", cut.trim_end())
    } else {
        clean
    }
}

/// What one hook means for the bubble.
pub fn step_for(input: &Value) -> Step {
    match text(input, "hook_event_name").unwrap_or("") {
        "UserPromptSubmit" => Step::Working("Working".into()),
        "PreToolUse" | "PostToolUse" => {
            let tool = text(input, "tool_name").unwrap_or("");
            let args = input.get("tool_input").unwrap_or(&Value::Null);
            Step::Working(describe_tool(tool, args))
        }
        "PermissionRequest" => {
            let tool = text(input, "tool_name").unwrap_or("");
            let args = input.get("tool_input").unwrap_or(&Value::Null);
            Step::Approval {
                request: describe_request(tool, args, text(input, "cwd")),
                answerable: can_answer(tool),
            }
        }
        "Notification" => match text(input, "notification_type") {
            Some("permission_prompt") => Step::Waiting("Needs your approval".into()),
            Some("agent_needs_input" | "elicitation_dialog" | "elicitation_url_dialog") => {
                Step::Waiting("Needs your answer".into())
            }
            // About a minute after the turn stopped and you have not typed.
            // Rejecting a tool sends no hook of its own (and no Stop), so
            // this is when its bubble goes. After a normal turn the bubble
            // already finished and this changes nothing.
            Some("idle_prompt") => Step::Closed,
            _ => Step::Nothing,
        },
        "Stop" => Step::Finished,
        "StopFailure" => Step::Failed(failure_text(input)),
        "SessionEnd" => Step::Closed,
        _ => Step::Nothing,
    }
}

/// Why a turn stopped, from the error kind Claude Code reports.
fn failure_text(input: &Value) -> String {
    let kind = text(input, "error")
        .or_else(|| input.get("error").and_then(|e| text(e, "type")))
        .unwrap_or("");
    match kind {
        "rate_limit" => "Usage limit reached",
        "overloaded" | "server_error" => "Service unavailable",
        "authentication_failed" => "Sign-in needed",
        "billing_error" => "Billing problem",
        _ => "Stopped by an error",
    }
    .into()
}

/// Names the Claude app is installed under in the app grid.
const CLAUDE_APP_NAMES: [&str; 2] = ["Claude", "Claude Desktop"];

/// Whether a desktop file's name is worth reading: it mentions Claude.
pub fn may_be_claude_app(file_name: &str) -> bool {
    file_name.ends_with(".desktop") && file_name.to_ascii_lowercase().contains("claude")
}

/// Whether a desktop entry is the Claude app: an application named
/// "Claude" that is shown in the app grid.
pub fn is_claude_app(desktop_file: &str) -> bool {
    let mut in_entry = false;
    let (mut name, mut application, mut hidden) = (None, false, false);
    for line in desktop_file.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_entry = line == "[Desktop Entry]";
            continue;
        }
        if !in_entry {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        match (key.trim(), value.trim()) {
            ("Name", value) => name = Some(value.to_string()),
            ("Type", value) => application = value == "Application",
            ("NoDisplay" | "Hidden", "true") => hidden = true,
            _ => {}
        }
    }
    application && !hidden && name.is_some_and(|n| CLAUDE_APP_NAMES.contains(&n.as_str()))
}

/// A desktop file's id, as the island accepts it ("claude-desktop" from
/// "claude-desktop.desktop").
pub fn desktop_id(file_name: &str) -> Option<String> {
    let id = file_name.strip_suffix(".desktop")?;
    let ok = !id.is_empty()
        && id.len() <= 120
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'));
    ok.then(|| id.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn ids_from_session_ids() {
        assert_eq!(
            event_id("1f2e-3d4c").as_deref(),
            Some("agents:claude:1f2e-3d4c")
        );
        assert_eq!(event_id("a/b c:d").as_deref(), Some("agents:claude:abcd"));
        assert_eq!(event_id("::"), None);
        assert!(event_id(&"x".repeat(500)).is_some_and(|id| id.len() <= 128));
    }

    #[test]
    fn titles_and_file_names() {
        assert_eq!(
            project_title(Some("/home/user/code/Selawi-Island")),
            "Selawi-Island"
        );
        assert_eq!(project_title(Some("/home/user/code/app/")), "app");
        assert_eq!(project_title(None), "Claude Code");
        assert_eq!(project_title(Some("/")), "Claude Code");
        assert_eq!(file_name("src/main.rs"), "main.rs");
        assert_eq!(file_name("bad\u{7}name.txt"), "badname.txt");
        assert_eq!(file_name(&"a".repeat(200)).chars().count(), 60);
    }

    #[test]
    fn tools_in_a_few_words_without_secrets() {
        let edit = json!({"file_path": "/home/user/app/src/lib.rs", "old_string": "x"});
        assert_eq!(describe_tool("Edit", &edit), "Editing lib.rs");
        assert_eq!(describe_tool("Write", &edit), "Writing lib.rs");
        assert_eq!(describe_tool("Read", &edit), "Reading lib.rs");
        assert_eq!(describe_tool("Edit", &json!({})), "Editing a file");
        let bash = json!({"command": "curl -H 'Authorization: secret' x", "description": "Fetch"});
        assert_eq!(describe_tool("Bash", &bash), "Running a command");
        assert_eq!(
            describe_tool("Grep", &json!({"pattern": "password"})),
            "Searching the code"
        );
        assert_eq!(
            describe_tool("WebFetch", &json!({"url": "https://example.com/private"})),
            "Reading a web page"
        );
        assert_eq!(
            describe_tool("mcp__github__create_issue", &json!({})),
            "Using github"
        );
        assert_eq!(
            describe_tool("SomethingNew", &json!({})),
            "Using SomethingNew"
        );
    }

    #[test]
    fn what_each_hook_means() {
        let step = |v: Value| step_for(&v);
        assert_eq!(
            step(json!({"hook_event_name": "UserPromptSubmit", "prompt": "my secret plan"})),
            Step::Working("Working".into())
        );
        assert_eq!(
            step(json!({"hook_event_name": "PreToolUse", "tool_name": "Edit",
                        "tool_input": {"file_path": "README.md"}})),
            Step::Working("Editing README.md".into())
        );
        assert_eq!(
            step(
                json!({"hook_event_name": "PermissionRequest", "tool_name": "Bash",
                        "tool_input": {"command": "npm test"}})
            ),
            Step::Approval {
                request: "Run: npm test".into(),
                answerable: true
            }
        );
        assert_eq!(
            step(
                json!({"hook_event_name": "PermissionRequest", "tool_name": "AskUserQuestion",
                        "tool_input": {"questions": [{"question": "Which database?"}]}})
            ),
            Step::Approval {
                request: "Which database?".into(),
                answerable: false
            }
        );
        assert_eq!(
            step(
                json!({"hook_event_name": "Notification", "notification_type": "permission_prompt"})
            ),
            Step::Waiting("Needs your approval".into())
        );
        assert_eq!(
            step(json!({"hook_event_name": "Notification", "notification_type": "idle_prompt"})),
            Step::Closed,
            "idle with no finished turn (a rejected tool): the bubble goes"
        );
        assert_eq!(
            step(json!({"hook_event_name": "Notification", "notification_type": "auth_success"})),
            Step::Nothing
        );
        assert_eq!(step(json!({"hook_event_name": "Stop"})), Step::Finished);
        assert_eq!(
            step(json!({"hook_event_name": "StopFailure", "error": "rate_limit"})),
            Step::Failed("Usage limit reached".into())
        );
        assert_eq!(
            step(json!({"hook_event_name": "StopFailure"})),
            Step::Failed("Stopped by an error".into())
        );
        assert_eq!(step(json!({"hook_event_name": "SessionEnd"})), Step::Closed);
        assert_eq!(
            step(json!({"hook_event_name": "SessionStart"})),
            Step::Nothing
        );
        assert_eq!(step(json!("not an object")), Step::Nothing);
    }

    #[test]
    fn finds_the_claude_app_in_the_app_grid() {
        let claude =
            "[Desktop Entry]\nType=Application\nName=Claude\nIcon=claude\nExec=claude-desktop %u\n";
        assert!(is_claude_app(claude));
        assert!(is_claude_app(
            &claude.replace("Name=Claude", "Name=Claude Desktop")
        ));
        assert!(
            !is_claude_app(&format!("{claude}NoDisplay=true\n")),
            "hidden from the app grid"
        );
        assert!(!is_claude_app(
            &claude.replace("Name=Claude", "Name=Claude Code Helper")
        ));
        assert!(!is_claude_app(
            &claude.replace("Type=Application", "Type=Link")
        ));
        assert!(
            !is_claude_app("[Desktop Action new]\nName=Claude\n[Desktop Entry]\nType=Application\nName=Notes\n"),
            "only the main entry's name counts"
        );
        assert!(may_be_claude_app("claude-desktop.desktop"));
        assert!(may_be_claude_app("com.anthropic.Claude.desktop"));
        assert!(!may_be_claude_app("org.gnome.Nautilus.desktop"));
        assert!(!may_be_claude_app("claude-notes.txt"));
        assert_eq!(
            desktop_id("claude-desktop.desktop").as_deref(),
            Some("claude-desktop")
        );
        assert_eq!(
            desktop_id("com.anthropic.Claude.desktop").as_deref(),
            Some("com.anthropic.Claude")
        );
        assert_eq!(desktop_id("bad name.desktop"), None);
        assert_eq!(desktop_id("claude"), None);
    }

    #[test]
    fn what_a_request_asks_for() {
        let cwd = Some("/home/user/app");
        let req = |name: &str, v: Value| describe_request(name, &v, cwd);
        assert_eq!(req("Bash", json!({"command": "npm test"})), "Run: npm test");
        assert_eq!(
            req("Bash", json!({"command": "\n  cd x &&\n make"})),
            "Run: cd x &&"
        );
        assert_eq!(
            req(
                "Edit",
                json!({"file_path": "/home/user/app/src/api/route.ts"})
            ),
            "Edit: src/api/route.ts"
        );
        assert_eq!(
            req("Write", json!({"file_path": "/etc/hosts"})),
            "Write: hosts",
            "outside the project: the name only"
        );
        assert_eq!(
            req(
                "WebFetch",
                json!({"url": "https://me:pw@docs.example.com/a?b=1"})
            ),
            "Fetch: docs.example.com"
        );
        assert_eq!(
            req("WebSearch", json!({"query": "rust zbus"})),
            "Search: rust zbus"
        );
        assert_eq!(
            req("mcp__github__create_issue", json!({})),
            "github: create_issue"
        );
        assert_eq!(req("ExitPlanMode", json!({})), "Review the plan");
        assert_eq!(req("Mystery", json!({})), "Use Mystery");
        assert_eq!(req("", json!({})), "Needs your approval");
        let long = req("Bash", json!({"command": "x".repeat(300)}));
        assert_eq!(long.chars().count(), 80);
        assert!(long.ends_with('…'));
        assert!(!req("Bash", json!({"command": "a\u{7}b"})).contains('\u{7}'));
    }

    #[test]
    fn answers_claude_code_understands() {
        assert!(can_answer("Bash") && can_answer("Edit") && can_answer("mcp__x__y"));
        assert!(!can_answer("AskUserQuestion") && !can_answer("ExitPlanMode"));
        let allow: Value = serde_json::from_str(&decision_json(true)).unwrap_or_default();
        assert_eq!(
            allow["hookSpecificOutput"]["hookEventName"],
            "PermissionRequest"
        );
        assert_eq!(allow["hookSpecificOutput"]["decision"]["behavior"], "allow");
        let deny: Value = serde_json::from_str(&decision_json(false)).unwrap_or_default();
        assert_eq!(deny["hookSpecificOutput"]["decision"]["behavior"], "deny");
        assert!(deny["hookSpecificOutput"]["decision"]["message"].is_string());
    }
}
