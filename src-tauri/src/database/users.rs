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

    /// Creates the first administrator only if no user exists yet — atomic, so two racing
    /// setups can never both succeed. Returns `None` when setup was already completed.
    pub async fn create_first_admin(&self, username: &str, password_hash: &str, display_name: &str) -> AppResult<Option<User>> {
        let (id, ts) = (new_id(), now());
        let res = sqlx::query(
            "INSERT INTO users (id, username, password_hash, display_name, role, created_at, updated_at)
             SELECT ?,?,?,?,'admin',?,? WHERE NOT EXISTS (SELECT 1 FROM users)",
        )
        .bind(&id).bind(username.trim()).bind(password_hash).bind(display_name.trim()).bind(&ts).bind(&ts)
        .execute(&self.pool).await?;
        if res.rows_affected() == 0 {
            return Ok(None);
        }
        Ok(Some(self.get_user(&id).await?))
    }

    pub async fn get_user(&self, id: &str) -> AppResult<User> {
        sqlx::query_as("SELECT * FROM users WHERE id = ?").bind(id).fetch_optional(&self.pool).await?
            .ok_or_else(|| crate::errors::AppError::NotFound("User not found.".into()))
    }

    pub async fn update_password_hash(&self, id: &str, password_hash: &str) -> AppResult<()> {
        sqlx::query("UPDATE users SET password_hash = ?, updated_at = ? WHERE id = ?").bind(password_hash).bind(now()).bind(id).execute(&self.pool).await?;
        Ok(())
    }

    pub async fn update_display_name(&self, id: &str, display_name: &str) -> AppResult<()> {
        sqlx::query("UPDATE users SET display_name = ?, updated_at = ? WHERE id = ?").bind(display_name.trim()).bind(now()).bind(id).execute(&self.pool).await?;
        Ok(())
    }
}
