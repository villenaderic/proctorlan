//! Application error type. Serialises to a user-safe string for the frontend;
//! the detailed cause is logged, never sent to the UI or to students.

use serde::{Serialize, Serializer};

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("database error: {0}")]
    Db(sqlx::Error),
    #[error("migration error: {0}")]
    Migrate(#[from] sqlx::migrate::MigrateError),
    #[error("{0}")]
    NotFound(String),
    #[error("{0}")]
    Conflict(String),
    #[error("{0}")]
    Validation(String),
    #[error("{0}")]
    Internal(String),
}

pub type AppResult<T> = Result<T, AppError>;

impl From<sqlx::Error> for AppError {
    /// Maps constraint violations to meaningful errors instead of opaque DB failures.
    fn from(e: sqlx::Error) -> Self {
        if let sqlx::Error::Database(db) = &e {
            if db.is_unique_violation() {
                return AppError::Conflict("That record already exists.".into());
            }
            if db.is_foreign_key_violation() {
                return AppError::Conflict("That record refers to data that does not exist or is still in use.".into());
            }
            if db.is_check_violation() {
                return AppError::Validation("A value was outside the allowed range.".into());
            }
        }
        AppError::Db(e)
    }
}

impl Serialize for AppError {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        tracing::error!(error = %self, "command failed");
        let msg = match self {
            AppError::Io(_) => "A file system error occurred.".to_string(),
            AppError::Db(_) | AppError::Migrate(_) => "A database error occurred.".to_string(),
            AppError::Internal(_) => "An unexpected error occurred.".to_string(),
            AppError::NotFound(m) | AppError::Conflict(m) | AppError::Validation(m) => m.clone(),
        };
        s.serialize_str(&msg)
    }
}
