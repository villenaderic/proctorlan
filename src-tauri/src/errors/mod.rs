//! Application error type. Serialises to a user-safe string for the frontend;
//! the detailed cause is logged, never sent to the UI or to students.

use serde::{Serialize, Serializer};

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("{0}")]
    Internal(String),
}

pub type AppResult<T> = Result<T, AppError>;

impl Serialize for AppError {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        tracing::error!(error = %self, "command failed");
        let user_msg = match self {
            AppError::Io(_) => "A file system error occurred.",
            AppError::Internal(_) => "An unexpected error occurred.",
        };
        s.serialize_str(user_msg)
    }
}
