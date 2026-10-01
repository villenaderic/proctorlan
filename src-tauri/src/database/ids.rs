use chrono::{SecondsFormat, Utc};

pub fn new_id() -> String {
    uuid::Uuid::new_v4().to_string()
}

/// Current UTC time as RFC3339 with milliseconds (sorts lexicographically).
pub fn now() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true)
}
