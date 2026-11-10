//! Desktop notifications, observed without replacing GNOME's notification
//! server: a dedicated session-bus connection becomes a D-Bus *monitor*
//! for `org.freedesktop.Notifications.Notify` calls only, and each one is
//! shown as a short event. GNOME keeps handling the notification itself
//! (history, actions); the extension can hide GNOME's own banner so it is
//! not shown twice.
//!
//! - Text markup is stripped. The body is shown only if
//!   `notifications.show_body` is on.
//! - Notifications from a media player that is already shown (e.g. a
//!   "now playing" popup) are skipped.
//! - Notification buttons are not offered: only GNOME, as the server, can
//!   deliver them back to the app.

use std::collections::HashMap;
use std::sync::Arc;

use futures_util::StreamExt;
use selawi_core::events::{
    Category, EventId, EventInput, LifecycleState, NotificationPayload, Payload, Source,
};
use selawi_core::Priority;
use zbus::fdo::MonitoringProxy;
use zbus::message::Type as MessageType;
use zbus::{MatchRule, MessageStream, OwnedVariant};

use super::variant::{self, Props};
use super::{id_part, Stop};
use crate::hub::{Hub, Status};

pub const ID: &str = "notifications";
pub const NAME: &str = "Notifications";
const DEFAULT_TTL_MS: u64 = 6_000;
const MIN_TTL_MS: u64 = 2_000;
const MAX_TTL_MS: u64 = 30_000;

/// The arguments of one `Notify` call.
#[derive(Debug, Clone, Default)]
pub struct Notification {
    pub app_name: String,
    pub summary: String,
    pub body: String,
    pub hints: Props,
    pub expire_timeout: i32,
}

/// Lowercase key identifying the sending app ("org.gnome.Nautilus" → "nautilus").
pub fn app_key(n: &Notification) -> String {
    let raw = variant::string(&n.hints, "desktop-entry").unwrap_or_else(|| n.app_name.clone());
    let last = raw.rsplit('.').next().unwrap_or(&raw);
    id_part(&last.to_lowercase())
}

/// The event for a notification, or `None` if there is nothing to show.
/// `seq` makes each notification's id unique.
pub fn notification_input(n: &Notification, seq: u64, show_body: bool) -> Option<EventInput> {
    let title = clean(&n.summary, 256);
    if title.is_empty() {
        return None;
    }
    let app = clean(&n.app_name, 64);
    let body = show_body
        .then(|| clean(&n.body, 1024))
        .filter(|b| !b.is_empty());
    let urgency = variant::u8_(&n.hints, "urgency").unwrap_or(1);
    let ttl = match u64::try_from(n.expire_timeout) {
        Ok(ms) if ms > 0 => ms.clamp(MIN_TTL_MS, MAX_TTL_MS),
        _ if urgency >= 2 => 15_000,
        _ => DEFAULT_TTL_MS,
    };
    let key = app_key(n);
    let input = EventInput {
        id: EventId::new(format!("{ID}:{key}:{seq}")).ok()?,
        source: Source {
            app: if app.is_empty() {
                "Notification".into()
            } else {
                app
            },
            integration: ID.into(),
            icon: variant::string(&n.hints, "desktop-entry").and_then(|e| super::app_icon(&e)),
        },
        category: Category::Notification,
        state: LifecycleState::Started,
        priority: match urgency {
            0 => Some(Priority::Reminder),
            2 => Some(Priority::Critical),
            _ => None,
        },
        persistent: false,
        ttl_ms: Some(ttl),
        group: Some(format!("{ID}:{key}")),
        payload: Payload::Notification(NotificationPayload { title, body }),
        actions: Vec::new(),
    };
    input.validate().ok().map(|()| input)
}

/// Strips markup (`<b>`, `<a href…>`), decodes the basic entities, folds
/// whitespace and control characters, and limits the length.
pub fn clean(text: &str, max: usize) -> String {
    let mut out = String::with_capacity(text.len());
    let mut in_tag = false;
    for c in text.chars() {
        match c {
            '<' => in_tag = true,
            '>' if in_tag => in_tag = false,
            _ if in_tag => {}
            c if c.is_control() || c.is_whitespace() => {
                if !out.ends_with(' ') {
                    out.push(' ');
                }
            }
            c => out.push(c),
        }
    }
    let decoded = out
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&#39;", "'")
        .replace("&amp;", "&");
    decoded.trim().chars().take(max).collect()
}

type NotifyArgs = (
    String,
    u32,
    String,
    String,
    String,
    Vec<String>,
    HashMap<String, OwnedVariant>,
    i32,
);

pub async fn run(hub: Arc<Hub>) -> Result<(), Stop> {
    // A connection of its own: once it is a monitor it can no longer send.
    let conn = zbus::connection::Builder::session().build().await?;
    watch(hub, conn).await
}

