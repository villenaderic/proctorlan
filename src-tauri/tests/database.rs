//! Integration tests against a real (in-memory) SQLite database with the production migrations.

use proctorlan_lib::database::Database;
use proctorlan_lib::errors::AppError;
use proctorlan_lib::models::*;

async fn db() -> Database {
    Database::open_in_memory().await.expect("open db")
}

fn choice(t: &str, ok: bool) -> NewChoice {
    NewChoice { choice_text: t.into(), is_correct: ok }
}

fn mc(text: &str, points: f64) -> NewQuestion {
    NewQuestion {
        question_text: text.into(), question_type: QuestionType::MultipleChoice, points, required: true, explanation: None,
        choices: vec![choice("A", true), choice("B", false), choice("C", false), choice("D", false)],
    }
}

fn exam(title: &str, questions: Vec<NewQuestion>) -> NewExam {
    NewExam {
        title: title.into(), description: "d".into(), instructions: "i".into(), duration_minutes: 60, passing_score: 75.0,
        randomize_questions: false, randomize_choices: true, allow_review: true, auto_submit: true, show_results: false, questions,
    }
}

/// exam + session + student + attempt, returning ids.
async fn setup_attempt(db: &Database) -> (String, ExamSession, Attempt) {
    let exam_id = db.create_exam(None, &exam("Intro", vec![mc("Q1", 2.0), mc("Q2", 3.0)])).await.unwrap();
    let session = db.create_session(&exam_id, "192.168.1.100", 38123).await.unwrap();
    let student = db.upsert_student("2026-001", "Juan Dela Cruz").await.unwrap();
    let attempt = db.create_attempt(&session.id, &student.id, "hash-1").await.unwrap();
    (exam_id, session, attempt)
}

#[tokio::test]
async fn migrations_create_every_required_table() {
    let db = db().await;
    let names: Vec<String> = sqlx::query_scalar("SELECT name FROM sqlite_master WHERE type='table'").fetch_all(db.pool()).await.unwrap();
    for t in ["users","exams","questions","choices","exam_sessions","students","attempts","answers","proctor_events","application_settings","audit_logs"] {
        assert!(names.iter().any(|n| n == t), "missing table {t}");
    }
    assert_eq!(db.migrations_applied().await.unwrap(), 1);
}

#[tokio::test]
async fn foreign_keys_are_enforced() {
    let db = db().await;
    let r = sqlx::query("INSERT INTO questions (id, exam_id, question_text, question_type, points, sort_order, created_at, updated_at) VALUES ('q','nope','t','multiple_choice',1,0,'x','x')")
        .execute(db.pool()).await;
    assert!(r.is_err(), "orphan question must be rejected");
}

#[tokio::test]
async fn exam_round_trip_preserves_order_and_fields() {
    let db = db().await;
    let id = db.create_exam(None, &exam("Math", vec![mc("first", 1.0), mc("second", 2.5)])).await.unwrap();
    let full = db.get_exam_full(&id).await.unwrap();
    assert_eq!(full.exam.title, "Math");
    assert_eq!(full.exam.status, ExamStatus::Inactive);
    assert!(full.exam.randomize_choices && !full.exam.randomize_questions);
    assert_eq!(full.questions.len(), 2);
    assert_eq!(full.questions[0].question.question_text, "first");
    assert_eq!(full.questions[1].question.sort_order, 1);
    assert_eq!(full.questions[0].choices.len(), 4);
    assert_eq!(full.questions[0].choices.iter().filter(|c| c.is_correct).count(), 1);
    let list = db.list_exams().await.unwrap();
    assert_eq!((list[0].question_count, list[0].total_points), (2, 3.5));
}

#[tokio::test]
async fn failed_exam_creation_leaves_nothing_behind() {
    let db = db().await;
    // second question has 0 points -> CHECK violation mid-transaction
    let bad = exam("Broken", vec![mc("ok", 1.0), mc("bad", 0.0)]);
    assert!(matches!(db.create_exam(None, &bad).await, Err(AppError::Validation(_))));
    assert!(db.list_exams().await.unwrap().is_empty(), "exam row must roll back");
    let n: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM questions").fetch_one(db.pool()).await.unwrap();
    assert_eq!(n, 0);
}

#[tokio::test]
async fn check_constraints_reject_invalid_exam_settings() {
    let db = db().await;
    let mut e = exam("X", vec![]);
    e.duration_minutes = 0;
    assert!(db.create_exam(None, &e).await.is_err());
    let mut e = exam("X", vec![]);
    e.passing_score = 101.0;
    assert!(db.create_exam(None, &e).await.is_err());
    assert!(db.create_exam(None, &exam("   ", vec![])).await.is_err(), "blank title");
}

