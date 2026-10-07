use super::Database;
use crate::errors::AppResult;
use crate::models::*;

impl Database {
    pub async fn list_result_sessions(&self) -> AppResult<Vec<ResultSessionRow>> {
        Ok(sqlx::query_as(
            "SELECT es.id AS session_id, e.title AS exam_title, es.session_code, es.status, es.created_at, es.ended_at,
                    (SELECT COUNT(*) FROM attempts a WHERE a.session_id = es.id) AS joined,
                    (SELECT COUNT(*) FROM attempts a WHERE a.session_id = es.id AND a.status IN ('SUBMITTED','AUTO_SUBMITTED')) AS submitted,
                    (SELECT COUNT(*) FROM attempts a WHERE a.session_id = es.id AND a.passed = 1) AS passed,
                    (SELECT AVG(a.percentage) FROM attempts a WHERE a.session_id = es.id AND a.status IN ('SUBMITTED','AUTO_SUBMITTED')) AS average_percentage
             FROM exam_sessions es JOIN exams e ON e.id = es.exam_id
             WHERE EXISTS (SELECT 1 FROM attempts a WHERE a.session_id = es.id)
             ORDER BY es.created_at DESC",
        ).fetch_all(&self.pool).await?)
    }

    pub async fn list_result_rows(&self, session_id: &str) -> AppResult<Vec<ResultRow>> {
        Ok(sqlx::query_as(
            "SELECT a.id AS attempt_id, s.student_number, s.name, a.status, a.started_at, a.submitted_at,
                    a.score, a.total_points, a.percentage, a.passed,
                    (SELECT COUNT(*) FROM answers w WHERE w.attempt_id = a.id AND w.answer_data NOT IN ('\"\"', '[]')) AS answered,
                    CASE WHEN a.started_at IS NOT NULL AND a.submitted_at IS NOT NULL
                         THEN CAST(ROUND((julianday(a.submitted_at) - julianday(a.started_at)) * 86400.0) AS INTEGER) END AS time_taken_seconds,
                    (SELECT COUNT(*) FROM proctor_events p WHERE p.attempt_id = a.id AND p.event_type = 'FOCUS_LOST') AS focus_lost_count,
                    (SELECT COALESCE(SUM(CAST(json_extract(p.metadata, '$.lostForMs') AS INTEGER)), 0) FROM proctor_events p WHERE p.attempt_id = a.id AND p.event_type = 'FOCUS_RESTORED') AS focus_lost_ms,
                    (SELECT COUNT(*) FROM proctor_events p WHERE p.attempt_id = a.id AND p.event_type = 'DISCONNECTED') AS disconnect_count
             FROM attempts a JOIN students s ON s.id = a.student_id
             WHERE a.session_id = ? ORDER BY s.name COLLATE NOCASE, s.student_number",
        ).bind(session_id).fetch_all(&self.pool).await?)
    }

    /// Every stored answer of the session's attempts (the service filters to submitted ones).
    pub async fn list_session_answers(&self, session_id: &str) -> AppResult<Vec<Answer>> {
        Ok(sqlx::query_as("SELECT w.* FROM answers w JOIN attempts a ON a.id = w.attempt_id WHERE a.session_id = ?")
            .bind(session_id).fetch_all(&self.pool).await?)
    }

    pub async fn list_student_summaries(&self) -> AppResult<Vec<StudentSummary>> {
        Ok(sqlx::query_as(
            "SELECT s.id, s.student_number, s.name,
                    COUNT(a.id) AS attempts,
                    AVG(CASE WHEN a.status IN ('SUBMITTED','AUTO_SUBMITTED') THEN a.percentage END) AS average_percentage,
                    MAX(a.submitted_at) AS last_attempt_at
             FROM students s LEFT JOIN attempts a ON a.student_id = s.id
             GROUP BY s.id ORDER BY s.name COLLATE NOCASE, s.student_number",
        ).fetch_all(&self.pool).await?)
    }

    pub async fn list_student_attempts(&self, student_id: &str) -> AppResult<Vec<StudentAttemptRow>> {
        Ok(sqlx::query_as(
            "SELECT a.id AS attempt_id, a.session_id, e.title AS exam_title, a.status, a.submitted_at,
                    a.score, a.total_points, a.percentage, a.passed, a.created_at
             FROM attempts a JOIN exam_sessions es ON es.id = a.session_id JOIN exams e ON e.id = es.exam_id
             WHERE a.student_id = ? ORDER BY a.created_at DESC",
        ).bind(student_id).fetch_all(&self.pool).await?)
    }
}
