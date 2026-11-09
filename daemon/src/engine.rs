//! Owns the event store and its optional on-disk snapshot.
//!
//! Everything here is synchronous and free of D-Bus so it can be tested
//! directly; `dbus.rs` wraps it.

use std::fs;
use std::io::Write;
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
use std::path::{Path, PathBuf};

use selawi_core::events::EventId;
use selawi_core::{parse_input, EngineConfig, EngineError, Event, EventStore, Timestamp};

/// Largest event JSON accepted over D-Bus.
pub const MAX_INPUT_BYTES: usize = 64 * 1024;
/// How long restored events wait for their source before expiring.
pub const RESTORE_GRACE_MS: u64 = 60_000;

#[derive(Debug)]
pub enum SubmitError {
    TooLarge,
    Rejected(EngineError),
}

impl std::fmt::Display for SubmitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TooLarge => write!(f, "event is larger than {MAX_INPUT_BYTES} bytes"),
            Self::Rejected(e) => write!(f, "{e}"),
        }
    }
}

pub struct Engine {
    store: EventStore,
    snapshot_path: Option<PathBuf>,
    last_saved: Option<String>,
}

impl Engine {
    /// Loads the snapshot at `snapshot_path` if there is one. A missing or
    /// unreadable snapshot starts empty; it is never fatal.
    pub fn new(snapshot_path: Option<PathBuf>, now: Timestamp) -> Self {
        let config = EngineConfig::default();
        let store = match snapshot_path.as_deref().map(load_snapshot) {
            Some(Ok(events)) => EventStore::restore(config, events, now, RESTORE_GRACE_MS),
            Some(Err(e)) => {
                eprintln!("Ignoring unreadable event snapshot: {e}");
                EventStore::new(config)
            }
            None => EventStore::new(config),
        };
        let mut engine = Self {
            store,
            snapshot_path,
            last_saved: None,
        };
        engine.save_if_changed();
        engine
    }

    /// Returns whether the visible list changed.
    pub fn submit(&mut self, json: &str, now: Timestamp) -> Result<bool, SubmitError> {
        if json.len() > MAX_INPUT_BYTES {
            return Err(SubmitError::TooLarge);
        }
        let input = parse_input(json).map_err(SubmitError::Rejected)?;
        self.store.tick(now);
        let changes = self
            .store
            .apply(input, now)
            .map_err(SubmitError::Rejected)?;
        self.save_if_changed();
        Ok(!changes.is_empty())
    }

    pub fn dismiss(&mut self, id: &str) -> bool {
        let Ok(id) = EventId::new(id) else {
            return false;
        };
        let removed = self.store.dismiss(&id).is_some();
        if removed {
            self.save_if_changed();
        }
        removed
    }

    pub fn has_action(&self, id: &str, action_id: &str) -> bool {
        EventId::new(id)
            .ok()
            .and_then(|id| self.store.get(&id))
            .is_some_and(|e| e.actions.iter().any(|a| a.id == action_id))
    }

    /// Returns whether anything expired.
    pub fn tick(&mut self, now: Timestamp) -> bool {
        let changed = !self.store.tick(now).is_empty();
        if changed {
            self.save_if_changed();
        }
        changed
    }

    pub fn next_deadline(&self) -> Option<Timestamp> {
        self.store.next_deadline()
    }

    pub fn len(&self) -> usize {
        self.store.len()
    }

    /// Current events in display order as a JSON array.
    pub fn list_json(&self) -> String {
        serde_json::to_string(&self.store.ordered()).unwrap_or_else(|_| "[]".into())
    }

    fn save_if_changed(&mut self) {
        let Some(path) = self.snapshot_path.clone() else {
            return;
        };
        let Ok(json) = serde_json::to_string(&self.store.snapshot()) else {
            return;
        };
        if self.last_saved.as_deref() == Some(json.as_str()) {
            return;
        }
        match save_snapshot(&path, &json) {
            Ok(()) => self.last_saved = Some(json),
            Err(e) => eprintln!("Could not save event snapshot: {e}"),
        }
    }
}

fn load_snapshot(path: &Path) -> Result<Vec<Event>, String> {
    match fs::read_to_string(path) {
        Ok(text) => serde_json::from_str(&text).map_err(|e| e.to_string()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(e) => Err(e.to_string()),
    }
}

/// Writes atomically (temp file + rename), readable by the user only.
fn save_snapshot(path: &Path, json: &str) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(dir)?;
    }
    let tmp = path.with_extension("json.tmp");
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(&tmp)?;
    file.write_all(json.as_bytes())?;
    file.sync_all()?;
    fs::rename(&tmp, path)
}