#[tokio::test]
async fn replace_exam_swaps_questions_and_cascades_choices() {
    let db = db().await;
    let id = db.create_exam(None, &exam("V1", vec![mc("old", 1.0)])).await.unwrap();
    db.replace_exam(&id, &exam("V2", vec![mc("new1", 1.0), mc("new2", 1.0), mc("new3", 1.0)])).await.unwrap();
    let full = db.get_exam_full(&id).await.unwrap();
    assert_eq!(full.exam.title, "V2");
    assert_eq!(full.questions.len(), 3);
    let choices: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM choices").fetch_one(db.pool()).await.unwrap();
    assert_eq!(choices, 12, "old choices must be gone");
    assert!(matches!(db.replace_exam("missing", &exam("x", vec![])).await, Err(AppError::NotFound(_))));
}

#[tokio::test]
async fn duplicate_exam_copies_everything() {
    let db = db().await;
    let id = db.create_exam(None, &exam("Orig", vec![mc("q", 4.0)])).await.unwrap();
    let copy = db.duplicate_exam(&id, None).await.unwrap();
    assert_ne!(copy, id);
    let c = db.get_exam_full(&copy).await.unwrap();
    assert_eq!(c.exam.title, "Orig (copy)");
    assert_eq!(c.questions[0].question.points, 4.0);
    assert_eq!(c.questions[0].choices.len(), 4);
}

#[tokio::test]
async fn delete_exam_cascades_but_is_blocked_by_sessions() {
    let db = db().await;
    let a = db.create_exam(None, &exam("Deletable", vec![mc("q", 1.0)])).await.unwrap();
    db.delete_exam(&a).await.unwrap();
    let q: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM questions").fetch_one(db.pool()).await.unwrap();
    assert_eq!(q, 0);
    assert!(matches!(db.delete_exam(&a).await, Err(AppError::NotFound(_))));

    let (exam_id, _, _) = setup_attempt(&db).await;
    assert!(matches!(db.delete_exam(&exam_id).await, Err(AppError::Conflict(_))));
    assert!(matches!(db.replace_exam(&exam_id, &exam("edit", vec![])).await, Err(AppError::Conflict(_))));
    assert!(db.get_exam_full(&exam_id).await.is_ok(), "exam must survive");
}

#[tokio::test]
async fn session_code_unique_among_open_sessions_but_reusable_after_end() {
    let db = db().await;
    let exam_id = db.create_exam(None, &exam("E", vec![mc("q", 1.0)])).await.unwrap();
    let s1 = db.create_session(&exam_id, "10.0.0.1", 38123).await.unwrap();
    let dup = |code: String| {
        let db = db.clone();
        let exam_id = exam_id.clone();
        async move {
            sqlx::query("INSERT INTO exam_sessions (id, exam_id, session_code, status, host_ip, host_port, created_at) VALUES (?,?,?,'CREATED','x',1,'t')")
                .bind(uuid_like()).bind(exam_id).bind(code).execute(db.pool()).await
        }
    };
    assert!(dup(s1.session_code.clone()).await.is_err(), "duplicate open code must fail");
    db.transition_session(&s1.id, SessionStatus::Ended).await.unwrap();
    assert!(dup(s1.session_code.clone()).await.is_ok(), "ended code can be reused");
}

