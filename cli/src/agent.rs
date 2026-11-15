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
const MAX_TEXT: usize = 60;

/// What a hook means for the session's bubble.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Step {
    /// Working, with what it is doing right now.
    Working(String),
    /// Stopped until you answer.
    Waiting(String),
    /// The turn finished.
    Finished,
    /// The turn ended with an error.
    Failed(String),
    /// The session closed: remove an unfinished bubble.
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

/// What one hook means for the bubble.
pub fn step_for(input: &Value) -> Step {
    match text(input, "hook_event_name").unwrap_or("") {
        "UserPromptSubmit" => Step::Working("Working".into()),
        "PreToolUse" | "PostToolUse" => {
            let tool = text(input, "tool_name").unwrap_or("");
            let args = input.get("tool_input").unwrap_or(&Value::Null);
            Step::Working(describe_tool(tool, args))
        }
        "PermissionRequest" => Step::Waiting("Needs your approval".into()),
        "Notification" => match text(input, "notification_type") {
            Some("permission_prompt") => Step::Waiting("Needs your approval".into()),
            Some("agent_needs_input" | "elicitation_dialog" | "elicitation_url_dialog") => {
                Step::Waiting("Needs your answer".into())
            }
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
            step(json!({"hook_event_name": "PermissionRequest", "tool_name": "Bash"})),
            Step::Waiting("Needs your approval".into())
        );
        assert_eq!(
            step(
                json!({"hook_event_name": "Notification", "notification_type": "permission_prompt"})
            ),
            Step::Waiting("Needs your approval".into())
        );
        assert_eq!(
            step(json!({"hook_event_name": "Notification", "notification_type": "idle_prompt"})),
            Step::Nothing,
            "idle after a finished turn is not news"
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
}
