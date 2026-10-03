//! Server-authoritative exam clock. Pure functions over RFC3339 timestamps so they are trivially
//! testable. Clients never supply time; they only receive `remaining_seconds` computed here.

use chrono::{DateTime, Duration, SecondsFormat, Utc};

pub fn parse(ts: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(ts).ok().map(|d| d.with_timezone(&Utc))
}

pub fn format(t: DateTime<Utc>) -> String {
    t.to_rfc3339_opts(SecondsFormat::Millis, true)
}

/// `ends_at` for an exam that starts at `start` and lasts `duration_minutes`.
pub fn ends_at(start: DateTime<Utc>, duration_minutes: i64) -> DateTime<Utc> {
    start + Duration::minutes(duration_minutes)
}

/// Pausing freezes the clock; resuming pushes the deadline back by the time spent paused.
pub fn extend_for_pause(ends_at: DateTime<Utc>, paused_at: DateTime<Utc>, resumed_at: DateTime<Utc>) -> DateTime<Utc> {
    let paused_for = (resumed_at - paused_at).max(Duration::zero());
    ends_at + paused_for
}

/// Whole seconds left (never negative). While paused the clock is frozen at `paused_at`.
pub fn remaining_seconds(ends_at: Option<DateTime<Utc>>, paused_at: Option<DateTime<Utc>>, now: DateTime<Utc>) -> Option<i64> {
    let ends = ends_at?;
    let reference = paused_at.unwrap_or(now);
    Some((ends - reference).num_seconds().max(0))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t(s: &str) -> DateTime<Utc> { parse(s).unwrap() }

    #[test]
    fn ends_at_adds_duration() {
        assert_eq!(ends_at(t("2026-10-03T08:00:00Z"), 60), t("2026-10-03T09:00:00Z"));
    }

    #[test]
    fn remaining_counts_down_and_floors_at_zero() {
        let e = Some(t("2026-10-03T09:00:00Z"));
        assert_eq!(remaining_seconds(e, None, t("2026-10-03T08:30:00Z")), Some(1800));
        assert_eq!(remaining_seconds(e, None, t("2026-10-03T09:00:00Z")), Some(0));
        assert_eq!(remaining_seconds(e, None, t("2026-10-03T10:00:00Z")), Some(0));
        assert_eq!(remaining_seconds(None, None, t("2026-10-03T10:00:00Z")), None);
    }

    #[test]
    fn pause_freezes_the_clock() {
        let e = Some(t("2026-10-03T09:00:00Z"));
        let p = Some(t("2026-10-03T08:40:00Z"));
        assert_eq!(remaining_seconds(e, p, t("2026-10-03T08:55:00Z")), Some(1200));
    }

    #[test]
    fn resume_extends_deadline_by_paused_time() {
        let new_end = extend_for_pause(t("2026-10-03T09:00:00Z"), t("2026-10-03T08:40:00Z"), t("2026-10-03T08:50:00Z"));
        assert_eq!(new_end, t("2026-10-03T09:10:00Z"));
        // clock skew can never shorten the exam
        let same = extend_for_pause(t("2026-10-03T09:00:00Z"), t("2026-10-03T08:50:00Z"), t("2026-10-03T08:40:00Z"));
        assert_eq!(same, t("2026-10-03T09:00:00Z"));
    }

    #[test]
    fn format_round_trips() {
        let x = t("2026-10-03T08:00:00.123Z");
        assert_eq!(parse(&format(x)), Some(x));
        assert!(parse("garbage").is_none());
    }
}