/// `$XDG_STATE_HOME/selawi-island/events.json`, or `~/.local/state/...`.
pub fn default_snapshot_path() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/state")))?;
    Some(base.join("selawi-island").join("events.json"))
}

#[cfg(test)]
mod tests {
    use super::*;

    const CALL: &str = r#"{"id":"c","source":{"app":"Slack","integration":"test"},
        "category":"call","state":"started","persistent":true,
        "payload":{"type":"call","title":"Standup","confidence":"confirmed"},
        "actions":[{"id":"leave","label":"Leave"}]}"#;
    const NOTE: &str = r#"{"id":"n","source":{"app":"A","integration":"test"},
        "category":"notification","state":"started","ttl_ms":1000,
        "payload":{"type":"notification","title":"Hi"}}"#;

    fn temp_path(name: &str) -> PathBuf {
        std::env::temp_dir()
            .join(format!("selawi-test-{}-{name}", std::process::id()))
            .join("events.json")
    }

    #[test]
    fn submit_list_dismiss() -> Result<(), SubmitError> {
        let mut engine = Engine::new(None, Timestamp(0));
        assert!(engine.submit(CALL, Timestamp(0))?);
        assert!(
            !engine.submit(CALL, Timestamp(1))?,
            "duplicate is not a change"
        );
        assert!(engine.list_json().contains("\"Standup\""));
        assert!(engine.has_action("c", "leave"));
        assert!(!engine.has_action("c", "rm -rf"));
        assert!(engine.dismiss("c"));
        assert!(!engine.dismiss("c"));
        assert_eq!(engine.list_json(), "[]");
        Ok(())
    }

    #[test]
    fn rejects_oversized_and_invalid_input() {
        let mut engine = Engine::new(None, Timestamp(0));
        let huge = format!("\"{}\"", "x".repeat(MAX_INPUT_BYTES));
        assert!(matches!(
            engine.submit(&huge, Timestamp(0)),
            Err(SubmitError::TooLarge)
        ));
        assert!(matches!(
            engine.submit("{}", Timestamp(0)),
            Err(SubmitError::Rejected(_))
        ));
        assert_eq!(engine.len(), 0);
    }

    #[test]
    fn tick_expires_events() -> Result<(), SubmitError> {
        let mut engine = Engine::new(None, Timestamp(0));
        engine.submit(NOTE, Timestamp(0))?;
        assert_eq!(engine.next_deadline(), Some(Timestamp(1000)));
        assert!(!engine.tick(Timestamp(999)));
        assert!(engine.tick(Timestamp(1000)));
        assert_eq!(engine.len(), 0);
        Ok(())
    }

    #[test]
    fn snapshot_survives_restart_with_grace_period() -> Result<(), Box<dyn std::error::Error>> {
        let path = temp_path("restart");
        let _ = fs::remove_dir_all(path.parent().ok_or("no parent")?);
        {
            let mut engine = Engine::new(Some(path.clone()), Timestamp(0));
            engine
                .submit(CALL, Timestamp(0))
                .map_err(|e| e.to_string())?;
            engine
                .submit(NOTE, Timestamp(0))
                .map_err(|e| e.to_string())?;
        }
        let mode = fs::metadata(&path)?.permissions();
        assert_eq!(
            std::os::unix::fs::PermissionsExt::mode(&mode) & 0o777,
            0o600
        );

        let mut engine = Engine::new(Some(path.clone()), Timestamp(5_000));
        assert_eq!(engine.len(), 1, "only the persistent call is restored");
        assert_eq!(
            engine.next_deadline(),
            Some(Timestamp(5_000 + RESTORE_GRACE_MS))
        );
        assert!(engine.tick(Timestamp(5_000 + RESTORE_GRACE_MS)));
        fs::remove_dir_all(path.parent().ok_or("no parent")?)?;
        Ok(())
    }

    #[test]
    fn corrupt_snapshot_is_ignored() -> Result<(), Box<dyn std::error::Error>> {
        let path = temp_path("corrupt");
        let dir = path.parent().ok_or("no parent")?;
        fs::create_dir_all(dir)?;
        fs::write(&path, "not json")?;
        let engine = Engine::new(Some(path.clone()), Timestamp(0));
        assert_eq!(engine.len(), 0);
        fs::remove_dir_all(dir)?;
        Ok(())
    }
}
