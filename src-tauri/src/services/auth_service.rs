//! Teacher account workflows. Commands stay thin; everything testable lives here.

use serde::Serialize;

use crate::auth::password::{dummy_hash, hash_password, verify_password};
use crate::auth::policy::{validate_display_name, validate_password, validate_username};
use crate::auth::session::{Limits, SessionState};
use crate::database::Database;
use crate::errors::{AppError, AppResult};
use crate::models::User;

const BAD_CREDENTIALS: &str = "Invalid username or password.";
pub const SESSION_EXPIRED: &str = "Your session has expired. Please sign in again.";

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthStatus {
    pub setup_required: bool,
    pub user: Option<User>,
}

pub struct AuthService {
    db: Database,
    state: SessionState,
}

/// Argon2 is CPU-heavy; keep it off the async executor threads.
async fn blocking<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> AppResult<T> {
    tokio::task::spawn_blocking(f).await.map_err(|e| AppError::Internal(format!("background task failed: {e}")))
}

impl AuthService {
    pub fn new(db: Database) -> Self {
        Self::with_limits(db, Limits::default())
    }

    pub fn with_limits(db: Database, limits: Limits) -> Self {
        Self { db, state: SessionState::new(limits) }
    }

    pub async fn status(&self) -> AppResult<AuthStatus> {
        let setup_required = self.db.count_users().await? == 0;
        let user = match self.state.active_user() {
            Some(id) => self.db.get_user(&id).await.ok(),
            None => None,
        };
        Ok(AuthStatus { setup_required, user })
    }

    /// First-launch "Create Administrator Account". Succeeds exactly once; signs the new admin in.
    pub async fn setup_admin(&self, username: &str, password: &str, display_name: &str) -> AppResult<User> {
        let username = validate_username(username)?;
        validate_password(password, &username)?;
        let display_name = validate_display_name(display_name)?;
        let pw = password.to_string();
        let hash = blocking(move || hash_password(&pw)).await??;
        let user = self.db.create_first_admin(&username, &hash, &display_name).await?
            .ok_or_else(|| AppError::Conflict("Setup has already been completed. Please sign in.".into()))?;
        self.state.sign_in(&user.id);
        self.db.audit(Some(&user.id), "auth.setup_admin", "user", Some(&user.id), None).await?;
        tracing::info!("administrator account created");
        Ok(user)
    }

    pub async fn login(&self, username: &str, password: &str) -> AppResult<User> {
        let key = username.trim().to_ascii_lowercase();
        if let Some(wait) = self.state.lockout_remaining(&key) {
            return Err(AppError::Unauthorized(format!("Too many failed attempts. Try again in {} seconds.", wait.as_secs() + 1)));
        }
        let user = self.db.find_user_by_username(username).await?;
        let stored = user.as_ref().map(|u| u.password_hash.clone());
        let pw = password.to_string();
        let ok = blocking(move || verify_password(&pw, stored.as_deref().unwrap_or_else(|| dummy_hash()))).await?;
        match user {
            Some(u) if ok => {
                self.state.clear_failures(&key);
                self.state.sign_in(&u.id);
                self.db.audit(Some(&u.id), "auth.login", "user", Some(&u.id), None).await?;
                tracing::info!("teacher signed in");
                Ok(u)
            }
            other => {
                self.state.record_failure(&key);
                // Log the account only if it exists; never log what was typed (it may be a password).
                self.db.audit(other.as_ref().map(|u| u.id.as_str()), "auth.login_failed", "user", other.as_ref().map(|u| u.id.as_str()), None).await?;
                tracing::warn!("failed login attempt");
                Err(AppError::Unauthorized(BAD_CREDENTIALS.into()))
            }
        }
    }

    pub async fn logout(&self) -> AppResult<()> {
        if let Some(id) = self.state.active_user() {
            self.db.audit(Some(&id), "auth.logout", "user", Some(&id), None).await?;
        }
        self.state.sign_out();
        Ok(())
    }

    /// Gate for every teacher-only command.
    pub async fn require_user(&self) -> AppResult<User> {
        let id = self.state.active_user().ok_or_else(|| AppError::Unauthorized(SESSION_EXPIRED.into()))?;
        self.db.get_user(&id).await.map_err(|_| {
            self.state.sign_out();
            AppError::Unauthorized(SESSION_EXPIRED.into())
        })
    }

    pub async fn change_password(&self, current: &str, new: &str) -> AppResult<()> {
        let user = self.require_user().await?;
        validate_password(new, &user.username)?;
        let (cur, stored) = (current.to_string(), user.password_hash.clone());
        if !blocking(move || verify_password(&cur, &stored)).await? {
            return Err(AppError::Validation("Current password is incorrect.".into()));
        }
        let pw = new.to_string();
        let hash = blocking(move || hash_password(&pw)).await??;
        self.db.update_password_hash(&user.id, &hash).await?;
        self.db.audit(Some(&user.id), "auth.password_changed", "user", Some(&user.id), None).await?;
        Ok(())
    }

    pub async fn update_display_name(&self, display_name: &str) -> AppResult<User> {
        let user = self.require_user().await?;
        let name = validate_display_name(display_name)?;
        self.db.update_display_name(&user.id, &name).await?;
        self.db.get_user(&user.id).await
    }
}
