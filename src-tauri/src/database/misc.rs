use super::{ids::*, Database};
use crate::errors::AppResult;
use crate::models::{ProctorEvent, ProctorEventType, Stats};

impl Database {
    pub async fn get_setting(&self, key: &str) -> AppResult<Option<String>> {
        Ok(sqlx::query_scalar("SELECT value FROM application_settings WHERE key = ?").bind(key).fetch_optional(&self.pool).await?)
    }

    pub async fn set_setting(&self, key: &str, value: &str) -> AppResult<()> {
        sqlx::query("INSERT INTO application_settings (key, value) VALUES (?,?) ON CONFLICT(key) DO UPDATE SET value = excluded.value")
            .bind(key).bind(value).execute(&self.pool).await?;
        Ok(())
    }

    /// Never put passwords, hashes or answer keys in `metadata`.
    pub async fn audit(&self, user_id: Option<&str>, action: &str, entity_type: &str, entity_id: Option<&str>, metadata: Option<&str>) -> AppResult<()> {
        sqlx::query("INSERT INTO audit_logs (id, user_id, action, entity_type, entity_id, metadata, created_at) VALUES (?,?,?,?,?,?,?)")
            .bind(new_id()).bind(user_id).bind(action).bind(entity_type).bind(entity_id).bind(metadata).bind(now())
            .execute(&self.pool).await?;
        Ok(())
    }

    pub async fn record_proctor_event(&self, attempt_id: &str, kind: ProctorEventType, description: &str, metadata: Option<&str>) -> AppResult<()> {
        sqlx::query("INSERT INTO proctor_events (id, attempt_id, event_type, description, metadata, created_at) VALUES (?,?,?,?,?,?)")
            .bind(new_id()).bind(attempt_id).bind(kind).bind(description).bind(metadata).bind(now())
            .execute(&self.pool).await?;
        Ok(())
    }

    pub async fn list_proctor_events(&self, attempt_id: &str) -> AppResult<Vec<ProctorEvent>> {
        Ok(sqlx::query_as("SELECT * FROM proctor_events WHERE attempt_id = ? ORDER BY created_at").bind(attempt_id).fetch_all(&self.pool).await?)
    }

    pub async fn stats(&self) -> AppResult<Stats> {
        Ok(sqlx::query_as(
            "SELECT (SELECT COUNT(*) FROM exams) AS exams,
                    (SELECT COUNT(*) FROM exam_sessions WHERE status IN ('WAITING','RUNNING','PAUSED')) AS active_sessions,
                    (SELECT COUNT(*) FROM students) AS students,
                    (SELECT COUNT(*) FROM attempts WHERE status IN ('SUBMITTED','AUTO_SUBMITTED')) AS completed_attempts",
        ).fetch_one(&self.pool).await?)
    }

    pub async fn migrations_applied(&self) -> AppResult<i64> {
        Ok(sqlx::query_scalar("SELECT COUNT(*) FROM _sqlx_migrations WHERE success = 1").fetch_one(&self.pool).await?)
    }
}
