//! Proctoring events. These are *signals for the teacher*, never verdicts: nothing here blocks,
//! penalises or flags a student automatically. See docs/proctoring.md for what this can and cannot detect.
//!
//! Client-reported: FOCUS_LOST / FOCUS_RESTORED (the exam window lost / regained focus).
//! Server-detected: DISCONNECTED / RECONNECTED (connection gone longer than a grace period),
//! SUBMISSION, TIMEOUT. The client can never report the server-side types.

use std::sync::Arc;

use chrono::Utc;
use serde_json::json;

use crate::config;
use crate::database::Database;
use crate::errors::AppResult;
use crate::models::*;
use crate::server::hub::Hub;
use crate::services::timer;

#[derive(Debug, PartialEq, Eq)]
pub enum ProctorError {
    /// Not an event type clients may report.
    UnknownType,
    Internal,
}

/// Parses a client-reportable event type. Server-side types are deliberately not accepted.
pub fn parse_client_kind(s: &str) -> Result<ProctorEventType, ProctorError> {
    match s {
        "FOCUS_LOST" => Ok(ProctorEventType::FocusLost),
        "FOCUS_RESTORED" => Ok(ProctorEventType::FocusRestored),
        _ => Err(ProctorError::UnknownType),
    }
}

fn short_duration(ms: i64) -> String {
    let s = (ms.max(0) + 500) / 1000;
    if s < 60 { format!("{s} s") } else { format!("{} min {} s", s / 60, s % 60) }
}

/// Records a focus event if it makes sense. `Ok(false)` = deliberately ignored (not an error):
/// wrong state, duplicate, or over the per-attempt cap.
pub async fn record_client_event(db: &Database, session: &ExamSession, attempt_id: &str, kind: ProctorEventType, lost_for_ms: Option<i64>) -> AppResult<bool> {
    let attempt = db.get_attempt(attempt_id).await?;
    if attempt.status != AttemptStatus::InProgress {
        return Ok(false); // not taking the exam (yet / any more)
    }
    // Leaving the window while the clock is stopped is not interesting; coming back is still recorded
    // so a pause never leaves an interval open.
    match (kind, session.status) {
        (ProctorEventType::FocusLost, SessionStatus::Running) => {}
        (ProctorEventType::FocusRestored, SessionStatus::Running | SessionStatus::Paused) => {}
        _ => return Ok(false),
    }
    if db.count_proctor_events(attempt_id).await? >= config::MAX_EVENTS_PER_ATTEMPT {
        return Ok(false);
    }
    let last = db.last_proctor_event(attempt_id, &[ProctorEventType::FocusLost, ProctorEventType::FocusRestored]).await?;
    let away = last.as_ref().is_some_and(|e| e.event_type == ProctorEventType::FocusLost);
    match kind {
        ProctorEventType::FocusLost if away => return Ok(false),
        ProctorEventType::FocusRestored if !away => return Ok(false),
        _ => {}
    }
    match kind {
        ProctorEventType::FocusLost => {
            db.record_proctor_event(attempt_id, kind, "Left the exam window", None).await?;
        }
        _ => {
            let ms = lost_for_ms.map(|v| v.clamp(0, config::MAX_FOCUS_LOST_MS)).unwrap_or(0);
            let meta = json!({ "lostForMs": ms }).to_string();
            db.record_proctor_event(attempt_id, kind, &format!("Returned to the exam window after {}", short_duration(ms)), Some(&meta)).await?;
        }
    }
    Ok(true)
}

/// Runs after a joined connection closes. If the student is still gone after the grace period
/// (and still mid-exam) a DISCONNECTED event is recorded. A quick reconnect leaves no trace.
pub async fn after_disconnect(db: Database, hub: Arc<Hub>, attempt_id: String, session_id: String) {
    tokio::time::sleep(hub.disconnect_grace()).await;
    if hub.is_online(&attempt_id) {
        return;
    }
    let Ok(attempt) = db.get_attempt(&attempt_id).await else { return };
    let Ok(session) = db.get_session(&session_id).await else { return };
    if attempt.status != AttemptStatus::InProgress || session.status != SessionStatus::Running {
        return;
    }
    if let Ok(Some(last)) = db.last_proctor_event(&attempt_id, &[ProctorEventType::Disconnected, ProctorEventType::Reconnected]).await {
        if last.event_type == ProctorEventType::Disconnected {
            return; // already recorded for this outage
        }
    }
    if db.count_proctor_events(&attempt_id).await.unwrap_or(i64::MAX) >= config::MAX_EVENTS_PER_ATTEMPT {
        return;
    }
    let meta = json!({ "graceMs": hub.disconnect_grace().as_millis() as u64 }).to_string();
    if let Err(e) = db.record_proctor_event(&attempt_id, ProctorEventType::Disconnected, "Lost connection to the teacher's computer", Some(&meta)).await {
        tracing::error!(error = %e, "could not record disconnect");
    }
}

/// Called when a student rejoins: closes an open outage with RECONNECTED and how long it lasted.
pub async fn on_rejoin(db: &Database, attempt_id: &str) -> AppResult<()> {
    let Some(last) = db.last_proctor_event(attempt_id, &[ProctorEventType::Disconnected, ProctorEventType::Reconnected]).await? else { return Ok(()) };
    if last.event_type != ProctorEventType::Disconnected {
        return Ok(());
    }
    let offline_ms = timer::parse(&last.created_at).map(|t| (Utc::now() - t).num_milliseconds().max(0)).unwrap_or(0);
    let meta = json!({ "offlineMs": offline_ms }).to_string();
    db.record_proctor_event(attempt_id, ProctorEventType::Reconnected, &format!("Reconnected after {}", short_duration(offline_ms)), Some(&meta)).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clients_may_only_report_focus_events() {
        assert_eq!(parse_client_kind("FOCUS_LOST"), Ok(ProctorEventType::FocusLost));
        assert_eq!(parse_client_kind("FOCUS_RESTORED"), Ok(ProctorEventType::FocusRestored));
        for forbidden in ["DISCONNECTED", "RECONNECTED", "SUBMISSION", "TIMEOUT", "focus_lost", "", "'; DROP TABLE x;--"] {
            assert_eq!(parse_client_kind(forbidden), Err(ProctorError::UnknownType), "{forbidden}");
        }
    }

    #[test]
    fn durations_read_naturally() {
        assert_eq!(short_duration(0), "0 s");
        assert_eq!(short_duration(12_400), "12 s");
        assert_eq!(short_duration(59_600), "1 min 0 s");
        assert_eq!(short_duration(125_000), "2 min 5 s");
        assert_eq!(short_duration(-5), "0 s");
    }
}
