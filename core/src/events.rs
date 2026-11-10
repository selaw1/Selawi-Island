//! Normalized event types shared by every source, the engine and the UI.
//!
//! Sources submit an [`EventInput`]. The engine validates it, stamps
//! timestamps, resolves priority and expiry, and stores an [`Event`].
//! Both serialize to the JSON shape documented in `docs/EVENT_SYSTEM.md`.

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::priority::Priority;

/// Version of the event protocol described in `docs/EVENT_SYSTEM.md`.
pub const PROTOCOL_VERSION: u32 = 1;

const MAX_ID_LEN: usize = 128;
const MAX_NAME_LEN: usize = 64;
const MAX_TITLE_LEN: usize = 256;
const MAX_TEXT_LEN: usize = 1024;
const MAX_LABEL_LEN: usize = 32;
const MAX_ACTIONS: usize = 4;
/// Longest time-to-live a source may request: 24 hours.
pub const MAX_TTL_MS: u64 = 24 * 60 * 60 * 1000;

/// Milliseconds since the Unix epoch.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Timestamp(pub u64);

impl Timestamp {
    /// Current wall-clock time. Falls back to 0 if the clock is before 1970.
    pub fn now() -> Timestamp {
        let ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0);
        Timestamp(u64::try_from(ms).unwrap_or(u64::MAX))
    }

    pub fn plus_ms(self, ms: u64) -> Timestamp {
        Timestamp(self.0.saturating_add(ms))
    }
}

/// Stable identifier chosen by the source. Reusing an id updates the
/// existing event instead of creating a new one.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct EventId(String);

