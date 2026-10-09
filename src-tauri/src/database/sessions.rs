use chrono::{DateTime, Utc};

use super::{ids::*, Database};
use crate::errors::{AppError, AppResult};
use crate::models::{ExamSession, SessionRow, SessionStatus};
use crate::services::{session_code, timer};

const CODE_ATTEMPTS: usize = 10;

/// status, started_at, ends_at, paused_at, paused_total_seconds, exam duration (minutes)
type SessionTimes = (SessionStatus, Option<String>, Option<String>, Option<String>, i64, i64);

impl Database {
    /// Creates a session in CREATED state with a fresh code that is unique among open sessions.
    pub async fn create_session(&self, exam_id: &str, host_ip: &str, host_port: u16) -> AppResult<ExamSession> {
        for _ in 0..CODE_ATTEMPTS {
            let (id, code) = (new_id(), session_code::generate());
            let res = sqlx::query("INSERT INTO exam_sessions (id, exam_id, session_code, status, host_ip, host_port, created_at) VALUES (?,?,?,'CREATED',?,?,?)")
                .bind(&id).bind(exam_id).bind(&code).bind(host_ip).bind(host_port as i64).bind(now())
                .execute(&self.pool).await;
            match res.map_err(AppError::from) {
                Ok(_) => return self.get_session(&id).await,
                Err(AppError::Conflict(_)) => {
                    // Either the code collided (retry) or the exam does not exist (fail).
                    let exam_exists: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM exams WHERE id = ?").bind(exam_id).fetch_one(&self.pool).await?;
                    if exam_exists == 0 {
                        return Err(AppError::NotFound("Exam not found.".into()));
                    }
                }
                Err(e) => return Err(e),
            }
        }
        Err(AppError::Internal("could not allocate a unique session code".into()))
    }

    pub async fn get_session(&self, id: &str) -> AppResult<ExamSession> {
        sqlx::query_as("SELECT * FROM exam_sessions WHERE id = ?").bind(id).fetch_optional(&self.pool).await?
            .ok_or_else(|| AppError::NotFound("Session not found.".into()))
    }

    /// Looks up a joinable (not ended) session by code; input is normalised first.
    pub async fn find_open_session_by_code(&self, code: &str) -> AppResult<Option<ExamSession>> {
        let Some(code) = session_code::normalize(code) else { return Ok(None) };
        Ok(sqlx::query_as("SELECT * FROM exam_sessions WHERE session_code = ? AND status <> 'ENDED'").bind(code).fetch_optional(&self.pool).await?)
    }

    pub async fn list_sessions(&self) -> AppResult<Vec<ExamSession>> {
        Ok(sqlx::query_as("SELECT * FROM exam_sessions ORDER BY created_at DESC").fetch_all(&self.pool).await?)
    }

    pub async fn list_session_rows(&self) -> AppResult<Vec<SessionRow>> {
        Ok(sqlx::query_as(
            "SELECT s.*, e.title AS exam_title FROM exam_sessions s JOIN exams e ON e.id = s.exam_id ORDER BY s.created_at DESC",
        ).fetch_all(&self.pool).await?)
    }

    pub async fn get_session_row(&self, id: &str) -> AppResult<SessionRow> {
        sqlx::query_as("SELECT s.*, e.title AS exam_title FROM exam_sessions s JOIN exams e ON e.id = s.exam_id WHERE s.id = ?")
            .bind(id).fetch_optional(&self.pool).await?
            .ok_or_else(|| AppError::NotFound("Session not found.".into()))
    }

    /// RUNNING sessions whose deadline has passed (paused sessions keep their clock frozen).
    pub async fn list_expired_running_sessions(&self, now: DateTime<Utc>) -> AppResult<Vec<ExamSession>> {
        let all: Vec<ExamSession> = sqlx::query_as("SELECT * FROM exam_sessions WHERE status = 'RUNNING' AND ends_at IS NOT NULL").fetch_all(&self.pool).await?;
        Ok(all.into_iter().filter(|s| s.ends_at.as_deref().and_then(timer::parse).is_some_and(|e| e <= now)).collect())
    }

    pub async fn transition_session(&self, id: &str, next: SessionStatus) -> AppResult<ExamSession> {
        self.transition_session_at(id, next, Utc::now()).await
    }

    /// Validates the lifecycle transition and applies the server-authoritative timer rules
    /// (start sets the deadline, pause freezes it, resume pushes it back) in one transaction.
    pub async fn transition_session_at(&self, id: &str, next: SessionStatus, now: DateTime<Utc>) -> AppResult<ExamSession> {
        let mut tx = self.pool.begin().await?;
        let row: Option<SessionTimes> = sqlx::query_as(
            "SELECT s.status, s.started_at, s.ends_at, s.paused_at, s.paused_total_seconds, e.duration_minutes
               FROM exam_sessions s JOIN exams e ON e.id = s.exam_id WHERE s.id = ?",
        ).bind(id).fetch_optional(&mut *tx).await?;
        let (current, started_at, ends_at, paused_at, paused_total, duration) =
            row.ok_or_else(|| AppError::NotFound("Session not found.".into()))?;
        if !current.can_transition_to(next) {
            return Err(AppError::Conflict(format!("Cannot change session from {current:?} to {next:?}.")));
        }

        let (mut started_at, mut ends_at, mut paused_at, mut paused_total) = (started_at, ends_at, paused_at, paused_total);
        let stamp = timer::format(now);
        match (current, next) {
            (SessionStatus::Waiting, SessionStatus::Running) => {
                started_at = Some(stamp.clone());
                ends_at = Some(timer::format(timer::ends_at(now, duration)));
            }
            (SessionStatus::Running, SessionStatus::Paused) => paused_at = Some(stamp.clone()),
            (SessionStatus::Paused, SessionStatus::Running) => {
                if let (Some(e), Some(p)) = (ends_at.as_deref().and_then(timer::parse), paused_at.as_deref().and_then(timer::parse)) {
                    ends_at = Some(timer::format(timer::extend_for_pause(e, p, now)));
                    paused_total += (now - p).num_seconds().max(0);
                }
                paused_at = None;
            }
            _ => {}
        }
        let ended_at = (next == SessionStatus::Ended).then(|| stamp.clone());
        sqlx::query(
            "UPDATE exam_sessions SET status = ?, started_at = ?, ends_at = ?, paused_at = ?, paused_total_seconds = ?,
               ended_at = COALESCE(?, ended_at) WHERE id = ?",
        )
        .bind(next).bind(started_at).bind(ends_at).bind(paused_at).bind(paused_total).bind(ended_at).bind(id)
        .execute(&mut *tx).await?;
        tx.commit().await?;
        self.get_session(id).await
    }
}
