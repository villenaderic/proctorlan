//! Exam engine over real sockets: delivery, answers, grading, submit, expiry, auto-submit.

mod common;

use std::time::Duration;

use common::*;
use proctorlan_lib::models::*;
use proctorlan_lib::services::exam_engine;
use proctorlan_lib::services::session_service::SessionAction;
use serde_json::{json, Value};

/// Opens a four-type session, starts it, joins one student who opens the exam.
async fn running(h: &Harness, exam: NewExam) -> (String, String, Ws, Value, Value) {
    let s = h.open_session_for(exam).await;
    let (sid, code) = (s.row.session.id.clone(), s.row.session.session_code.clone());
    let (mut ws, joined) = h.student(&code, "Ana Reyes", "A1").await;
    h.sessions.act(&h.user, &sid, SessionAction::Start).await.unwrap();
    ws.recv_kind("session_started").await;
    let paper = ws.start_exam().await;
    assert_eq!(paper["type"], "exam_paper", "{paper}");
    (sid, code, ws, joined, paper["payload"]["paper"].clone())
}

async fn answer_all_correctly(ws: &mut Ws, paper: &Value) {
    let a = |id: String, v: Value| (id, v);
    let answers = [
        a(qid(paper, "Capital"), json!(cid(paper, "Capital", "Paris"))),
        a(qid(paper, "CSS"), json!(cid(paper, "CSS", "True"))),
        a(qid(paper, "languages"), json!([cid(paper, "languages", "Rust"), cid(paper, "languages", "Python")])),
        a(qid(paper, "Hamlet"), json!("  shakespeare ")),
    ];
    for (i, (q, v)) in answers.into_iter().enumerate() {
        let r = ws.answer(&q, v, i as i64 + 1).await;
        assert_eq!(r["type"], "answer_ack", "{r}");
        assert_eq!(r["payload"]["stored"], true);
    }
}

#[tokio::test]
async fn the_paper_never_contains_answer_keys_explanations_or_identification_answers() {
    let h = harness().await;
    let (_, _, _, _, paper) = running(&h, four_type_exam()).await;
    let dump = paper.to_string();
    for forbidden in ["isCorrect", "is_correct", "SECRET EXPLANATION", "explanation", "Shakespeare", "William"] {
        assert!(!dump.contains(forbidden), "paper leaked {forbidden}: {dump}");
    }
    let qs = paper["questions"].as_array().unwrap();
    assert_eq!(qs.len(), 4);
    let ident = qs.iter().find(|q| q["type"] == "identification").unwrap();
    assert_eq!(ident["choices"].as_array().unwrap().len(), 0);
    assert_eq!(paper["totalPoints"], 5.0);
    assert!(qs.iter().find(|q| q["type"] == "multiple_choice").unwrap()["choices"].as_array().unwrap().len() == 3);
}

#[tokio::test]
async fn the_exam_cannot_be_opened_or_answered_unless_running() {
    let h = harness().await;
    let s = h.open_session_for(four_type_exam()).await;
    let (sid, code) = (s.row.session.id.clone(), s.row.session.session_code.clone());
    let (mut ws, _) = h.student(&code, "Ana", "A1").await;
    assert_eq!(ws.start_exam().await["payload"]["code"], "exam_not_running", "waiting");
    h.sessions.act(&h.user, &sid, SessionAction::Start).await.unwrap();
    ws.recv_kind("session_started").await;

    // Answering before opening the exam is refused.
    let r = ws.answer("whatever", json!("x"), 1).await;
    assert_eq!(r["payload"]["code"], "exam_not_started");

    let paper = ws.start_exam().await["payload"]["paper"].clone();
    h.sessions.act(&h.user, &sid, SessionAction::Pause).await.unwrap();
    ws.recv_kind("session_paused").await;
    let q = qid(&paper, "CSS");
    let t = cid(&paper, "CSS", "True");
    assert_eq!(ws.answer(&q, json!(t), 1).await["payload"]["code"], "exam_not_running", "paused");
    assert_eq!(ws.start_exam().await["payload"]["code"], "exam_not_running", "no questions during a pause");
    h.sessions.act(&h.user, &sid, SessionAction::Resume).await.unwrap();
    ws.recv_kind("session_resumed").await;
    assert_eq!(ws.answer(&q, json!(t), 1).await["type"], "answer_ack");
}