impl EventId {
    pub fn new(value: impl Into<String>) -> Result<Self, ValidationError> {
        let value = value.into();
        if is_valid_token(&value, MAX_ID_LEN) {
            Ok(EventId(value))
        } else {
            Err(ValidationError::InvalidId(value))
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for EventId {
    type Error = ValidationError;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        EventId::new(value)
    }
}

impl From<EventId> for String {
    fn from(id: EventId) -> String {
        id.0
    }
}

impl fmt::Display for EventId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Where an event came from.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Source {
    /// Human-readable application name, e.g. "Slack".
    pub app: String,
    /// Integration that produced the event, e.g. "mpris" or "simulator".
    pub integration: String,
    /// The app's or site's logo: `app:<desktop entry id>` for an installed
    /// app (e.g. `app:spotify`), or a small PNG as
    /// `data:image/png;base64,…` (at most `MAX_ICON_LEN` characters).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
}

/// Longest `source.icon` accepted (about 36 KB of PNG).
pub const MAX_ICON_LEN: usize = 48 * 1024;
const PNG_DATA_PREFIX: &str = "data:image/png;base64,";

pub(crate) fn check_icon(icon: &str) -> Result<(), ValidationError> {
    let bad = || ValidationError::InvalidText {
        field: "source.icon",
        reason: "must be app:<id> or a PNG data URL",
    };
    if let Some(id) = icon.strip_prefix("app:") {
        let ok = !id.is_empty()
            && id.len() <= MAX_ID_LEN
            && id
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'));
        return if ok { Ok(()) } else { Err(bad()) };
    }
    if let Some(data) = icon.strip_prefix(PNG_DATA_PREFIX) {
        if icon.len() > MAX_ICON_LEN {
            return Err(ValidationError::OutOfRange {
                field: "source.icon",
            });
        }
        let ok = !data.is_empty()
            && data
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '/' | '='));
        return if ok { Ok(()) } else { Err(bad()) };
    }
    Err(bad())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Category {
    Call,
    Media,
    Transfer,
    Device,
    Notification,
    System,
    Reminder,
    Metric,
    Custom,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LifecycleState {
    Started,
    Updated,
    Ended,
    Failed,
    Expired,
}

impl LifecycleState {
    pub fn is_terminal(self) -> bool {
        matches!(self, Self::Ended | Self::Failed | Self::Expired)
    }
}

/// How sure an integration is that a call is really happening.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Confidence {
    Confirmed,
    Probable,
    AudioOnly,
    Unknown,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    Info,
    Warning,
    Critical,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeviceKind {
    Headphones,
    Speaker,
    Keyboard,
    Mouse,
    Phone,
    Other,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CallPayload {
    /// Meeting or caller name.
    pub title: String,
    pub confidence: Confidence,
    /// Whether the user's microphone is muted in the call. Only set when
    /// the source really knows (never guessed).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub muted: Option<bool>,
    /// Whether the user's camera is on in the call. Only set when the
    /// source really knows.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub camera: Option<bool>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MediaPayload {
    pub title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub artist: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub album: Option<String>,
    pub playing: bool,
    /// Playback position at the moment the event was last updated.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub position_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u64>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProgressPayload {
    pub title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    /// 0–100, or `None` when progress is unknown.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub percent: Option<f64>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DevicePayload {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub device_kind: Option<DeviceKind>,
    pub connected: bool,
    /// Only set when the device actually reports it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub battery_percent: Option<u8>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NotificationPayload {
    pub title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub body: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SystemPayload {
    pub title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    pub severity: Severity,
}

/// Typed event content. Serialized with a `"type"` tag.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Payload {
    Call(CallPayload),
    Media(MediaPayload),
    Progress(ProgressPayload),
    Device(DevicePayload),
    Notification(NotificationPayload),
    System(SystemPayload),
}

/// A button the UI may show. The id is sent back to the source when pressed.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Action {
    pub id: String,
    pub label: String,
}

/// What a source submits to the engine.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EventInput {
    pub id: EventId,
    pub source: Source,
    pub category: Category,
    pub state: LifecycleState,
    /// Overrides the category's default priority.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub priority: Option<Priority>,
    /// Persistent events stay until they end or the user dismisses them.
    #[serde(default)]
    pub persistent: bool,
    /// Time-to-live for non-persistent events. Refreshed on every update.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ttl_ms: Option<u64>,
    /// Events with the same group key are shown together.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
    pub payload: Payload,
    #[serde(default)]
    pub actions: Vec<Action>,
}

/// An event as stored by the engine and sent to the UI.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Event {
    pub id: EventId,
    pub source: Source,
    pub category: Category,
    pub state: LifecycleState,
    pub priority: Priority,
    pub persistent: bool,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<Timestamp>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
    pub payload: Payload,
    #[serde(default)]
    pub actions: Vec<Action>,
}

impl Event {
    /// The input that would produce this event, e.g. to send it again with
    /// a new state. The resolved priority is kept as an explicit priority.
    pub fn to_input(&self) -> EventInput {
        EventInput {
            id: self.id.clone(),
            source: self.source.clone(),
            category: self.category,
            state: self.state,
            priority: Some(self.priority),
            persistent: self.persistent,
            ttl_ms: None,
            group: self.group.clone(),
            payload: self.payload.clone(),
            actions: self.actions.clone(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ValidationError {
    InvalidId(String),
    InvalidText {
        field: &'static str,
        reason: &'static str,
    },
    OutOfRange {
        field: &'static str,
    },
    TooManyActions,
    DuplicateAction(String),
    PayloadCategoryMismatch,
}

impl fmt::Display for ValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidId(id) => write!(f, "invalid identifier {id:?}"),
            Self::InvalidText { field, reason } => write!(f, "{field}: {reason}"),
            Self::OutOfRange { field } => write!(f, "{field} is out of range"),
            Self::TooManyActions => write!(f, "at most {MAX_ACTIONS} actions are allowed"),
            Self::DuplicateAction(id) => write!(f, "duplicate action id {id:?}"),
            Self::PayloadCategoryMismatch => write!(f, "payload type does not match category"),
        }
    }
}

impl std::error::Error for ValidationError {}

impl EventInput {
    /// Checks everything serde cannot: lengths, ranges and consistency.
    pub fn validate(&self) -> Result<(), ValidationError> {
        check_text("source.app", &self.source.app, MAX_NAME_LEN)?;
        check_text("source.integration", &self.source.integration, MAX_NAME_LEN)?;
        if let Some(icon) = &self.source.icon {
            check_icon(icon)?;
        }
        if let Some(group) = &self.group {
            if !is_valid_token(group, MAX_ID_LEN) {
                return Err(ValidationError::InvalidId(group.clone()));
            }
        }
        if let Some(ttl) = self.ttl_ms {
            if ttl == 0 || ttl > MAX_TTL_MS {
                return Err(ValidationError::OutOfRange { field: "ttl_ms" });
            }
        }
        self.validate_actions()?;
        self.validate_payload()
    }