fn uuid_like() -> String {
    format!("id-{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos())
}

#[tokio::test]
async fn find_session_by_code_normalises_input_and_hides_ended() {
    let db = db().await;
    let exam_id = db.create_exam(None, &exam("E", vec![mc("q", 1.0)])).await.unwrap();
    let s = db.create_session(&exam_id, "10.0.0.1", 38123).await.unwrap();
    let lower = format!("  {} ", s.session_code.to_lowercase());
    assert_eq!(db.find_open_session_by_code(&lower).await.unwrap().unwrap().id, s.id);
    assert!(db.find_open_session_by_code("nonsense'; --").await.unwrap().is_none());
    db.transition_session(&s.id, SessionStatus::Ended).await.unwrap();
    assert!(db.find_open_session_by_code(&s.session_code).await.unwrap().is_none());
}

#[tokio::test]
async fn create_session_for_missing_exam_is_not_found() {
    let db = db().await;
    assert!(matches!(db.create_session("ghost", "1.1.1.1", 1000).await, Err(AppError::NotFound(_))));
}

#[tokio::test]
async fn session_lifecycle_rules_are_enforced() {
    let db = db().await;
    let exam_id = db.create_exam(None, &exam("E", vec![mc("q", 1.0)])).await.unwrap();
    let s = db.create_session(&exam_id, "10.0.0.1", 38123).await.unwrap();
    assert_eq!(s.status, SessionStatus::Created);
    assert!(matches!(db.transition_session(&s.id, SessionStatus::Running).await, Err(AppError::Conflict(_))), "must pass WAITING first");
    db.transition_session(&s.id, SessionStatus::Waiting).await.unwrap();
    let r = db.transition_session(&s.id, SessionStatus::Running).await.unwrap();
    assert!(r.started_at.is_some());
    db.transition_session(&s.id, SessionStatus::Paused).await.unwrap();
    let r = db.transition_session(&s.id, SessionStatus::Running).await.unwrap();
    assert!(r.started_at.is_some());
    let ended = db.transition_session(&s.id, SessionStatus::Ended).await.unwrap();
    assert!(ended.ended_at.is_some());
    assert!(matches!(db.transition_session(&s.id, SessionStatus::Running).await, Err(AppError::Conflict(_))), "ENDED is terminal");
}

#[tokio::test]
async fn one_attempt_per_student_per_session() {
    let db = db().await;
    let (_, session, attempt) = setup_attempt(&db).await;
    let again = db.create_attempt(&session.id, &attempt.student_id, "hash-2").await;
    assert!(matches!(again, Err(AppError::Conflict(_))));
    let found = db.find_attempt(&session.id, &attempt.student_id).await.unwrap().unwrap();
    assert_eq!(found.id, attempt.id);
    assert_eq!(db.find_attempt_by_token_hash("hash-1").await.unwrap().unwrap().id, attempt.id);
    assert!(db.find_attempt_by_token_hash("wrong").await.unwrap().is_none());
}

#[tokio::test]
async fn student_upsert_updates_name_without_duplicating() {
    let db = db().await;
    let a = db.upsert_student("2026-001", "Juan").await.unwrap();
    let b = db.upsert_student("2026-001", "Juan D. Cruz").await.unwrap();
    assert_eq!(a.id, b.id);
    assert_eq!(b.name, "Juan D. Cruz");
}

#[tokio::test]
async fn answer_sync_is_idempotent_and_ignores_stale_updates() {
    let db = db().await;
    let (exam_id, _, attempt) = setup_attempt(&db).await;
    let q = db.get_exam_full(&exam_id).await.unwrap().questions[0].question.id.clone();

    assert!(db.upsert_answer(&attempt.id, &q, r#"["a"]"#, 1).await.unwrap(), "first write stored");
    assert!(!db.upsert_answer(&attempt.id, &q, r#"["a"]"#, 1).await.unwrap(), "exact duplicate ignored");
    assert!(db.upsert_answer(&attempt.id, &q, r#"["b"]"#, 3).await.unwrap(), "newer replaces");
    assert!(!db.upsert_answer(&attempt.id, &q, r#"["stale"]"#, 2).await.unwrap(), "late older message ignored");

    let answers = db.list_answers(&attempt.id).await.unwrap();
    assert_eq!(answers.len(), 1, "never duplicate rows");
    assert_eq!(answers[0].answer_data, r#"["b"]"#);
    assert_eq!(answers[0].client_seq, 3);
}

#[tokio::test]
async fn answer_validation_rejects_bad_input() {
    let db = db().await;
    let (exam_id, _, attempt) = setup_attempt(&db).await;
    let q = db.get_exam_full(&exam_id).await.unwrap().questions[0].question.id.clone();
    assert!(matches!(db.upsert_answer(&attempt.id, &q, "not json {", 1).await, Err(AppError::Validation(_))));
    assert!(matches!(db.upsert_answer(&attempt.id, "invented-question", r#""x""#, 1).await, Err(AppError::Validation(_))));

    // a question from a different exam must be rejected too
    let other = db.create_exam(None, &exam("Other", vec![mc("foreign", 1.0)])).await.unwrap();
    let foreign = db.get_exam_full(&other).await.unwrap().questions[0].question.id.clone();
    assert!(matches!(db.upsert_answer(&attempt.id, &foreign, r#""x""#, 1).await, Err(AppError::Validation(_))));
    assert_eq!(db.count_answers(&attempt.id).await.unwrap(), 0);
}

#[tokio::test]
async fn submitted_attempts_reject_further_answers() {
    let db = db().await;
    let (exam_id, _, attempt) = setup_attempt(&db).await;
    let q = db.get_exam_full(&exam_id).await.unwrap().questions[0].question.id.clone();
    db.set_attempt_status(&attempt.id, AttemptStatus::InProgress).await.unwrap();
    assert!(db.get_attempt(&attempt.id).await.unwrap().started_at.is_some());
    db.set_attempt_status(&attempt.id, AttemptStatus::Submitted).await.unwrap();
    assert!(matches!(db.upsert_answer(&attempt.id, &q, r#""late""#, 9).await, Err(AppError::Conflict(_))));
}

#[tokio::test]
async fn proctor_events_and_audit_log_are_stored() {
    let db = db().await;
    let (_, _, attempt) = setup_attempt(&db).await;
    db.record_proctor_event(&attempt.id, ProctorEventType::FocusLost, "window lost focus", None).await.unwrap();
    db.record_proctor_event(&attempt.id, ProctorEventType::FocusRestored, "", Some(r#"{"ms":1200}"#)).await.unwrap();
    let ev = db.list_proctor_events(&attempt.id).await.unwrap();
    assert_eq!(ev.len(), 2);
    assert_eq!(ev[0].event_type, ProctorEventType::FocusLost);
    db.audit(None, "exam.create", "exam", Some("x"), None).await.unwrap();
    let n: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM audit_logs").fetch_one(db.pool()).await.unwrap();
    assert_eq!(n, 1);
}

#[tokio::test]
async fn users_are_unique_case_insensitively_and_hash_is_not_serialised() {
    let db = db().await;
    let u = db.create_user("Teacher1", "$argon2id$fake", "Ms. Cruz", UserRole::Admin).await.unwrap();
    assert!(matches!(db.create_user("teacher1", "h", "Dup", UserRole::Teacher).await, Err(AppError::Conflict(_))));
    assert_eq!(db.find_user_by_username("TEACHER1").await.unwrap().unwrap().id, u.id);
    assert_eq!(db.count_users().await.unwrap(), 1);
    let json = serde_json::to_string(&u).unwrap();
    assert!(!json.contains("argon2") && !json.contains("passwordHash"), "hash must never reach the UI");
}

#[tokio::test]
async fn settings_upsert_and_stats_reflect_real_data() {
    let db = db().await;
    assert_eq!(db.get_setting("port").await.unwrap(), None);
    db.set_setting("port", "38123").await.unwrap();
    db.set_setting("port", "40000").await.unwrap();
    assert_eq!(db.get_setting("port").await.unwrap().as_deref(), Some("40000"));

    let (_, session, attempt) = setup_attempt(&db).await;
    let s = db.stats().await.unwrap();
    assert_eq!((s.exams, s.students, s.active_sessions, s.completed_attempts), (1, 1, 0, 0));
    db.transition_session(&session.id, SessionStatus::Waiting).await.unwrap();
    db.set_attempt_status(&attempt.id, AttemptStatus::Submitted).await.unwrap();
    let s = db.stats().await.unwrap();
    assert_eq!((s.active_sessions, s.completed_attempts), (1, 1));
}

#[tokio::test]
async fn hostile_strings_are_stored_literally_not_executed() {
    let db = db().await;
    let evil = "Robert'); DROP TABLE exams;--";
    let id = db.create_exam(None, &exam(evil, vec![mc(evil, 1.0)])).await.unwrap();
    assert_eq!(db.get_exam_full(&id).await.unwrap().exam.title, evil);
    db.upsert_student(evil, evil).await.unwrap();
    assert_eq!(db.list_exams().await.unwrap().len(), 1, "exams table must still exist");
}

#[tokio::test]
async fn file_database_persists_across_reopen() {
    let dir = std::env::temp_dir().join(format!("proctorlan-test-{}", uuid_like()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("t.db");
    {
        let db = Database::open(&path).await.unwrap();
        db.create_exam(None, &exam("Persisted", vec![mc("q", 1.0)])).await.unwrap();
        db.close().await;
    }
    let db = Database::open(&path).await.unwrap();
    assert_eq!(db.list_exams().await.unwrap()[0].exam.title, "Persisted");
    assert_eq!(db.migrations_applied().await.unwrap(), 1, "migrations must not re-run");
    db.close().await;
    let _ = std::fs::remove_dir_all(&dir);
}