#[tokio::test]
async fn answers_are_idempotent_and_the_latest_sequence_wins() {
    let h = harness().await;
    let (_, _, mut ws, _, paper) = running(&h, four_type_exam()).await;
    let q = qid(&paper, "Capital");
    let (paris, rome) = (cid(&paper, "Capital", "Paris"), cid(&paper, "Capital", "Rome"));
    assert_eq!(ws.answer(&q, json!(rome), 5).await["payload"]["stored"], true);
    assert_eq!(ws.answer(&q, json!(rome), 5).await["payload"]["stored"], false, "exact duplicate");
    assert_eq!(ws.answer(&q, json!(paris), 3).await["payload"]["stored"], false, "stale retry must not overwrite");
    assert_eq!(ws.answer(&q, json!(paris), 6).await["payload"]["stored"], true);
    let n: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM answers").fetch_one(h.db.pool()).await.unwrap();
    assert_eq!(n, 1);
    let stored: String = sqlx::query_scalar("SELECT answer_data FROM answers").fetch_one(h.db.pool()).await.unwrap();
    assert_eq!(stored, format!("\"{paris}\""));
}

#[tokio::test]
async fn bad_answers_get_clear_errors_and_store_nothing() {
    let h = harness().await;
    let (_, _, mut ws, _, paper) = running(&h, four_type_exam()).await;
    let (mc, ms, id) = (qid(&paper, "Capital"), qid(&paper, "languages"), qid(&paper, "Hamlet"));
    let cases = [
        (mc.clone(), json!("not-a-choice")),
        (mc.clone(), json!(["array-for-single"])),
        (ms.clone(), json!("string-for-multi")),
        (ms.clone(), json!(["nope"])),
        (id.clone(), json!(42)),
        (id.clone(), json!("a".repeat(501))),
        ("unknown-question".to_string(), json!("x")),
    ];
    for (q, v) in cases {
        let r = ws.answer(&q, v.clone(), 1).await;
        assert_eq!(r["payload"]["code"], "invalid_answer", "{v}");
    }
    assert_eq!(ws.request("answer", json!({ "questionId": mc, "answer": "x" })).await["payload"]["code"], "invalid_answer", "missing clientSeq");
    let n: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM answers").fetch_one(h.db.pool()).await.unwrap();
    assert_eq!(n, 0);
}

#[tokio::test]
async fn submit_grades_on_the_server_and_hides_the_score_unless_results_are_enabled() {
    let h = harness().await;
    let (sid, _, mut ws, joined, paper) = running(&h, four_type_exam()).await;
    answer_all_correctly(&mut ws, &paper).await;
    let r = ws.submit().await;
    assert_eq!(r["type"], "submitted");
    assert!(r["payload"]["result"].is_null(), "show_results is off, so no score for the student: {r}");

    let attempt = h.db.get_attempt(joined["attemptId"].as_str().unwrap()).await.unwrap();
    assert_eq!(attempt.status, AttemptStatus::Submitted);
    assert_eq!((attempt.score, attempt.total_points, attempt.percentage, attempt.passed), (Some(5.0), Some(5.0), Some(100.0), Some(true)));
    assert!(attempt.submitted_at.is_some());
    let graded = h.db.list_answers(&attempt.id).await.unwrap();
    assert!(graded.iter().all(|a| a.is_correct == Some(true) && a.points_awarded.is_some()));
    assert_eq!(h.sessions.snapshot(&sid).await.unwrap().submitted, 1);
}

#[tokio::test]
async fn results_are_returned_when_enabled_and_partial_credit_is_not_given() {
    let h = harness().await;
    let mut exam = four_type_exam();
    exam.show_results = true;
    let (_, _, mut ws, joined, paper) = running(&h, exam).await;
    // Right: MC, TF. Wrong: multi-select (only one of two), identification (misspelt). => 2 of 5 = 40%.
    ws.answer(&qid(&paper, "Capital"), json!(cid(&paper, "Capital", "Paris")), 1).await;
    ws.answer(&qid(&paper, "CSS"), json!(cid(&paper, "CSS", "True")), 2).await;
    ws.answer(&qid(&paper, "languages"), json!([cid(&paper, "languages", "Rust")]), 3).await;
    ws.answer(&qid(&paper, "Hamlet"), json!("Shakespear"), 4).await;
    let r = ws.submit().await;
    let res = &r["payload"]["result"];
    assert_eq!((res["score"].as_f64(), res["totalPoints"].as_f64(), res["percentage"].as_f64(), res["passed"].as_bool()), (Some(2.0), Some(5.0), Some(40.0), Some(false)), "{r}");
    let a = h.db.get_attempt(joined["attemptId"].as_str().unwrap()).await.unwrap();
    assert_eq!(a.passed, Some(false));
}