    fn validate_actions(&self) -> Result<(), ValidationError> {
        if self.actions.len() > MAX_ACTIONS {
            return Err(ValidationError::TooManyActions);
        }
        for (index, action) in self.actions.iter().enumerate() {
            if !is_valid_token(&action.id, MAX_NAME_LEN) {
                return Err(ValidationError::InvalidId(action.id.clone()));
            }
            check_text("actions.label", &action.label, MAX_LABEL_LEN)?;
            if self.actions[..index].iter().any(|a| a.id == action.id) {
                return Err(ValidationError::DuplicateAction(action.id.clone()));
            }
        }
        Ok(())
    }

    fn validate_payload(&self) -> Result<(), ValidationError> {
        let category_ok = match &self.payload {
            Payload::Call(_) => self.category == Category::Call,
            Payload::Media(_) => self.category == Category::Media,
            Payload::Device(_) => self.category == Category::Device,
            Payload::Progress(_) | Payload::Notification(_) | Payload::System(_) => true,
        };
        if !category_ok {
            return Err(ValidationError::PayloadCategoryMismatch);
        }
        match &self.payload {
            Payload::Call(p) => check_text("payload.title", &p.title, MAX_TITLE_LEN),
            Payload::Media(p) => {
                check_text("payload.title", &p.title, MAX_TITLE_LEN)?;
                check_optional_text("payload.artist", p.artist.as_deref(), MAX_TITLE_LEN)?;
                check_optional_text("payload.album", p.album.as_deref(), MAX_TITLE_LEN)
            }
            Payload::Progress(p) => {
                check_text("payload.title", &p.title, MAX_TITLE_LEN)?;
                check_optional_text("payload.detail", p.detail.as_deref(), MAX_TEXT_LEN)?;
                match p.percent {
                    Some(v) if !(0.0..=100.0).contains(&v) => Err(ValidationError::OutOfRange {
                        field: "payload.percent",
                    }),
                    _ => Ok(()),
                }
            }
            Payload::Device(p) => {
                check_text("payload.name", &p.name, MAX_TITLE_LEN)?;
                match p.battery_percent {
                    Some(v) if v > 100 => Err(ValidationError::OutOfRange {
                        field: "payload.battery_percent",
                    }),
                    _ => Ok(()),
                }
            }
            Payload::Notification(p) => {
                check_text("payload.title", &p.title, MAX_TITLE_LEN)?;
                check_optional_text("payload.body", p.body.as_deref(), MAX_TEXT_LEN)
            }
            Payload::System(p) => {
                check_text("payload.title", &p.title, MAX_TITLE_LEN)?;
                check_optional_text("payload.detail", p.detail.as_deref(), MAX_TEXT_LEN)
            }
        }
    }
}

/// Letters, digits and `. _ : -`, 1..=max characters.
fn is_valid_token(value: &str, max: usize) -> bool {
    !value.is_empty()
        && value.len() <= max
        && value
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | ':' | '-'))
}

