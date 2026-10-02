//! Argon2id password hashing (RustCrypto `argon2`, default params: m=19 MiB, t=2, p=1, random salt).

use argon2::password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use argon2::Argon2;
use std::sync::OnceLock;

use crate::errors::{AppError, AppResult};

pub fn hash_password(password: &str) -> AppResult<String> {
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map(|h| h.to_string())
        .map_err(|e| AppError::Internal(format!("password hashing failed: {e}")))
}

/// Returns false for a wrong password *and* for a malformed hash (never panics).
pub fn verify_password(password: &str, hash: &str) -> bool {
    PasswordHash::new(hash)
        .map(|parsed| Argon2::default().verify_password(password.as_bytes(), &parsed).is_ok())
        .unwrap_or(false)
}

/// A valid hash of a throwaway secret. Verifying against it when a username does not exist
/// keeps response time similar to a real failed login (no user-enumeration timing leak).
pub fn dummy_hash() -> &'static str {
    static HASH: OnceLock<String> = OnceLock::new();
    HASH.get_or_init(|| hash_password("not-a-real-password-\u{1f512}").expect("argon2 must work"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_is_argon2id_and_never_contains_the_password() {
        let h = hash_password("correct horse battery").unwrap();
        assert!(h.starts_with("$argon2id$"));
        assert!(!h.contains("correct horse"));
    }

    #[test]
    fn same_password_gets_different_salts() {
        assert_ne!(hash_password("same-password").unwrap(), hash_password("same-password").unwrap());
    }

    #[test]
    fn verify_accepts_right_and_rejects_wrong() {
        let h = hash_password("s3cret-pass").unwrap();
        assert!(verify_password("s3cret-pass", &h));
        assert!(!verify_password("s3cret-pasS", &h));
        assert!(!verify_password("", &h));
    }

    #[test]
    fn malformed_hash_is_a_failed_login_not_a_panic() {
        assert!(!verify_password("x", "not a hash"));
        assert!(!verify_password("x", ""));
    }

    #[test]
    fn dummy_hash_is_valid_but_matches_nothing_realistic() {
        assert!(PasswordHash::new(dummy_hash()).is_ok());
        assert!(!verify_password("password123", dummy_hash()));
    }
}