#[tokio::test]
async fn unanswered_questions_score_zero_and_double_submit_is_harmless() {
    let h = harness().await;
    let mut exam = four_type_exam();
    exam.show_results = true;
    let (_, _, mut ws, _, paper) = running(&h, exam).await;
    ws.answer(&qid(&paper, "CSS"), json!(cid(&paper, "CSS", "True")), 1).await;
    let first = ws.submit().await;
    assert_eq!(first["payload"]["result"]["percentage"], 20.0);
    let second = ws.submit().await;
    assert_eq!(second["payload"]["result"], first["payload"]["result"]);
    let r = ws.answer(&qid(&paper, "Capital"), json!(cid(&paper, "Capital", "Paris")), 2).await;
    assert_eq!(r["payload"]["code"], "already_submitted");
    assert_eq!(ws.start_exam().await["payload"]["code"], "already_submitted");
}

#[tokio::test]
async fn rejoining_restores_saved_answers_and_the_same_question_order() {
    let h = harness().await;
    let mut exam = four_type_exam();
    exam.randomize_questions = true;
    exam.randomize_choices = true;
    exam.questions.extend((0..8).map(|i| question(QuestionType::MultipleChoice, &format!("Filler {i}"), 1.0, vec![ch("a", true), ch("b", false), ch("c", false), ch("d", false)])));
    let s = h.open_session_for(exam).await;
    let (sid, code) = (s.row.session.id.clone(), s.row.session.session_code.clone());
    h.sessions.act(&h.user, &sid, SessionAction::Start).await.unwrap();

    let mut ws = h.ws().await;
    ws.hello(&code).await.unwrap();
    let joined = ws.join("Ana", "A1", None).await;
    let token = joined["payload"]["token"].as_str().unwrap().to_string();
    let paper = ws.start_exam().await["payload"]["paper"].clone();
    let order = |p: &Value| p["questions"].as_array().unwrap().iter().map(|q| q["id"].as_str().unwrap().to_string()).collect::<Vec<_>>();
    let choice_order = |p: &Value| p["questions"].as_array().unwrap().iter().map(|q| q["choices"].as_array().unwrap().iter().map(|c| c["id"].as_str().unwrap().to_string()).collect::<Vec<_>>()).collect::<Vec<_>>();
    let q = qid(&paper, "Capital");
    ws.answer(&q, json!(cid(&paper, "Capital", "Rome")), 1).await;
    drop(ws);

    let mut back = h.ws().await;
    back.hello(&code).await.unwrap();
    let again = back.join("Ana", "A1", Some(&token)).await;
    assert_eq!(again["payload"]["resumed"], true);
    let paper2 = back.start_exam().await["payload"]["paper"].clone();
    assert_eq!(order(&paper), order(&paper2), "same attempt, same question order");
    assert_eq!(choice_order(&paper), choice_order(&paper2), "same attempt, same choice order");
    assert_eq!(paper2["answers"][&q], json!(cid(&paper, "Capital", "Rome")), "saved answer restored");
    assert_ne!(order(&paper), (0..12).map(|i| paper["questions"][i]["id"].as_str().unwrap().to_string()).collect::<Vec<_>>().into_iter().rev().collect::<Vec<_>>());

    // A different student gets a different shuffle of the same exam.
    let (mut other, _) = h.student(&code, "Ben", "B2").await;
    let paper3 = other.start_exam().await["payload"]["paper"].clone();
    assert_ne!(order(&paper), order(&paper3), "orders differ between students");
    let mut a = order(&paper); let mut b = order(&paper3);
    a.sort(); b.sort();
    assert_eq!(a, b, "same questions, different order");
}

