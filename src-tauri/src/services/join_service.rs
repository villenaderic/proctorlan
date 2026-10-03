//! Student join/rejoin. The bearer token proves "this is the same student coming back";
//! only its SHA-256 hash is stored. A second person claiming the same student number is refused.

use sha2::{Digest, Sha256};

use crate::database::Database;
use crate::errors::AppError;
use crate::models::*;

pub const NAME_MAX_LEN: usize = 64;
pub const NUMBER_MAX_LEN: usize = 32;

#[derive(Debug, PartialEq, Eq)]
pub enum JoinError {
    /// Bad name / student number. Message is safe to show the student.
    Invalid(String),
    /// The session no longer accepts students.
    Closed,
    /// That student number already joined and the caller cannot prove it is the same person.
    AlreadyJoined,
    Internal,
}

impl JoinError {
    pub fn code(&self) -> &'static str {
        match self {
            JoinError::Invalid(_) => "invalid_identity",
            JoinError::Closed => "session_closed",
            JoinError::AlreadyJoined => "already_joined",
            JoinError::Internal => "internal",
        }
    }
    pub fn message(&self) -> String {
        match self {
            JoinError::Invalid(m) => m.clone(),
            JoinError::Closed => "This session is no longer accepting students.".into(),
            JoinError::AlreadyJoined => "That student ID has already joined this session. If this is you on a new device, ask your teacher to remove the old entry.".into(),
            JoinError::Internal => "The server hit an error.".into(),
        }
    }
}

pub struct Joined {
    pub attempt: Attempt,
    pub student: Student,
    /// Present only for a brand-new attempt; resuming clients already hold theirs.
    pub new_token: Option<String>,
}

/// Trims and checks the identity the student typed. Student numbers are stored upper-case so
/// `2024-001a` and `2024-001A` cannot become two students.
pub fn validate_identity(name: &str, student_number: &str) -> Result<(String, String), JoinError> {
    let name = name.split_whitespace().collect::<Vec<_>>().join(" ");
    let number = student_number.trim().to_uppercase();
    if name.is_empty() {
        return Err(JoinError::Invalid("Enter your full name.".into()));
    }
    if name.chars().count() > NAME_MAX_LEN || name.chars().any(|c| c.is_control()) {
        return Err(JoinError::Invalid(format!("Your name must be at most {NAME_MAX_LEN} characters.")));
    }
    if number.is_empty() {
        return Err(JoinError::Invalid("Enter your student ID.".into()));
    }
    if number.len() > NUMBER_MAX_LEN || !number.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | '/')) {
        return Err(JoinError::Invalid("Student ID may use letters, numbers, - _ . / and up to 32 characters.".into()));
    }
    Ok((name, number))
}

pub fn hash_token(token: &str) -> String {
    Sha256::digest(token.as_bytes()).iter().map(|b| format!("{b:02x}")).collect()
}

fn new_token() -> String {
    format!("{}{}", uuid::Uuid::new_v4().simple(), uuid::Uuid::new_v4().simple())
}

/// Constant-time-ish comparison of two equal-purpose hashes (both are hex of fixed length).
fn same(a: &str, b: &str) -> bool {
    a.len() == b.len() && a.bytes().zip(b.bytes()).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

pub async fn join(db: &Database, session: &ExamSession, name: &str, number: &str, token: Option<&str>) -> Result<Joined, JoinError> {
    let (name, number) = validate_identity(name, number)?;
    if !session.status.accepts_students() {
        return Err(JoinError::Closed);
    }
    let internal = |e: AppError| {
        tracing::error!(error = %e, "join failed");
        JoinError::Internal
    };

    // Look up an existing attempt without creating or renaming the student first.
    let existing = sqlx::query_as::<_, Student>("SELECT * FROM students WHERE student_number = ?")
        .bind(&number).fetch_optional(db.pool()).await.map_err(|e| internal(e.into()))?;
    if let Some(student) = existing {
        if let Some(attempt) = db.find_attempt(&session.id, &student.id).await.map_err(internal)? {
            return match token {
                Some(t) if same(&hash_token(t), &attempt.token_hash) => Ok(Joined { attempt, student, new_token: None }),
                _ => Err(JoinError::AlreadyJoined),
            };
        }
    }
    let student = db.upsert_student(&number, &name).await.map_err(internal)?;
    let token = new_token();
    match db.create_attempt(&session.id, &student.id, &hash_token(&token)).await {
        Ok(attempt) => Ok(Joined { attempt, student, new_token: Some(token) }),
        // Two simultaneous joins for one student number: the loser is refused.
        Err(AppError::Conflict(_)) => Err(JoinError::AlreadyJoined),
        Err(e) => Err(internal(e)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_is_trimmed_collapsed_and_uppercased() {
        let (n, id) = validate_identity("  Ana   Reyes ", " 2024-001a ").unwrap();
        assert_eq!((n.as_str(), id.as_str()), ("Ana Reyes", "2024-001A"));
    }

    #[test]
    fn bad_identities_are_rejected_with_student_friendly_messages() {
        for (n, i) in [("", "1"), ("  ", "1"), ("Ana", ""), ("Ana", "has space"), ("Ana", "a;b"), ("Ana", &"9".repeat(33)), ("A\u{0007}", "1"), (&"x".repeat(65), "1")] {
            assert!(matches!(validate_identity(n, i), Err(JoinError::Invalid(_))), "{n:?} {i:?}");
        }
    }

    #[test]
    fn token_hash_is_stable_sha256_hex() {
        assert_eq!(hash_token("abc"), "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
        assert_ne!(new_token(), new_token());
        assert_eq!(new_token().len(), 64);
    }
}
