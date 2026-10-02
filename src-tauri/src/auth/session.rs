//! In-memory teacher session and failed-login throttling.
//!
//! The desktop app has one teacher window, and teacher commands are Tauri IPC (not reachable over
//! the LAN), so the signed-in user lives here in Rust — the frontend never holds a credential.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use crate::config;

#[derive(Debug, Clone, Copy)]
pub struct Limits {
    pub max_failures: u32,
    pub lockout: Duration,
    pub idle_timeout: Duration,
}

impl Default for Limits {
    fn default() -> Self {
        Self { max_failures: config::MAX_FAILED_LOGINS, lockout: config::LOGIN_LOCKOUT, idle_timeout: config::TEACHER_IDLE_TIMEOUT }
    }
}

struct Active {
    user_id: String,
    last_seen: Instant,
}

#[derive(Default)]
struct Failure {
    count: u32,
    locked_until: Option<Instant>,
}

#[derive(Default)]
struct Inner {
    active: Option<Active>,
    failures: HashMap<String, Failure>,
}

pub struct SessionState {
    limits: Limits,
    inner: Mutex<Inner>,
}

impl SessionState {
    pub fn new(limits: Limits) -> Self {
        Self { limits, inner: Mutex::new(Inner::default()) }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    pub fn sign_in(&self, user_id: &str) {
        self.lock().active = Some(Active { user_id: user_id.to_string(), last_seen: Instant::now() });
    }

    pub fn sign_out(&self) {
        self.lock().active = None;
    }

    /// The signed-in user id, refreshing the idle timer; `None` if signed out or idle too long.
    pub fn active_user(&self) -> Option<String> {
        let mut g = self.lock();
        let expired = g.active.as_ref().is_some_and(|a| a.last_seen.elapsed() > self.limits.idle_timeout);
        if expired {
            g.active = None;
        }
        g.active.as_mut().map(|a| {
            a.last_seen = Instant::now();
            a.user_id.clone()
        })
    }

    /// Remaining lockout time for this username key, if currently locked.
    pub fn lockout_remaining(&self, key: &str) -> Option<Duration> {
        let mut g = self.lock();
        let until = g.failures.get(key).and_then(|f| f.locked_until)?;
        let now = Instant::now();
        if until > now {
            Some(until - now)
        } else {
            g.failures.remove(key); // lockout served: start fresh
            None
        }
    }

    pub fn record_failure(&self, key: &str) {
        let mut g = self.lock();
        let f = g.failures.entry(key.to_string()).or_default();
        f.count += 1;
        if f.count >= self.limits.max_failures {
            f.locked_until = Some(Instant::now() + self.limits.lockout);
        }
    }

    pub fn clear_failures(&self, key: &str) {
        self.lock().failures.remove(key);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state(max: u32, lock_ms: u64, idle_ms: u64) -> SessionState {
        SessionState::new(Limits { max_failures: max, lockout: Duration::from_millis(lock_ms), idle_timeout: Duration::from_millis(idle_ms) })
    }

    #[test]
    fn locks_after_max_failures_and_recovers() {
        let s = state(3, 60, 10_000);
        s.record_failure("a");
        s.record_failure("a");
        assert!(s.lockout_remaining("a").is_none());
        s.record_failure("a");
        assert!(s.lockout_remaining("a").is_some());
        assert!(s.lockout_remaining("other").is_none(), "other usernames unaffected");
        std::thread::sleep(Duration::from_millis(80));
        assert!(s.lockout_remaining("a").is_none());
    }

    #[test]
    fn success_clears_failure_count() {
        let s = state(2, 1000, 10_000);
        s.record_failure("a");
        s.clear_failures("a");
        s.record_failure("a");
        assert!(s.lockout_remaining("a").is_none());
    }

    #[test]
    fn session_expires_when_idle() {
        let s = state(5, 1000, 40);
        s.sign_in("u1");
        assert_eq!(s.active_user().as_deref(), Some("u1"));
        std::thread::sleep(Duration::from_millis(60));
        assert_eq!(s.active_user(), None);
    }

    #[test]
    fn activity_keeps_session_alive_and_sign_out_ends_it() {
        let s = state(5, 1000, 80);
        s.sign_in("u1");
        for _ in 0..3 {
            std::thread::sleep(Duration::from_millis(40));
            assert!(s.active_user().is_some());
        }
        s.sign_out();
        assert!(s.active_user().is_none());
    }
}