#[tokio::test]
async fn students_do_not_get_a_shuffle_when_randomisation_is_off() {
    let h = harness().await;
    let (_, _, _, _, paper) = running(&h, four_type_exam()).await;
    let texts: Vec<_> = paper["questions"].as_array().unwrap().iter().map(|q| q["text"].as_str().unwrap().to_string()).collect();
    assert_eq!(texts, ["Capital of France?", "CSS styles pages.", "Pick the languages", "Who wrote Hamlet?"]);
    let mc = &paper["questions"][0];
    let choices: Vec<_> = mc["choices"].as_array().unwrap().iter().map(|c| c["text"].as_str().unwrap()).collect();
    assert_eq!(choices, ["Paris", "Rome", "Oslo"]);
}

#[tokio::test]
async fn teacher_ending_the_session_grades_working_students_and_tells_them_first() {
    let h = harness().await;
    let mut exam = four_type_exam();
    exam.show_results = true;
    let (sid, code, mut ws, joined, paper) = running(&h, exam).await;
    answer_all_correctly(&mut ws, &paper).await;
    // A second student joined but never opened the exam.
    let (_idle, idle_joined) = h.student(&code, "Idle Ian", "I9").await;

    h.sessions.act(&h.user, &sid, SessionAction::End).await.unwrap();
    let pushed = ws.recv_kind("submitted").await;
    assert_eq!(pushed["payload"]["auto"], true);
    assert_eq!(pushed["payload"]["result"]["percentage"], 100.0);
    ws.recv_kind("session_ended").await;

    let a = h.db.get_attempt(joined["attemptId"].as_str().unwrap()).await.unwrap();
    assert_eq!((a.status, a.percentage), (AttemptStatus::AutoSubmitted, Some(100.0)));
    let idle = h.db.get_attempt(idle_joined["attemptId"].as_str().unwrap()).await.unwrap();
    assert_eq!((idle.status, idle.score), (AttemptStatus::Joined, None), "never started: left ungraded, not faked");
    let roster = h.sessions.roster(&sid).await.unwrap();
    assert_eq!(roster.iter().find(|r| r.student_number == "A1").unwrap().answered, 4);
}

#[tokio::test]
async fn time_running_out_auto_submits_when_enabled() {
    let h = harness().await;
    let (sid, _, mut ws, joined, paper) = running(&h, four_type_exam()).await;
    answer_all_correctly(&mut ws, &paper).await;
    h.set_ends_at(&sid, "2020-01-01T00:00:00.000Z").await;

    let mut notified = std::collections::HashSet::new();
    exam_engine::expire_due(&h.db, &h.hub, &mut notified).await.unwrap();
    let pushed = ws.recv_kind("submitted").await;
    assert_eq!(pushed["payload"]["auto"], true);
    assert_eq!(ws.recv_kind("time_up").await["payload"]["autoSubmit"], true);

    let a = h.db.get_attempt(joined["attemptId"].as_str().unwrap()).await.unwrap();
    assert_eq!((a.status, a.percentage), (AttemptStatus::AutoSubmitted, Some(100.0)));
    assert_eq!(ws.answer(&qid(&paper, "CSS"), json!(cid(&paper, "CSS", "False")), 99).await["payload"]["code"], "already_submitted");

    // The same expired deadline is announced only once.
    exam_engine::expire_due(&h.db, &h.hub, &mut notified).await.unwrap();
    ws.send(json!({ "id": "r1", "type": "heartbeat" })).await;
    assert_eq!(ws.recv_reply().await["type"], "heartbeat_ack", "no second time_up queued ahead of the ack");
}

#[tokio::test]
async fn without_auto_submit_time_up_locks_answers_but_the_student_can_still_submit() {
    let h = harness().await;
    let mut exam = four_type_exam();
    exam.auto_submit = false;
    let (sid, _, mut ws, joined, paper) = running(&h, exam).await;
    ws.answer(&qid(&paper, "Capital"), json!(cid(&paper, "Capital", "Paris")), 1).await;
    h.set_ends_at(&sid, "2020-01-01T00:00:00.000Z").await;
    exam_engine::expire_due(&h.db, &h.hub, &mut Default::default()).await.unwrap();
    assert_eq!(ws.recv_kind("time_up").await["payload"]["autoSubmit"], false);

    let a = h.db.get_attempt(joined["attemptId"].as_str().unwrap()).await.unwrap();
    assert_eq!(a.status, AttemptStatus::InProgress, "not auto-submitted");
    assert_eq!(ws.answer(&qid(&paper, "CSS"), json!(cid(&paper, "CSS", "True")), 2).await["payload"]["code"], "time_up");
    assert_eq!(ws.submit().await["type"], "submitted");
    let a = h.db.get_attempt(&a.id).await.unwrap();
    assert_eq!((a.status, a.score), (AttemptStatus::Submitted, Some(1.0)), "only the answer saved before the deadline counts");
}

