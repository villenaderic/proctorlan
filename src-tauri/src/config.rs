//! Central backend configuration. Mirror any value shared with `src/config.ts`.

use std::time::Duration;

pub const APP_NAME: &str = "ProctorLAN";
/// Default LAN host port (non-privileged, rarely used by other software).
pub const DEFAULT_PORT: u16 = 38_123;
pub const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(5);
/// A student is only "Disconnected" after several missed beats, never one.
pub const HEARTBEAT_MISSES_BEFORE_DISCONNECT: u32 = 4;
pub const MAX_STUDENTS: usize = 100;
pub const SESSION_CODE_LENGTH: usize = 5;
pub const DEFAULT_LOG_FILTER: &str = "info";
pub const DB_FILE_NAME: &str = "proctorlan.db";
/// Teacher authentication limits (mirrored in src/config.ts for form validation).
pub const MIN_PASSWORD_LEN: usize = 8;
pub const MAX_PASSWORD_LEN: usize = 128;
pub const USERNAME_MIN_LEN: usize = 3;
pub const USERNAME_MAX_LEN: usize = 32;
pub const DISPLAY_NAME_MAX_LEN: usize = 64;
pub const MAX_FAILED_LOGINS: u32 = 5;
pub const LOGIN_LOCKOUT: Duration = Duration::from_secs(60);
/// Teacher is signed out after this long without any command.
pub const TEACHER_IDLE_TIMEOUT: Duration = Duration::from_secs(8 * 60 * 60);
/// Exam content limits (protect the database, the LAN payloads and the UI from absurd input).
pub const MAX_QUESTIONS_PER_EXAM: usize = 500;
pub const MAX_CHOICES_PER_QUESTION: usize = 10;
pub const MAX_TITLE_LEN: usize = 200;
pub const MAX_TEXT_LEN: usize = 5_000;
pub const MAX_CHOICE_LEN: usize = 500;
pub const MAX_POINTS: f64 = 1_000.0;
pub const MAX_DURATION_MINUTES: i64 = 1_440;
/// LAN server limits.
pub const MAX_HTTP_BODY_BYTES: usize = 256 * 1024;
pub const MAX_WS_MESSAGE_BYTES: usize = 64 * 1024;
/// A WebSocket must send `hello` within this time or it is dropped.
pub const WS_HELLO_TIMEOUT: Duration = Duration::from_secs(10);
/// Server broadcasts a time sync to connected clients this often.
pub const TIMER_SYNC_INTERVAL: Duration = Duration::from_secs(5);
pub const MDNS_SERVICE_TYPE: &str = "_proctorlan._tcp.local.";
pub const SETTING_PORT: &str = "network.port";
pub const SETTING_INTERFACE: &str = "network.interface";
/// Longest typed answer for an identification question.
pub const MAX_IDENTIFICATION_ANSWER_LEN: usize = 500;
/// Network-delay allowance: answers arriving this soon after the deadline still count.
pub const ANSWER_GRACE: Duration = Duration::from_secs(3);
/// How often the server checks for sessions whose time has run out.
pub const EXPIRY_SWEEP_INTERVAL: Duration = Duration::from_secs(1);
/// Most answers accepted in one `answers_sync` batch (a student who was offline for a while).
pub const MAX_SYNC_BATCH: usize = 200;
/// Per-connection message budget: above SOFT messages in a second extra ones are refused with
/// `rate_limited`; above HARD the connection is closed. Normal use is a handful per second.
pub const RATE_SOFT_PER_SECOND: u32 = 40;
pub const RATE_HARD_PER_SECOND: u32 = 200;
pub const DB_MAX_CONNECTIONS: u32 = 5;
pub const DB_BUSY_TIMEOUT: Duration = Duration::from_secs(5);

/// Time after which a silent student is considered disconnected.
pub fn disconnect_timeout() -> Duration {
    HEARTBEAT_INTERVAL * HEARTBEAT_MISSES_BEFORE_DISCONNECT
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disconnect_needs_more_than_one_missed_beat() {
        assert!(HEARTBEAT_MISSES_BEFORE_DISCONNECT > 1);
        assert!(disconnect_timeout() > HEARTBEAT_INTERVAL);
    }

    #[test]
    fn port_is_unprivileged() {
        assert!(DEFAULT_PORT > 1024);
    }
}
