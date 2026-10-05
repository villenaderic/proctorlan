//! Per-connection message budget. Protects the teacher's computer from a buggy or hostile client
//! flooding one socket. Counts in one-second windows; the clock is passed in so it is testable.

use std::time::{Duration, Instant};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verdict {
    Allow,
    /// Over the soft limit: refuse this message, keep the connection.
    Throttle,
    /// Over the hard limit: drop the connection.
    Close,
}

pub struct RateLimiter {
    soft: u32,
    hard: u32,
    window_start: Instant,
    count: u32,
}

impl RateLimiter {
    pub fn new(soft: u32, hard: u32, now: Instant) -> Self {
        Self { soft, hard, window_start: now, count: 0 }
    }

    pub fn check(&mut self, now: Instant) -> Verdict {
        if now.duration_since(self.window_start) >= Duration::from_secs(1) {
            self.window_start = now;
            self.count = 0;
        }
        self.count += 1;
        if self.count > self.hard {
            Verdict::Close
        } else if self.count > self.soft {
            Verdict::Throttle
        } else {
            Verdict::Allow
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allows_normal_use_throttles_bursts_and_closes_floods() {
        let t0 = Instant::now();
        let mut l = RateLimiter::new(3, 5, t0);
        assert_eq!((0..3).map(|_| l.check(t0)).collect::<Vec<_>>(), vec![Verdict::Allow; 3]);
        assert_eq!(l.check(t0), Verdict::Throttle);
        assert_eq!(l.check(t0), Verdict::Throttle);
        assert_eq!(l.check(t0), Verdict::Close);
    }

    #[test]
    fn the_budget_refills_every_second() {
        let t0 = Instant::now();
        let mut l = RateLimiter::new(2, 4, t0);
        l.check(t0);
        l.check(t0);
        assert_eq!(l.check(t0), Verdict::Throttle);
        assert_eq!(l.check(t0 + Duration::from_millis(999)), Verdict::Throttle);
        assert_eq!(l.check(t0 + Duration::from_millis(1000)), Verdict::Allow);
    }
}
