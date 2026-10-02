//! Exam service workflows against a real in-memory database.

use proctorlan_lib::database::Database;
use proctorlan_lib::errors::AppError;
use proctorlan_lib::models::*;
use proctorlan_lib::services::exam_service::ExamService;

fn ch(t: &str, ok: bool) -> NewChoice { NewChoice { choice_text: t.into(), is_correct: ok } }

fn good_exam(title: &str) -> NewExam {
    let q = |ty, text: &str, choices| NewQuestion { question_text: text.into(), question_type: ty, points: 2.0, required: true, explanation: Some("because".into()), choices };
    NewExam {
        title: title.into(), description: "d".into(), instructions: "i".into(), duration_minutes: 45, passing_score: 70.0,
        randomize_questions: true, randomize_choices: true, allow_review: true, auto_submit: true, show_results: false,
        questions: vec![
            q(QuestionType::MultipleChoice, "What does HTML stand for?", vec![ch("Hyper Text Markup Language", true), ch("High Text Machine Language", false), ch("Home Tool Markup Language", false)]),
            q(QuestionType::TrueFalse, "CSS styles pages.", vec![ch("True", true), ch("False", false)]),
            q(QuestionType::MultipleSelect, "Pick the languages", vec![ch("Rust", true), ch("Python", true), ch("HTML", false)]),
            q(QuestionType::Identification, "Capital of France?", vec![ch("Paris", true), ch("paris", true)]),
        ],
    }
}

async fn setup() -> (ExamService, Database, User) {
    let db = Database::open_in_memory().await.unwrap();
    let user = db.create_user("teacher", "hash", "Teacher", UserRole::Admin).await.unwrap();
    (ExamService::new(db.clone()), db, user)
}

#[tokio::test]
async fn create_persists_all_four_question_types_with_audit_entry() {
    let (svc, db, user) = setup().await;
    let full = svc.create(&user, good_exam("Web Basics")).await.unwrap();
    assert_eq!(full.exam.status, ExamStatus::Inactive);
    assert_eq!(full.exam.created_by.as_deref(), Some(user.id.as_str()));
    let types: Vec<_> = full.questions.iter().map(|q| q.question.question_type).collect();
    assert_eq!(types, [QuestionType::MultipleChoice, QuestionType::TrueFalse, QuestionType::MultipleSelect, QuestionType::Identification]);
    assert_eq!(full.questions[3].choices.len(), 2);
    assert_eq!(full.questions[0].question.explanation.as_deref(), Some("because"));
    let audits: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM audit_logs WHERE action='exam.create'").fetch_one(db.pool()).await.unwrap();
    assert_eq!(audits, 1);
}

#[tokio::test]
async fn incomplete_drafts_can_be_saved_but_not_activated() {
    let (svc, _, user) = setup().await;
    let mut draft = good_exam("Draft");
    draft.questions[0].choices.iter_mut().for_each(|c| c.is_correct = false); // no correct answer yet
    let saved = svc.create(&user, draft).await.unwrap();
    let err = svc.set_active(&user, &saved.exam.id, true).await.unwrap_err();
    assert!(matches!(&err, AppError::Validation(m) if m.contains("not ready") && m.contains("Question 1")), "{err}");
    assert_eq!(svc.get(&saved.exam.id).await.unwrap().exam.status, ExamStatus::Inactive);
}

#[tokio::test]
async fn hard_rules_block_even_drafts() {
    let (svc, db, user) = setup().await;
    let mut e = good_exam("   ");
    assert!(matches!(svc.create(&user, e.clone()).await, Err(AppError::Validation(_))));
    e.title = "ok".into();
    e.duration_minutes = 0;
    assert!(matches!(svc.create(&user, e.clone()).await, Err(AppError::Validation(_))));
    e.duration_minutes = 10;
    e.questions[1].points = -5.0;
    assert!(matches!(svc.create(&user, e).await, Err(AppError::Validation(_))));
    assert!(db.list_exams().await.unwrap().is_empty());
}

