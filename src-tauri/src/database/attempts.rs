use super::{ids::*, Database};
use crate::errors::{AppError, AppResult};
use crate::models::{Answer, Attempt, AttemptStatus, Student};

impl Database {
    /// Creates the student or refreshes the display name for an existing student number.
    pub async fn upsert_student(&self, student_number: &str, name: &str) -> AppResult<Student> {
        sqlx::query("INSERT INTO students (id, student_number, name, created_at) VALUES (?,?,?,?)
                     ON CONFLICT(student_number) DO UPDATE SET name = excluded.name")
            .bind(new_id()).bind(student_number.trim()).bind(name.trim()).bind(now())
            .execute(&self.pool).await?;
        Ok(sqlx::query_as("SELECT * FROM students WHERE student_number = ?").bind(student_number.trim()).fetch_one(&self.pool).await?)
    }

    /// One attempt per (session, student): a second call fails with Conflict.
    pub async fn create_attempt(&self, session_id: &str, student_id: &str, token_hash: &str) -> AppResult<Attempt> {
        let id = new_id();
        sqlx::query("INSERT INTO attempts (id, session_id, student_id, status, token_hash, created_at) VALUES (?,?,?,'JOINED',?,?)")
            .bind(&id).bind(session_id).bind(student_id).bind(token_hash).bind(now())
            .execute(&self.pool).await?;
        self.get_attempt(&id).await
    }

    pub async fn get_attempt(&self, id: &str) -> AppResult<Attempt> {
        sqlx::query_as("SELECT * FROM attempts WHERE id = ?").bind(id).fetch_optional(&self.pool).await?
            .ok_or_else(|| AppError::NotFound("Attempt not found.".into()))
    }

    pub async fn find_attempt(&self, session_id: &str, student_id: &str) -> AppResult<Option<Attempt>> {
        Ok(sqlx::query_as("SELECT * FROM attempts WHERE session_id = ? AND student_id = ?").bind(session_id).bind(student_id).fetch_optional(&self.pool).await?)
    }

    pub async fn find_attempt_by_token_hash(&self, token_hash: &str) -> AppResult<Option<Attempt>> {
        Ok(sqlx::query_as("SELECT * FROM attempts WHERE token_hash = ?").bind(token_hash).fetch_optional(&self.pool).await?)
    }

    pub async fn set_attempt_status(&self, id: &str, status: AttemptStatus) -> AppResult<()> {
        let res = sqlx::query("UPDATE attempts SET status = ?, started_at = CASE WHEN ? = 'IN_PROGRESS' AND started_at IS NULL THEN ? ELSE started_at END WHERE id = ?")
            .bind(status).bind(status).bind(now()).bind(id).execute(&self.pool).await?;
        if res.rows_affected() == 0 {
            return Err(AppError::NotFound("Attempt not found.".into()));
        }
        Ok(())
    }

    /// Idempotent answer save. Returns `true` if stored, `false` if ignored because a newer or
    /// equal `client_seq` is already saved (duplicate or out-of-order retry). Rejects answers for
    /// questions outside the attempt's exam, malformed JSON, and finished attempts.
    pub async fn upsert_answer(&self, attempt_id: &str, question_id: &str, answer_data: &str, client_seq: i64) -> AppResult<bool> {
        serde_json::from_str::<serde_json::Value>(answer_data).map_err(|_| AppError::Validation("Answer is not valid JSON.".into()))?;
        let mut tx = self.pool.begin().await?;
        let status: Option<AttemptStatus> = sqlx::query_scalar(
            "SELECT a.status FROM attempts a
               JOIN exam_sessions s ON s.id = a.session_id
               JOIN questions q ON q.exam_id = s.exam_id
              WHERE a.id = ? AND q.id = ?",
        ).bind(attempt_id).bind(question_id).fetch_optional(&mut *tx).await?;
        match status {
            None => return Err(AppError::Validation("That question does not belong to this exam.".into())),
            Some(s) if s.is_final() => return Err(AppError::Conflict("This exam has already been submitted.".into())),
            Some(_) => {}
        }
        let ts = now();
        let res = sqlx::query(
            "INSERT INTO answers (id, attempt_id, question_id, answer_data, client_seq, answered_at, updated_at)
             VALUES (?,?,?,?,?,?,?)
             ON CONFLICT(attempt_id, question_id) DO UPDATE SET
               answer_data = excluded.answer_data, client_seq = excluded.client_seq, updated_at = excluded.updated_at
             WHERE excluded.client_seq > answers.client_seq",
        )
        .bind(new_id()).bind(attempt_id).bind(question_id).bind(answer_data).bind(client_seq).bind(&ts).bind(&ts)
        .execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(res.rows_affected() > 0)
    }

    pub async fn list_answers(&self, attempt_id: &str) -> AppResult<Vec<Answer>> {
        Ok(sqlx::query_as("SELECT * FROM answers WHERE attempt_id = ? ORDER BY answered_at").bind(attempt_id).fetch_all(&self.pool).await?)
    }

    pub async fn count_answers(&self, attempt_id: &str) -> AppResult<i64> {
        Ok(sqlx::query_scalar("SELECT COUNT(*) FROM answers WHERE attempt_id = ?").bind(attempt_id).fetch_one(&self.pool).await?)
    }

    pub async fn count_attempts_in_session(&self, session_id: &str) -> AppResult<i64> {
        Ok(sqlx::query_scalar("SELECT COUNT(*) FROM attempts WHERE session_id = ?").bind(session_id).fetch_one(&self.pool).await?)
    }

    pub async fn count_submitted_in_session(&self, session_id: &str) -> AppResult<i64> {
        Ok(sqlx::query_scalar("SELECT COUNT(*) FROM attempts WHERE session_id = ? AND status IN ('SUBMITTED','AUTO_SUBMITTED')").bind(session_id).fetch_one(&self.pool).await?)
    }
}
