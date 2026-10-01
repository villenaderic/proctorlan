use super::{ids::*, Database};
use crate::errors::AppResult;
use crate::models::{User, UserRole};

impl Database {
    pub async fn count_users(&self) -> AppResult<i64> {
        Ok(sqlx::query_scalar("SELECT COUNT(*) FROM users").fetch_one(&self.pool).await?)
    }

    /// `password_hash` must already be an Argon2 hash (Phase 3); plaintext never reaches this layer.
    pub async fn create_user(&self, username: &str, password_hash: &str, display_name: &str, role: UserRole) -> AppResult<User> {
        let (id, ts) = (new_id(), now());
        sqlx::query("INSERT INTO users (id, username, password_hash, display_name, role, created_at, updated_at) VALUES (?,?,?,?,?,?,?)")
            .bind(&id).bind(username.trim()).bind(password_hash).bind(display_name.trim()).bind(role).bind(&ts).bind(&ts)
            .execute(&self.pool).await?;
        Ok(sqlx::query_as("SELECT * FROM users WHERE id = ?").bind(&id).fetch_one(&self.pool).await?)
    }

    pub async fn find_user_by_username(&self, username: &str) -> AppResult<Option<User>> {
        Ok(sqlx::query_as("SELECT * FROM users WHERE username = ?").bind(username.trim()).fetch_optional(&self.pool).await?)
    }
}
