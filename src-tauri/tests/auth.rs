//! Teacher authentication workflows against a real in-memory database.

use std::time::Duration;

use proctorlan_lib::auth::session::Limits;
use proctorlan_lib::database::Database;
use proctorlan_lib::errors::AppError;
use proctorlan_lib::services::auth_service::AuthService;

const PW: &str = "Str0ng-Passphrase";

async fn svc_with(limits: Limits) -> (AuthService, Database) {
    let db = Database::open_in_memory().await.unwrap();
    (AuthService::with_limits(db.clone(), limits), db)
}

async fn svc() -> (AuthService, Database) {
    svc_with(Limits { max_failures: 3, lockout: Duration::from_millis(250), idle_timeout: Duration::from_secs(60) }).await
}

#[tokio::test]
async fn first_launch_requires_setup_then_signs_in() {
    let (a, _) = svc().await;
    let s = a.status().await.unwrap();
    assert!(s.setup_required && s.user.is_none());
    a.setup_admin("teacher", PW, "Ms. Reyes").await.unwrap();
    let s = a.status().await.unwrap();
    assert!(!s.setup_required);
    assert_eq!(s.user.unwrap().display_name, "Ms. Reyes");
}

#[tokio::test]
async fn setup_can_only_happen_once() {
    let (a, db) = svc().await;
    a.setup_admin("teacher", PW, "One").await.unwrap();
    let second = a.setup_admin("intruder", PW, "Two").await;
    assert!(matches!(second, Err(AppError::Conflict(_))));
    assert_eq!(db.count_users().await.unwrap(), 1);
}

#[tokio::test]
async fn racing_setups_create_exactly_one_admin() {
    let (a, db) = svc().await;
    let a = std::sync::Arc::new(a);
    let tasks: Vec<_> = (0..4).map(|i| {
        let a = a.clone();
        tokio::spawn(async move { a.setup_admin(&format!("user{i}"), PW, "X").await.is_ok() })
    }).collect();
    let mut wins = 0;
    for t in tasks { if t.await.unwrap() { wins += 1; } }
    assert_eq!(wins, 1);
    assert_eq!(db.count_users().await.unwrap(), 1);
}

#[tokio::test]
async fn password_is_stored_as_argon2id_never_plaintext() {
    let (a, db) = svc().await;
    a.setup_admin("teacher", PW, "T").await.unwrap();
    let stored: String = sqlx::query_scalar("SELECT password_hash FROM users").fetch_one(db.pool()).await.unwrap();
    assert!(stored.starts_with("$argon2id$") && !stored.contains(PW));
}

#[tokio::test]
async fn setup_rejects_weak_or_malformed_input() {
    let (a, db) = svc().await;
    assert!(matches!(a.setup_admin("teacher", "short", "T").await, Err(AppError::Validation(_))));
    assert!(matches!(a.setup_admin("bad name!", PW, "T").await, Err(AppError::Validation(_))));
    assert!(matches!(a.setup_admin("teacher", PW, "   ").await, Err(AppError::Validation(_))));
    assert!(matches!(a.setup_admin("teacher", "teacher", "T").await, Err(AppError::Validation(_))));
    assert_eq!(db.count_users().await.unwrap(), 0, "rejected setup must not create a user");
}

#[tokio::test]
async fn login_succeeds_case_insensitively_and_fails_with_identical_message() {
    let (a, _) = svc().await;
    a.setup_admin("Teacher", PW, "T").await.unwrap();
    a.logout().await.unwrap();
    assert!(a.login("TEACHER", PW).await.is_ok());
    a.logout().await.unwrap();

    let wrong_pw = a.login("teacher", "wrong-password").await.unwrap_err().to_string();
    let no_user = a.login("nobody", "wrong-password").await.unwrap_err().to_string();
    assert_eq!(wrong_pw, no_user, "must not reveal whether the username exists");
    assert!(matches!(a.require_user().await, Err(AppError::Unauthorized(_))));
}

#[tokio::test]
async fn lockout_blocks_even_the_correct_password_then_recovers() {
    let (a, _) = svc().await;
    a.setup_admin("teacher", PW, "T").await.unwrap();
    a.logout().await.unwrap();
    for _ in 0..3 { let _ = a.login("teacher", "nope-nope").await; }
    let locked = a.login("teacher", PW).await;
    assert!(matches!(&locked, Err(AppError::Unauthorized(m)) if m.contains("Too many")));
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert!(a.login("teacher", PW).await.is_ok());
}

#[tokio::test]
async fn protected_calls_need_a_session() {
    let (a, _) = svc().await;
    a.setup_admin("teacher", PW, "T").await.unwrap();
    assert!(a.require_user().await.is_ok());
    a.logout().await.unwrap();
    assert!(matches!(a.require_user().await, Err(AppError::Unauthorized(_))));
    assert!(matches!(a.change_password(PW, "another-pass-1").await, Err(AppError::Unauthorized(_))));
    assert!(matches!(a.update_display_name("Hacker").await, Err(AppError::Unauthorized(_))));
}

#[tokio::test]
async fn idle_sessions_expire() {
    let (a, _) = svc_with(Limits { max_failures: 5, lockout: Duration::from_secs(1), idle_timeout: Duration::from_millis(80) }).await;
    a.setup_admin("teacher", PW, "T").await.unwrap();
    tokio::time::sleep(Duration::from_millis(120)).await;
    assert!(matches!(a.require_user().await, Err(AppError::Unauthorized(_))));
}

#[tokio::test]
async fn change_password_flow() {
    let (a, _) = svc().await;
    a.setup_admin("teacher", PW, "T").await.unwrap();
    assert!(matches!(a.change_password("wrong-current", "brand-new-pass").await, Err(AppError::Validation(_))));
    assert!(matches!(a.change_password(PW, "short").await, Err(AppError::Validation(_))));
    a.change_password(PW, "brand-new-pass").await.unwrap();
    a.logout().await.unwrap();
    assert!(a.login("teacher", PW).await.is_err(), "old password must stop working");
    assert!(a.login("teacher", "brand-new-pass").await.is_ok());
}

#[tokio::test]
async fn display_name_can_be_updated() {
    let (a, _) = svc().await;
    a.setup_admin("teacher", PW, "Old").await.unwrap();
    assert_eq!(a.update_display_name("  New Name ").await.unwrap().display_name, "New Name");
    assert!(a.update_display_name("").await.is_err());
}

#[tokio::test]
async fn audit_trail_records_events_without_secrets() {
    let (a, db) = svc().await;
    a.setup_admin("teacher", PW, "T").await.unwrap();
    let _ = a.login("teacher", "my-secret-typo").await;
    a.logout().await.unwrap();
    a.login("teacher", PW).await.unwrap();
    let rows: Vec<(String, Option<String>)> = sqlx::query_as("SELECT action, metadata FROM audit_logs ORDER BY created_at").fetch_all(db.pool()).await.unwrap();
    let actions: Vec<_> = rows.iter().map(|r| r.0.as_str()).collect();
    for expected in ["auth.setup_admin", "auth.login_failed", "auth.logout", "auth.login"] {
        assert!(actions.contains(&expected), "missing audit action {expected}");
    }
    let dump = format!("{rows:?}");
    assert!(!dump.contains(PW) && !dump.contains("my-secret-typo"));
}

#[tokio::test]
async fn serialised_user_never_contains_the_hash() {
    let (a, _) = svc().await;
    let u = a.setup_admin("teacher", PW, "T").await.unwrap();
    let json = serde_json::to_string(&u).unwrap();
    assert!(!json.contains("argon2") && !json.to_lowercase().contains("password"));
}
