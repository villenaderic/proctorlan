//! Teacher-side session workflows: create, open, start, pause, resume, end, plus live snapshots.

use std::sync::Arc;

use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::database::Database;
use crate::errors::{AppError, AppResult};
use crate::models::*;
use crate::server::hub::Hub;
use crate::server::SharedStatus;
use crate::services::exam_engine;
use crate::services::exam_validation::validate_exam;
use crate::services::timer;
use crate::websocket::protocol::{server as msg, Envelope};

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SessionAction {
    Start,
    Pause,
    Resume,
    End,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionSnapshot {
    #[serde(flatten)]
    pub row: SessionRow,
    pub question_count: i64,
    pub passing_score: f64,
    /// Students who have joined (have an attempt row).
    pub joined: i64,
    /// Students with a live WebSocket right now.
    pub connected: usize,
    /// Joined students with a live connection (what the teacher cares about).
    pub online: usize,
    pub submitted: i64,
    pub remaining_seconds: Option<i64>,
    pub server_time: String,
}

pub struct SessionService {
    db: Database,
    hub: Arc<Hub>,
    server: SharedStatus,
}

impl SessionService {
    pub fn new(db: Database, hub: Arc<Hub>, server: SharedStatus) -> Self {
        Self { db, hub, server }
    }

    /// Creates a session for an active, complete exam and opens it for students (WAITING).
    pub async fn create(&self, user: &User, exam_id: &str) -> AppResult<SessionSnapshot> {
        let (running, ip, port) = {
            let s = self.server.read().unwrap_or_else(|e| e.into_inner());
            (s.running, s.ip.clone(), s.port)
        };
        if !running {
            return Err(AppError::Conflict("The LAN server is not running, so students cannot connect. Check Settings → Network.".into()));
        }
        let exam = self.db.get_exam_full(exam_id).await?;
        if exam.exam.status != ExamStatus::Active {
            return Err(AppError::Conflict("Activate the exam in the exam list before starting a session.".into()));
        }
        if let Some(issue) = validate_exam(&NewExam::from(&exam), true).first() {
            return Err(AppError::Validation(format!("This exam is not ready to run. {}", issue.message)));
        }
        let created = self.db.create_session(exam_id, &ip, port).await?;
        let session = self.db.transition_session(&created.id, SessionStatus::Waiting).await?;
        self.db.audit(Some(&user.id), "session.create", "session", Some(&session.id), None).await?;
        tracing::info!(session_id = %session.id, "session created and open for students");
        self.snapshot(&session.id).await
    }

    pub async fn act(&self, user: &User, id: &str, action: SessionAction) -> AppResult<SessionSnapshot> {
        let target = match action {
            SessionAction::Start => SessionStatus::Running,
            SessionAction::Pause => SessionStatus::Paused,
            SessionAction::Resume => SessionStatus::Running,
            SessionAction::End => SessionStatus::Ended,
        };
        let before = self.db.get_session(id).await?;
        // Resume is only valid from PAUSED, Start only from WAITING: reject mix-ups clearly.
        match (action, before.status) {
            (SessionAction::Start, s) if s != SessionStatus::Waiting => {
                return Err(AppError::Conflict("Only a session that is waiting can be started.".into()))
            }
            (SessionAction::Resume, s) if s != SessionStatus::Paused => {
                return Err(AppError::Conflict("Only a paused session can be resumed.".into()))
            }
            _ => {}
        }
        let after = self.db.transition_session(id, target).await?;
        let audit = match action {
            SessionAction::Start => "session.start",
            SessionAction::Pause => "session.pause",
            SessionAction::Resume => "session.resume",
            SessionAction::End => "session.end",
        };
        self.db.audit(Some(&user.id), audit, "session", Some(id), None).await?;
        tracing::info!(session_id = %id, action = audit, "session state changed");

        // Ending closes and grades everyone still working, before students hear the session is over.
        if action == SessionAction::End {
            let n = exam_engine::finalize_session_attempts(&self.db, &self.hub, &after, exam_engine::AutoReason::SessionEnded).await?;
            tracing::info!(session_id = %id, auto_submitted = n, "session ended");
        }

        let snap = self.snapshot(id).await?;
        let kind = match action {
            SessionAction::Start => msg::SESSION_STARTED,
            SessionAction::Pause => msg::SESSION_PAUSED,
            SessionAction::Resume => msg::SESSION_RESUMED,
            SessionAction::End => msg::SESSION_ENDED,
        };
        self.hub.publish(Envelope::new(kind, Some(id), json!({
            "status": after.status,
            "endsAt": after.ends_at,
            "remainingSeconds": snap.remaining_seconds,
            "serverTime": snap.server_time,
        })));
        Ok(snap)
    }

    pub async fn list(&self) -> AppResult<Vec<SessionRow>> {
        self.db.list_session_rows().await
    }

    pub async fn snapshot(&self, id: &str) -> AppResult<SessionSnapshot> {
        let row = self.db.get_session_row(id).await?;
        let exam = self.db.get_exam_full(&row.session.exam_id).await?;
        let now = Utc::now();
        let remaining = timer::remaining_seconds(
            row.session.ends_at.as_deref().and_then(timer::parse),
            row.session.paused_at.as_deref().and_then(timer::parse),
            now,
        );
        Ok(SessionSnapshot {
            question_count: exam.questions.len() as i64,
            passing_score: exam.exam.passing_score,
            joined: self.db.count_attempts_in_session(id).await?,
            connected: self.hub.connected(id),
            online: self.hub.online_in_session(id),
            submitted: self.db.count_submitted_in_session(id).await?,
            remaining_seconds: remaining,
            server_time: timer::format(now),
            row,
        })
    }

    /// Newest-first proctoring feed for the teacher (capped).
    pub async fn events(&self, id: &str) -> AppResult<Vec<SessionEvent>> {
        self.db.list_session_events(id, 500).await
    }

    pub async fn roster(&self, id: &str) -> AppResult<Vec<RosterRow>> {
        let mut rows = self.db.list_roster(id).await?;
        for r in &mut rows {
            r.online = self.hub.is_online(&r.attempt_id);
        }
        Ok(rows)
    }

    /// Lets a student who has not started yet join again (typo, lost token, new device).
    pub async fn remove_student(&self, user: &User, attempt_id: &str) -> AppResult<()> {
        let session_id = self.db.remove_unstarted_attempt(attempt_id).await?;
        self.db.audit(Some(&user.id), "student.remove", "attempt", Some(attempt_id), None).await?;
        self.hub.publish(Envelope::new(msg::REMOVED, Some(&session_id), json!({ "attemptId": attempt_id })));
        Ok(())
    }
}
