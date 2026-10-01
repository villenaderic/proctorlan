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