#[tokio::test]
async fn ready_exam_can_be_activated_and_deactivated() {
    let (svc, _, user) = setup().await;
    let id = svc.create(&user, good_exam("Ready")).await.unwrap().exam.id;
    assert_eq!(svc.set_active(&user, &id, true).await.unwrap().exam.status, ExamStatus::Active);
    assert_eq!(svc.set_active(&user, &id, false).await.unwrap().exam.status, ExamStatus::Inactive);
}

#[tokio::test]
async fn editing_an_active_exam_must_keep_it_complete() {
    let (svc, _, user) = setup().await;
    let id = svc.create(&user, good_exam("Live")).await.unwrap().exam.id;
    svc.set_active(&user, &id, true).await.unwrap();
    let mut broken = good_exam("Live");
    broken.questions.clear();
    assert!(matches!(svc.update(&user, &id, broken).await, Err(AppError::Validation(_))));
    assert_eq!(svc.get(&id).await.unwrap().questions.len(), 4, "failed update must change nothing");
    let mut fine = good_exam("Live v2");
    fine.questions.truncate(2);
    let updated = svc.update(&user, &id, fine).await.unwrap();
    assert_eq!((updated.exam.title.as_str(), updated.questions.len()), ("Live v2", 2));
    assert_eq!(updated.exam.status, ExamStatus::Active);
}

#[tokio::test]
async fn update_reorders_questions() {
    let (svc, _, user) = setup().await;
    let id = svc.create(&user, good_exam("Order")).await.unwrap().exam.id;
    let mut e = good_exam("Order");
    e.questions.reverse();
    let after = svc.update(&user, &id, e).await.unwrap();
    assert_eq!(after.questions[0].question.question_text, "Capital of France?");
    assert_eq!(after.questions[0].question.sort_order, 0);
}

#[tokio::test]
async fn duplicate_creates_independent_inactive_copy() {
    let (svc, _, user) = setup().await;
    let orig = svc.create(&user, good_exam("Original")).await.unwrap().exam.id;
    svc.set_active(&user, &orig, true).await.unwrap();
    let copy = svc.duplicate(&user, &orig).await.unwrap();
    assert_eq!(copy.exam.title, "Original (copy)");
    assert_eq!(copy.exam.status, ExamStatus::Inactive);
    assert_eq!(copy.questions.len(), 4);
    svc.delete(&user, &copy.exam.id).await.unwrap();
    assert!(svc.get(&orig).await.is_ok(), "deleting the copy must not touch the original");
}

#[tokio::test]
async fn exams_with_sessions_are_locked() {
    let (svc, db, user) = setup().await;
    let id = svc.create(&user, good_exam("Used")).await.unwrap().exam.id;
    db.create_session(&id, "192.168.1.5", 38123).await.unwrap();
    assert_eq!(svc.get(&id).await.unwrap().session_count, 1);
    assert_eq!(svc.list().await.unwrap()[0].session_count, 1);
    assert!(matches!(svc.update(&user, &id, good_exam("Edited")).await, Err(AppError::Conflict(_))));
    assert!(matches!(svc.delete(&user, &id).await, Err(AppError::Conflict(_))));
    assert!(svc.duplicate(&user, &id).await.is_ok(), "duplicating is the supported way to change a used exam");
}

#[tokio::test]
async fn check_reports_issues_with_paths_for_the_builder() {
    let (svc, _, _) = setup().await;
    let mut e = good_exam("T");
    e.questions[0].choices.iter_mut().for_each(|c| c.is_correct = false);
    let issues = svc.check(&e);
    assert!(issues.iter().any(|i| i.path == "questions.0.correct"));
    assert!(svc.check(&good_exam("T")).is_empty());
}

#[tokio::test]
async fn delete_and_missing_ids() {
    let (svc, _, user) = setup().await;
    let id = svc.create(&user, good_exam("Gone")).await.unwrap().exam.id;
    svc.delete(&user, &id).await.unwrap();
    assert!(matches!(svc.get(&id).await, Err(AppError::NotFound(_))));
    assert!(matches!(svc.delete(&user, &id).await, Err(AppError::NotFound(_))));
    assert!(matches!(svc.set_active(&user, "nope", false).await, Err(AppError::NotFound(_))));
}
