//! Adds and removes the island's own hooks in Claude Code's settings
//! (`~/.claude/settings.json`). Pure functions on the parsed file; main.rs
//! reads, backs up and writes it.
//!
//! Only entries whose command runs `selawi-island agent-hook` are ever
//! touched. Everything else in the file stays exactly as it was.

use serde_json::{json, Map, Value};

/// Claude Code hook events the island listens to, and whether the event
/// takes a matcher (tool events).
pub const EVENTS: [(&str, bool); 8] = [
    ("UserPromptSubmit", false),
    ("PreToolUse", true),
    ("PostToolUse", true),
    ("PermissionRequest", true),
    ("Notification", false),
    ("Stop", false),
    ("StopFailure", false),
    ("SessionEnd", false),
];

/// Seconds Claude Code waits for the hook at most. It normally takes a
/// few milliseconds.
const TIMEOUT_SECS: u64 = 5;

/// The command Claude Code runs. It never fails: if the island is gone
/// (for example uninstalled), the hook quietly does nothing.
pub fn hook_command(program: &str) -> String {
    let quoted = program.replace('\'', r"'\''");
    format!("'{quoted}' agent-hook claude 2>/dev/null || true")
}

/// Whether a hook command is one of the island's.
pub fn is_ours(command: &str) -> bool {
    command.contains("selawi-island") && command.contains(" agent-hook ")
}

/// Reads the settings file's text. An empty file counts as `{}`.
pub fn parse_settings(text: &str) -> Result<Value, String> {
    if text.trim().is_empty() {
        return Ok(Value::Object(Map::new()));
    }
    let value: Value = serde_json::from_str(text)
        .map_err(|e| format!("it is not valid JSON ({e}); nothing was changed"))?;
    if !value.is_object() {
        return Err("it is not a JSON object; nothing was changed".into());
    }
    Ok(value)
}

/// Adds (or refreshes) the island's hooks, calling `command`. Returns
/// whether anything changed.
pub fn install(settings: &mut Value, command: &str) -> Result<bool, String> {
    let before = settings.clone();
    remove_ours_completely(settings)?;
    let root = settings
        .as_object_mut()
        .ok_or("it is not a JSON object; nothing was changed")?;
    let hooks = root
        .entry("hooks")
        .or_insert_with(|| Value::Object(Map::new()));
    let Some(events) = hooks.as_object_mut() else {
        return Err("its \"hooks\" entry is not an object; nothing was changed".into());
    };
    for (name, has_matcher) in EVENTS {
        let groups = events
            .entry(name)
            .or_insert_with(|| Value::Array(Vec::new()));
        let Some(groups) = groups.as_array_mut() else {
            return Err(format!(
                "its \"{name}\" hooks are not a list; nothing was changed"
            ));
        };
        let handler = json!({"type": "command", "command": command, "timeout": TIMEOUT_SECS});
        let mut group = Map::new();
        if has_matcher {
            group.insert("matcher".into(), json!("*"));
        }
        group.insert("hooks".into(), json!([handler]));
        groups.push(Value::Object(group));
    }
    Ok(*settings != before)
}

/// Removes the island's hooks, and the groups and event lists that only
/// held them, so nothing empty is left behind.
fn remove_ours_completely(settings: &mut Value) -> Result<bool, String> {
    let root = settings
        .as_object_mut()
        .ok_or("it is not a JSON object; nothing was changed")?;
    let Some(events) = root.get_mut("hooks").and_then(Value::as_object_mut) else {
        return Ok(false);
    };
    let mut changed = false;
    let names: Vec<String> = events.keys().cloned().collect();
    for name in names {
        let Some(groups) = events.get_mut(&name).and_then(Value::as_array_mut) else {
            continue;
        };
        let mut emptied_here = false;
        groups.retain_mut(|group| {
            let Some(handlers) = group.get_mut("hooks").and_then(Value::as_array_mut) else {
                return true;
            };
            let count = handlers.len();
            handlers.retain(|h| {
                !h.get("command")
                    .and_then(Value::as_str)
                    .is_some_and(is_ours)
            });
            if handlers.len() == count {
                return true;
            }
            changed = true;
            emptied_here = true;
            !handlers.is_empty()
        });
        if emptied_here && groups.is_empty() {
            events.remove(&name);
        }
    }
    if changed && events.is_empty() {
        root.remove("hooks");
    }
    Ok(changed)
}

/// Removes the island's hooks and anything left empty by that.
pub fn uninstall(settings: &mut Value) -> Result<bool, String> {
    remove_ours_completely(settings)
}