fn check_text(field: &'static str, value: &str, max: usize) -> Result<(), ValidationError> {
    if value.trim().is_empty() {
        return Err(ValidationError::InvalidText {
            field,
            reason: "must not be empty",
        });
    }
    if value.chars().count() > max {
        return Err(ValidationError::InvalidText {
            field,
            reason: "is too long",
        });
    }
    if value.chars().any(|c| c.is_control() && c != '\n') {
        return Err(ValidationError::InvalidText {
            field,
            reason: "contains control characters",
        });
    }
    Ok(())
}

fn check_optional_text(
    field: &'static str,
    value: Option<&str>,
    max: usize,
) -> Result<(), ValidationError> {
    value.map_or(Ok(()), |v| check_text(field, v, max))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn notification(title: &str) -> EventInput {
        EventInput {
            id: EventId("n1".into()),
            source: Source {
                app: "Test".into(),
                integration: "test".into(),
                icon: None,
            },
            category: Category::Notification,
            state: LifecycleState::Started,
            priority: None,
            persistent: false,
            ttl_ms: None,
            group: None,
            payload: Payload::Notification(NotificationPayload {
                title: title.into(),
                body: None,
            }),
            actions: vec![],
        }
    }

    #[test]
    fn event_id_rejects_bad_characters() {
        assert!(EventId::new("download-1").is_ok());
        assert!(EventId::new("").is_err());
        assert!(EventId::new("has space").is_err());
        assert!(EventId::new("x".repeat(129)).is_err());
    }

    #[test]
    fn rejects_empty_and_control_text() {
        assert!(notification("Hello").validate().is_ok());
        assert!(notification("   ").validate().is_err());
        assert!(notification("bad\u{7}bell").validate().is_err());
    }

    #[test]
    fn rejects_payload_category_mismatch() {
        let mut input = notification("x");
        input.payload = Payload::Call(CallPayload {
            title: "Standup".into(),
            confidence: Confidence::Confirmed,
            muted: None,
            camera: None,
        });
        assert_eq!(
            input.validate(),
            Err(ValidationError::PayloadCategoryMismatch)
        );
    }

    #[test]
    fn rejects_out_of_range_percent_and_ttl() {
        let mut input = notification("x");
        input.category = Category::Transfer;
        input.payload = Payload::Progress(ProgressPayload {
            title: "file.iso".into(),
            detail: None,
            percent: Some(120.0),
        });
        assert!(input.validate().is_err());

        let mut input = notification("x");
        input.ttl_ms = Some(0);
        assert!(input.validate().is_err());
    }

    #[test]
    fn rejects_duplicate_and_excess_actions() {
        let action = |id: &str| Action {
            id: id.into(),
            label: "Go".into(),
        };
        let mut input = notification("x");
        input.actions = vec![action("a"), action("a")];
        assert!(matches!(
            input.validate(),
            Err(ValidationError::DuplicateAction(_))
        ));
        input.actions = (0..5).map(|i| action(&format!("a{i}"))).collect();
        assert_eq!(input.validate(), Err(ValidationError::TooManyActions));
    }

    #[test]
    fn json_round_trip_uses_documented_shape() -> Result<(), serde_json::Error> {
        let json = r#"{
            "id": "dl-1",
            "source": {"app": "Firefox", "integration": "browser"},
            "category": "transfer",
            "state": "updated",
            "persistent": true,
            "payload": {"type": "progress", "title": "file.iso", "percent": 42.5},
            "actions": [{"id": "cancel", "label": "Cancel"}]
        }"#;
        let input: EventInput = serde_json::from_str(json)?;
        assert_eq!(input.validate(), Ok(()));
        let back: EventInput = serde_json::from_str(&serde_json::to_string(&input)?)?;
        assert_eq!(back, input);
        Ok(())
    }

    #[test]
    fn unknown_fields_are_rejected() {
        let json = r#"{"id":"a","source":{"app":"A","integration":"b"},"category":"notification",
            "state":"started","payload":{"type":"notification","title":"t"},"shell":"rm -rf"}"#;
        assert!(serde_json::from_str::<EventInput>(json).is_err());
    }
}
