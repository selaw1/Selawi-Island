//! Messages between the browser extension and this host, and the rules
//! every event from the browser must follow.
//!
//! Wire format (native messaging): a 32-bit length in native byte order,
//! then that many bytes of UTF-8 JSON.

use std::io;

use selawi_core::{parse_input, EventInput};
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

/// Largest message accepted from the browser.
pub const MAX_MESSAGE_BYTES: usize = 64 * 1024;
/// Every browser event id starts with this; the host refuses anything else,
/// so a web page can never pose as another integration.
pub const ID_PREFIX: &str = "browser:";
pub const INTEGRATION: &str = "browser";

#[derive(Debug, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum FromBrowser {
    Hello { version: u32, browser: String },
    Event { event: serde_json::Value },
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ToBrowser {
    /// A button on one of the browser's events was pressed in the island.
    Action { id: String, action: String },
}

/// Reads one message. `Ok(None)` means the browser closed the connection.
pub async fn read_message<R: AsyncRead + Unpin>(reader: &mut R) -> io::Result<Option<Vec<u8>>> {
    let mut len = [0u8; 4];
    match reader.read_exact(&mut len).await {
        Ok(_) => {}
        Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => return Ok(None),
        Err(e) => return Err(e),
    }
    let len = u32::from_ne_bytes(len) as usize;
    if len > MAX_MESSAGE_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "message too large",
        ));
    }
    let mut body = vec![0u8; len];
    reader.read_exact(&mut body).await?;
    Ok(Some(body))
}

pub async fn write_message<W: AsyncWrite + Unpin>(writer: &mut W, body: &[u8]) -> io::Result<()> {
    let len = u32::try_from(body.len())
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "message too large"))?;
    writer.write_all(&len.to_ne_bytes()).await?;
    writer.write_all(body).await?;
    writer.flush().await
}

/// Checks an event from the browser: valid protocol v1 input, id under
/// `browser:`, and marked as coming from the browser integration.
pub fn validate_event(value: &serde_json::Value) -> Result<EventInput, String> {
    let input = parse_input(&value.to_string()).map_err(|e| e.to_string())?;
    if !input.id.as_str().starts_with(ID_PREFIX) {
        return Err(format!(
            "event ids from the browser must start with {ID_PREFIX:?}"
        ));
    }
    if input.source.integration != INTEGRATION {
        return Err(format!("source.integration must be {INTEGRATION:?}"));
    }
    Ok(input)
}

/// Allows a burst of `capacity` messages, refilled at `per_second`.
pub struct RateLimit {
    capacity: f64,
    per_second: f64,
    tokens: f64,
    last: std::time::Instant,
}

impl RateLimit {
    pub fn new(capacity: u32, per_second: u32) -> Self {
        Self {
            capacity: f64::from(capacity),
            per_second: f64::from(per_second),
            tokens: f64::from(capacity),
            last: std::time::Instant::now(),
        }
    }

    pub fn allow(&mut self) -> bool {
        let now = std::time::Instant::now();
        let elapsed = now.duration_since(self.last).as_secs_f64();
        self.last = now;
        self.tokens = (self.tokens + elapsed * self.per_second).min(self.capacity);
        if self.tokens >= 1.0 {
            self.tokens -= 1.0;
            true
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn meet_event() -> serde_json::Value {
        json!({
            "id": "browser:call:12",
            "source": {"app": "Google Meet", "integration": "browser"},
            "category": "call", "state": "started", "persistent": true,
            "payload": {"type": "call", "title": "Weekly sync", "confidence": "confirmed", "muted": false},
            "actions": [{"id": "mute", "label": "Mute"}, {"id": "leave", "label": "Leave"}]
        })
    }

    #[tokio::test]
    async fn framing_round_trip() -> io::Result<()> {
        let mut buf = Vec::new();
        write_message(
            &mut buf,
            br#"{"type":"hello","version":1,"browser":"chrome"}"#,
        )
        .await?;
        write_message(&mut buf, b"{}").await?;
        let mut reader = buf.as_slice();
        let first = read_message(&mut reader).await?.unwrap_or_default();
        let parsed: FromBrowser = serde_json::from_slice(&first).map_err(io::Error::other)?;
        assert_eq!(
            parsed,
            FromBrowser::Hello {
                version: 1,
                browser: "chrome".into()
            }
        );
        assert_eq!(read_message(&mut reader).await?, Some(b"{}".to_vec()));
        assert_eq!(read_message(&mut reader).await?, None, "end of stream");
        Ok(())
    }

    #[tokio::test]
    async fn oversized_messages_are_refused() {
        let mut buf = ((MAX_MESSAGE_BYTES + 1) as u32).to_ne_bytes().to_vec();
        buf.extend(vec![b' '; 16]);
        assert!(read_message(&mut buf.as_slice()).await.is_err());
    }

    #[test]
    fn accepts_browser_events() {
        assert!(validate_event(&meet_event()).is_ok());
    }

    #[test]
    fn refuses_events_posing_as_other_integrations() {
        let mut e = meet_event();
        e["id"] = json!("mpris:spotify");
        assert!(validate_event(&e).is_err());
        let mut e = meet_event();
        e["source"]["integration"] = json!("upower");
        assert!(validate_event(&e).is_err());
        let mut e = meet_event();
        e["exec"] = json!("rm -rf ~");
        assert!(validate_event(&e).is_err(), "unknown fields are refused");
        assert!(serde_json::from_str::<FromBrowser>(r#"{"type":"run","cmd":"x"}"#).is_err());
    }

    #[test]
    fn rate_limit_allows_a_burst_then_refuses() {
        let mut limit = RateLimit::new(5, 1);
        assert_eq!((0..10).filter(|_| limit.allow()).count(), 5);
    }

    #[test]
    fn actions_serialize_for_the_extension() -> Result<(), serde_json::Error> {
        let json = serde_json::to_string(&ToBrowser::Action {
            id: "browser:call:12".into(),
            action: "mute".into(),
        })?;
        assert_eq!(
            json,
            r#"{"type":"action","id":"browser:call:12","action":"mute"}"#
        );
        Ok(())
    }
}