#[tokio::test]
async fn an_answer_just_after_the_deadline_is_accepted_within_the_grace_window() {
    let h = harness().await;
    let (sid, _, mut ws, _, paper) = running(&h, four_type_exam()).await;
    let one_second_ago = proctorlan_lib::services::timer::format(chrono::Utc::now() - chrono::Duration::seconds(1));
    h.set_ends_at(&sid, &one_second_ago).await;
    let r = ws.answer(&qid(&paper, "CSS"), json!(cid(&paper, "CSS", "True")), 1).await;
    assert_eq!(r["type"], "answer_ack", "latency allowance: {r}");
    let ten_seconds_ago = proctorlan_lib::services::timer::format(chrono::Utc::now() - chrono::Duration::seconds(10));
    h.set_ends_at(&sid, &ten_seconds_ago).await;
    let r = ws.answer(&qid(&paper, "CSS"), json!(cid(&paper, "CSS", "False")), 2).await;
    assert_eq!(r["payload"]["code"], "time_up");
}

#[tokio::test]
async fn the_background_sweeper_enforces_the_deadline_without_any_teacher_action() {
    let h = harness().await;
    let (sid, _, mut ws, joined, paper) = running(&h, four_type_exam()).await;
    ws.answer(&qid(&paper, "CSS"), json!(cid(&paper, "CSS", "True")), 1).await;
    h.set_ends_at(&sid, "2020-01-01T00:00:00.000Z").await;
    tokio::time::timeout(Duration::from_secs(5), ws.recv_kind("time_up")).await.expect("sweeper never fired");
    let a = h.db.get_attempt(joined["attemptId"].as_str().unwrap()).await.unwrap();
    assert_eq!(a.status, AttemptStatus::AutoSubmitted);
}

#[tokio::test]
async fn a_paused_session_is_never_expired_even_with_a_past_deadline() {
    let h = harness().await;
    let (sid, _, mut ws, _, _) = running(&h, four_type_exam()).await;
    h.sessions.act(&h.user, &sid, SessionAction::Pause).await.unwrap();
    ws.recv_kind("session_paused").await;
    // Even with a deadline in the past, a PAUSED session is not expired.
    h.set_ends_at(&sid, "2020-01-01T00:00:00.000Z").await;
    exam_engine::expire_due(&h.db, &h.hub, &mut Default::default()).await.unwrap();
    let a = h.db.list_roster(&sid).await.unwrap();
    assert_eq!(a[0].status, AttemptStatus::InProgress);
}

#[tokio::test]
async fn one_students_connection_cannot_touch_anothers_attempt() {
    let h = harness().await;
    let s = h.open_session_for(four_type_exam()).await;
    let (sid, code) = (s.row.session.id.clone(), s.row.session.session_code.clone());
    let (mut a, _) = h.student(&code, "Ana", "A1").await;
    let (mut b, bj) = h.student(&code, "Ben", "B2").await;
    h.sessions.act(&h.user, &sid, SessionAction::Start).await.unwrap();
    let paper = a.start_exam().await["payload"]["paper"].clone();
    b.start_exam().await;
    a.answer(&qid(&paper, "CSS"), json!(cid(&paper, "CSS", "True")), 1).await;
    // Ben has no answers; Ana's submit does not touch Ben.
    a.submit().await;
    let ben = h.db.get_attempt(bj["attemptId"].as_str().unwrap()).await.unwrap();
    assert_eq!(ben.status, AttemptStatus::InProgress);
    assert_eq!(h.db.count_answers(&ben.id).await.unwrap(), 0);
    // Without joining, exam messages are refused outright.
    let mut anon = h.ws().await;
    anon.hello(&code).await.unwrap();
    assert_eq!(anon.start_exam().await["payload"]["code"], "join_required");
    assert_eq!(anon.submit().await["payload"]["code"], "join_required");
}
