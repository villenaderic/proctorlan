//! Input validation for account fields. Frontend mirrors these rules for instant feedback,
//! but this is the authoritative check.

use crate::config::*;
use crate::errors::{AppError, AppResult};

fn invalid<T>(msg: impl Into<String>) -> AppResult<T> {
    Err(AppError::Validation(msg.into()))
}

/// Returns the trimmed username if valid: 3–32 chars of letters, digits, `.`, `_`, `-`.
pub fn validate_username(raw: &str) -> AppResult<String> {
    let u = raw.trim();
    let len = u.chars().count();
    if !(USERNAME_MIN_LEN..=USERNAME_MAX_LEN).contains(&len) {
        return invalid(format!("Username must be {USERNAME_MIN_LEN}–{USERNAME_MAX_LEN} characters."));
    }
    if !u.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-')) {
        return invalid("Username may only contain letters, numbers, dots, dashes and underscores.");
    }
    Ok(u.to_string())
}

/// Passwords are not trimmed (spaces are legitimate); length is counted in characters.
pub fn validate_password(password: &str, username: &str) -> AppResult<()> {
    let len = password.chars().count();
    if len < MIN_PASSWORD_LEN {
        return invalid(format!("Password must be at least {MIN_PASSWORD_LEN} characters."));
    }
    if len > MAX_PASSWORD_LEN {
        return invalid(format!("Password must be at most {MAX_PASSWORD_LEN} characters."));
    }
    if password.eq_ignore_ascii_case(username.trim()) {
        return invalid("Password must not be the same as the username.");
    }
    Ok(())
}

pub fn validate_display_name(raw: &str) -> AppResult<String> {
    let n = raw.trim();
    if n.is_empty() || n.chars().count() > DISPLAY_NAME_MAX_LEN {
        return invalid(format!("Display name must be 1–{DISPLAY_NAME_MAX_LEN} characters."));
    }
    if n.chars().any(|c| c.is_control()) {
        return invalid("Display name contains invalid characters.");
    }
    Ok(n.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn username_rules() {
        assert_eq!(validate_username("  teacher.1 ").unwrap(), "teacher.1");
        assert!(validate_username("ab").is_err());
        assert!(validate_username(&"a".repeat(33)).is_err());
        assert!(validate_username("bad name").is_err());
        assert!(validate_username("x'; DROP--").is_err());
        assert!(validate_username("añb").is_err());
    }

    #[test]
    fn password_rules() {
        assert!(validate_password("12345678", "teacher").is_ok());
        assert!(validate_password("short", "teacher").is_err());
        assert!(validate_password(&"x".repeat(129), "teacher").is_err());
        assert!(validate_password("Teacher12", "teacher12").is_err());
        assert!(validate_password("pass word with spaces", "t").is_ok());
    }

    #[test]
    fn display_name_rules() {
        assert_eq!(validate_display_name("  Ms. Reyes ").unwrap(), "Ms. Reyes");
        assert!(validate_display_name("   ").is_err());
        assert!(validate_display_name("bad\nname").is_err());
    }
}