/// Whether the settings hold the island's hooks.
pub fn installed(settings: &Value) -> bool {
    settings
        .get("hooks")
        .and_then(Value::as_object)
        .is_some_and(|events| {
            events
                .values()
                .filter_map(Value::as_array)
                .flatten()
                .any(|group| {
                    group
                        .get("hooks")
                        .and_then(Value::as_array)
                        .is_some_and(|handlers| {
                            handlers.iter().any(|h| {
                                h.get("command")
                                    .and_then(Value::as_str)
                                    .is_some_and(is_ours)
                            })
                        })
                })
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    const CMD: &str = "'/usr/bin/selawi-island' agent-hook claude 2>/dev/null || true";

    #[test]
    fn the_command_never_fails_and_is_recognised() {
        assert_eq!(hook_command("/usr/bin/selawi-island"), CMD);
        assert!(is_ours(CMD));
        assert!(is_ours(&hook_command(
            "/home/user/.local/bin/selawi-island"
        )));
        assert!(!is_ours("/usr/local/bin/my-own-hook.sh"));
        assert_eq!(
            hook_command("/odd/it's/selawi-island"),
            r"'/odd/it'\''s/selawi-island' agent-hook claude 2>/dev/null || true"
        );
    }

    #[test]
    fn adds_to_an_empty_or_missing_file() -> Result<(), String> {
        let mut settings = parse_settings("")?;
        assert!(install(&mut settings, CMD)?);
        assert!(installed(&settings));
        let events = settings["hooks"].as_object().ok_or("no hooks")?;
        assert_eq!(events.len(), EVENTS.len());
        assert_eq!(settings["hooks"]["PreToolUse"][0]["matcher"], "*");
        assert!(settings["hooks"]["Stop"][0].get("matcher").is_none());
        assert_eq!(settings["hooks"]["Stop"][0]["hooks"][0]["command"], CMD);
        assert_eq!(settings["hooks"]["Stop"][0]["hooks"][0]["timeout"], 5);
        Ok(())
    }

    #[test]
    fn keeps_everything_else_and_its_order() -> Result<(), String> {
        let text = r#"{
  "model": "opus",
  "permissions": {"allow": ["Bash(ls:*)"]},
  "hooks": {
    "PreToolUse": [{"matcher": "Bash", "hooks": [{"type": "command", "command": "~/guard.sh"}]}],
    "SessionStart": [{"hooks": [{"type": "command", "command": "echo hi"}]}]
  },
  "env": {"A": "1"}
}"#;
        let original = parse_settings(text)?;
        let mut settings = original.clone();
        assert!(install(&mut settings, CMD)?);
        let keys: Vec<&String> = settings.as_object().ok_or("object")?.keys().collect();
        assert_eq!(
            keys,
            ["model", "permissions", "hooks", "env"],
            "key order kept"
        );
        assert_eq!(
            settings["hooks"]["PreToolUse"][0],
            original["hooks"]["PreToolUse"][0]
        );
        assert_eq!(
            settings["hooks"]["PreToolUse"].as_array().map(Vec::len),
            Some(2)
        );
        assert_eq!(
            settings["hooks"]["SessionStart"],
            original["hooks"]["SessionStart"]
        );

        // Running it again changes nothing.
        let once = settings.clone();
        assert!(!install(&mut settings, CMD)?);
        assert_eq!(settings, once);

        // Removing gives back exactly the original.
        assert!(uninstall(&mut settings)?);
        assert_eq!(settings, original);
        assert!(!uninstall(&mut settings)?, "nothing left to remove");
        Ok(())
    }

    #[test]
    fn a_moved_program_is_replaced_not_duplicated() -> Result<(), String> {
        let mut settings = parse_settings("{}")?;
        install(
            &mut settings,
            &hook_command("/home/user/.local/bin/selawi-island"),
        )?;
        assert!(install(&mut settings, CMD)?);
        let stop = settings["hooks"]["Stop"].as_array().ok_or("stop")?;
        assert_eq!(stop.len(), 1);
        assert_eq!(stop[0]["hooks"][0]["command"], CMD);
        Ok(())
    }

    #[test]
    fn a_group_shared_with_another_hook_keeps_that_hook() -> Result<(), String> {
        let text = format!(
            r#"{{"hooks":{{"Stop":[{{"hooks":[{{"type":"command","command":"notify-send done"}},{{"type":"command","command":"{}"}}]}}]}}}}"#,
            CMD.replace('"', "\\\"")
        );
        let mut settings = parse_settings(&text)?;
        assert!(uninstall(&mut settings)?);
        assert_eq!(
            settings["hooks"]["Stop"][0]["hooks"],
            serde_json::json!([{"type": "command", "command": "notify-send done"}])
        );
        Ok(())
    }

    #[test]
    fn refuses_files_it_does_not_understand() {
        assert!(parse_settings("{ not json").is_err());
        assert!(parse_settings("[1, 2]").is_err());
        let Ok(mut odd) = parse_settings(r#"{"hooks": "nope"}"#) else {
            panic!("parses");
        };
        assert!(install(&mut odd, CMD).is_err());
        let Ok(mut odd) = parse_settings(r#"{"hooks": {"Stop": "nope"}}"#) else {
            panic!("parses");
        };
        assert!(install(&mut odd, CMD).is_err());
    }
}
