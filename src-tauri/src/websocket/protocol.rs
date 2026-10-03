//! Wire protocol. Every frame is a JSON envelope:
//! `{ "id", "type", "sessionId", "timestamp", "payload" }`.
//! Phase 5 implements: hello/welcome; Phase 6 adds join/joined/removed, heartbeat/heartbeat_ack, timer_sync, session_* broadcasts, error.
//! Student answer/submit/proctor messages are added in Phases 6–9 as new `type` values.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::database::ids::{new_id, now};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Envelope {
    #[serde(default)]
    pub id: String,
    #[serde(rename = "type")]
    pub kind: String,
    #[serde(default)]
    pub session_id: Option<String>,
    #[serde(default)]
    pub timestamp: String,
    #[serde(default)]
    pub payload: Value,
}

/// Message types the server accepts from clients.
pub mod client {
    pub const HELLO: &str = "hello";
    pub const HEARTBEAT: &str = "heartbeat";
    pub const JOIN: &str = "join";
}

/// Message types the server sends.
pub mod server {
    pub const WELCOME: &str = "welcome";
    pub const JOINED: &str = "joined";
    pub const REMOVED: &str = "removed";
    pub const HEARTBEAT_ACK: &str = "heartbeat_ack";
    pub const TIMER_SYNC: &str = "timer_sync";
    pub const SESSION_WAITING: &str = "session_waiting";
    pub const SESSION_STARTED: &str = "session_started";
    pub const SESSION_PAUSED: &str = "session_paused";
    pub const SESSION_RESUMED: &str = "session_resumed";
    pub const SESSION_ENDED: &str = "session_ended";
    pub const ERROR: &str = "error";
}

impl Envelope {
    pub fn new(kind: &str, session_id: Option<&str>, payload: Value) -> Self {
        Self { id: new_id(), kind: kind.to_string(), session_id: session_id.map(str::to_string), timestamp: now(), payload }
    }

    pub fn error(code: &str, message: &str) -> Self {
        Self::new(server::ERROR, None, json!({ "code": code, "message": message }))
    }

    /// Reply that references the message it answers (`inReplyTo`) so clients can match it.
    pub fn reply(kind: &str, to: &Envelope, session_id: Option<&str>, mut payload: Value) -> Self {
        if let Value::Object(map) = &mut payload {
            map.insert("inReplyTo".into(), Value::String(to.id.clone()));
        }
        Self::new(kind, session_id, payload)
    }

    pub fn to_text(&self) -> String {
        serde_json::to_string(self).unwrap_or_else(|_| "{}".into())
    }
}

/// Parses one text frame; rejects oversize or malformed input without panicking.
pub fn parse_frame(text: &str, max_bytes: usize) -> Result<Envelope, &'static str> {
    if text.len() > max_bytes {
        return Err("message_too_large");
    }
    let env: Envelope = serde_json::from_str(text).map_err(|_| "malformed_message")?;
    if env.kind.is_empty() || env.kind.len() > 64 {
        return Err("malformed_message");
    }
    Ok(env)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_valid_frames_with_defaults() {
        let e = parse_frame(r#"{"type":"heartbeat"}"#, 1024).unwrap();
        assert_eq!(e.kind, "heartbeat");
        assert!(e.session_id.is_none() && e.payload.is_null());
        let e = parse_frame(r#"{"id":"1","type":"hello","sessionId":"s","timestamp":"t","payload":{"a":1}}"#, 1024).unwrap();
        assert_eq!((e.id.as_str(), e.session_id.as_deref()), ("1", Some("s")));
    }

    #[test]
    fn rejects_garbage_missing_type_and_oversize() {
        assert_eq!(parse_frame("not json", 1024).unwrap_err(), "malformed_message");
        assert_eq!(parse_frame("[]", 1024).unwrap_err(), "malformed_message");
        assert_eq!(parse_frame(r#"{"payload":{}}"#, 1024).unwrap_err(), "malformed_message");
        assert_eq!(parse_frame(r#"{"type":""}"#, 1024).unwrap_err(), "malformed_message");
        assert_eq!(parse_frame(&format!(r#"{{"type":"{}"}}"#, "x".repeat(100)), 1024).unwrap_err(), "malformed_message");
        assert_eq!(parse_frame(&format!(r#"{{"type":"hello","payload":"{}"}}"#, "x".repeat(2000)), 1024).unwrap_err(), "message_too_large");
    }

    #[test]
    fn replies_reference_the_request_and_use_camel_case() {
        let req = Envelope::new("hello", None, json!({}));
        let r = Envelope::reply("welcome", &req, Some("sess"), json!({ "x": 1 }));
        assert_eq!(r.payload["inReplyTo"], req.id);
        assert!(r.to_text().contains("\"sessionId\":\"sess\""));
    }
}
