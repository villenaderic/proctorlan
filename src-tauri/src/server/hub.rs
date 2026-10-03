//! Live connection registry + event bus shared by the HTTP/WS server and the session service.

use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use tokio::sync::broadcast;

use crate::config;
use crate::websocket::protocol::Envelope;

const FAILURE_WINDOW: Duration = Duration::from_secs(60);
const MAX_FAILURES_PER_WINDOW: u32 = 10;

pub struct Hub {
    events: broadcast::Sender<Envelope>,
    clients: Mutex<HashMap<String, usize>>,
    failures: Mutex<HashMap<IpAddr, (u32, Instant)>>,
}

/// Held for the lifetime of one WebSocket connection; dropping it unregisters the client.
pub struct ConnGuard {
    hub: Arc<Hub>,
    session_id: String,
}

impl Drop for ConnGuard {
    fn drop(&mut self) {
        let mut g = self.hub.clients.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(n) = g.get_mut(&self.session_id) {
            *n = n.saturating_sub(1);
            if *n == 0 {
                g.remove(&self.session_id);
            }
        }
    }
}

impl Hub {
    pub fn new() -> Arc<Self> {
        Arc::new(Self { events: broadcast::channel(256).0, clients: Mutex::default(), failures: Mutex::default() })
    }

    pub fn connected(&self, session_id: &str) -> usize {
        self.clients.lock().unwrap_or_else(|e| e.into_inner()).get(session_id).copied().unwrap_or(0)
    }

    /// Registers a connection unless the session is at capacity.
    pub fn try_join(self: &Arc<Self>, session_id: &str) -> Option<ConnGuard> {
        let mut g = self.clients.lock().unwrap_or_else(|e| e.into_inner());
        let n = g.entry(session_id.to_string()).or_insert(0);
        if *n >= config::MAX_STUDENTS {
            return None;
        }
        *n += 1;
        Some(ConnGuard { hub: self.clone(), session_id: session_id.to_string() })
    }

    pub fn publish(&self, env: Envelope) {
        let _ = self.events.send(env); // no subscribers is fine
    }

    pub fn subscribe(&self) -> broadcast::Receiver<Envelope> {
        self.events.subscribe()
    }

    /// True if this address is currently blocked for too many bad session-code guesses.
    pub fn is_blocked(&self, ip: IpAddr) -> bool {
        let mut g = self.failures.lock().unwrap_or_else(|e| e.into_inner());
        match g.get(&ip) {
            Some((n, since)) if since.elapsed() <= FAILURE_WINDOW => *n >= MAX_FAILURES_PER_WINDOW,
            Some(_) => {
                g.remove(&ip);
                false
            }
            None => false,
        }
    }

    pub fn note_failure(&self, ip: IpAddr) {
        let mut g = self.failures.lock().unwrap_or_else(|e| e.into_inner());
        let entry = g.entry(ip).or_insert((0, Instant::now()));
        if entry.1.elapsed() > FAILURE_WINDOW {
            *entry = (0, Instant::now());
        }
        entry.0 += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn guard_counts_connections_and_cleans_up() {
        let hub = Hub::new();
        let a = hub.try_join("s1").unwrap();
        let b = hub.try_join("s1").unwrap();
        assert_eq!(hub.connected("s1"), 2);
        drop(a);
        assert_eq!(hub.connected("s1"), 1);
        drop(b);
        assert_eq!(hub.connected("s1"), 0);
        assert_eq!(hub.connected("other"), 0);
    }

    #[test]
    fn capacity_is_enforced_per_session() {
        let hub = Hub::new();
        let guards: Vec<_> = (0..config::MAX_STUDENTS).map(|_| hub.try_join("s").unwrap()).collect();
        assert!(hub.try_join("s").is_none());
        assert!(hub.try_join("other").is_some());
        drop(guards);
        assert!(hub.try_join("s").is_some());
    }

    #[test]
    fn repeated_failures_block_only_that_address() {
        let hub = Hub::new();
        let bad: IpAddr = "10.0.0.5".parse().unwrap();
        let good: IpAddr = "10.0.0.6".parse().unwrap();
        for _ in 0..MAX_FAILURES_PER_WINDOW {
            assert!(!hub.is_blocked(bad));
            hub.note_failure(bad);
        }
        assert!(hub.is_blocked(bad));
        assert!(!hub.is_blocked(good));
    }
}