/// Turns `conn` into a monitor for Notify calls and follows them.
pub async fn watch(hub: Arc<Hub>, conn: zbus::Connection) -> Result<(), Stop> {
    let rule = MatchRule::builder()
        .msg_type(MessageType::MethodCall)
        .interface("org.freedesktop.Notifications")
        .member("Notify")
        .build()?;
    MonitoringProxy::new(&conn)
        .await?
        .become_monitor(&[rule], 0)
        .await
        .map_err(|e| Stop::Unavailable(format!("the session bus refused monitoring: {e}")))?;
    hub.set_status(ID, NAME, Status::Active("watching notifications".into()))
        .await;

    let mut messages = MessageStream::from(&conn);
    let mut seq: u64 = 0;
    while let Some(message) = messages.next().await {
        let Ok(message) = message else { continue };
        let header = message.header();
        if header.message_type() != MessageType::MethodCall
            || header.member().map(|m| m.as_str()) != Some("Notify")
        {
            continue;
        }
        let Ok((app_name, _replaces, _icon, summary, body, _actions, hints, expire_timeout)) =
            message.body().deserialize::<NotifyArgs>()
        else {
            continue;
        };
        let n = Notification {
            app_name,
            summary,
            body,
            hints,
            expire_timeout,
        };

        // A media player's own "now playing" popup duplicates the media event.
        let key = app_key(&n);
        if hub
            .media_players_json()
            .await
            .contains(&format!("\"id\":\"{key}\""))
        {
            continue;
        }
        seq += 1;
        let show_body = hub.config().notifications.show_body;
        if let Some(input) = notification_input(&n, seq, show_body) {
            if let Err(e) = hub.submit(input).await {
                eprintln!("Notification event rejected: {e}");
            }
        }
    }
    Err(Stop::Failed("monitor connection closed".into()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use zbus::Variant;

    fn note(app: &str, summary: &str, body: &str) -> Notification {
        Notification {
            app_name: app.into(),
            summary: summary.into(),
            body: body.into(),
            hints: Props::new(),
            expire_timeout: -1,
        }
    }

    fn with_hint(mut n: Notification, key: &str, v: Variant<'_>) -> Notification {
        if let Ok(o) = v.try_into_owned() {
            n.hints.insert(key.into(), o);
        }
        n
    }

    fn payload(input: &EventInput) -> (&str, Option<&str>) {
        match &input.payload {
            Payload::Notification(p) => (p.title.as_str(), p.body.as_deref()),
            _ => ("?", None),
        }
    }

    #[test]
    fn plain_notification() -> Result<(), &'static str> {
        let input =
            notification_input(&note("Slack", "Yousef", "Lunch?"), 7, true).ok_or("none")?;
        assert_eq!(input.id.as_str(), "notifications:slack:7");
        assert_eq!(input.group.as_deref(), Some("notifications:slack"));
        assert_eq!(payload(&input), ("Yousef", Some("Lunch?")));
        assert_eq!(input.ttl_ms, Some(DEFAULT_TTL_MS));
        assert!(input.actions.is_empty());
        Ok(())
    }

    #[test]
    fn body_hidden_when_turned_off() -> Result<(), &'static str> {
        let input =
            notification_input(&note("Mail", "New mail", "secret"), 1, false).ok_or("none")?;
        assert_eq!(payload(&input), ("New mail", None));
        Ok(())
    }

    #[test]
    fn markup_and_entities_are_cleaned() {
        assert_eq!(
            clean("<b>Bold</b> &amp; <a href=\"x\">link</a>\n\nnext", 100),
            "Bold & link next"
        );
        assert_eq!(clean("a\u{7}b", 100), "a b");
        assert_eq!(clean(&"x".repeat(2000), 1024).len(), 1024);
    }

    #[test]
    fn urgency_sets_priority_and_time() -> Result<(), &'static str> {
        let critical = with_hint(note("Power", "Battery", ""), "urgency", Variant::U8(2));
        let input = notification_input(&critical, 1, true).ok_or("none")?;
        assert_eq!(input.priority, Some(Priority::Critical));
        assert_eq!(input.ttl_ms, Some(15_000));
        let low = with_hint(note("Updates", "Done", ""), "urgency", Variant::U8(0));
        assert_eq!(
            notification_input(&low, 1, true).ok_or("none")?.priority,
            Some(Priority::Reminder)
        );
        let mut timed = note("A", "B", "");
        timed.expire_timeout = 500;
        assert_eq!(
            notification_input(&timed, 1, true).ok_or("none")?.ttl_ms,
            Some(MIN_TTL_MS)
        );
        Ok(())
    }

    #[test]
    fn empty_title_shows_nothing() {
        assert!(notification_input(&note("A", "  <b></b> ", "body"), 1, true).is_none());
    }

    #[test]
    fn app_key_prefers_desktop_entry() {
        let n = with_hint(
            note("Files", "Copied", ""),
            "desktop-entry",
            Variant::from("org.gnome.Nautilus"),
        );
        assert_eq!(app_key(&n), "nautilus");
        assert_eq!(app_key(&note("Spotify", "x", "")), "spotify");
    }
}
