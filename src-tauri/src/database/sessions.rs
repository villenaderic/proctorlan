use super::{ids::*, Database};
use crate::errors::{AppError, AppResult};
use crate::models::{ExamSession, SessionStatus};
use crate::services::session_code;

const CODE_ATTEMPTS: usize = 10;

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

    /// Validates the lifecycle transition inside a transaction. Timer fields (`ends_at`, pauses)
    /// are owned by the exam engine (Phase 7); this only records status and start/end stamps.
    pub async fn transition_session(&self, id: &str, next: SessionStatus) -> AppResult<ExamSession> {
        let mut tx = self.pool.begin().await?;
        let current: SessionStatus = sqlx::query_scalar("SELECT status FROM exam_sessions WHERE id = ?").bind(id).fetch_optional(&mut *tx).await?
            .ok_or_else(|| AppError::NotFound("Session not found.".into()))?;
        if !current.can_transition_to(next) {
            return Err(AppError::Conflict(format!("Cannot change session from {current:?} to {next:?}.")));
        }
        let ts = now();
        sqlx::query(
            "UPDATE exam_sessions SET status = ?,
               started_at = CASE WHEN ? = 'RUNNING' AND started_at IS NULL THEN ? ELSE started_at END,
               ended_at   = CASE WHEN ? = 'ENDED' THEN ? ELSE ended_at END
             WHERE id = ?",
        )
        .bind(next).bind(next).bind(&ts).bind(next).bind(&ts).bind(id)
        .execute(&mut *tx).await?;
        tx.commit().await?;
        self.get_session(id).await
    }
}
