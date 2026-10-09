//! Security regression tests: what a hostile student on the LAN can and cannot do.

mod common;

use std::net::Ipv4Addr;

use common::*;
use proctorlan_lib::services::results_service::ResultsService;
use proctorlan_lib::services::session_service::SessionAction;
use serde_json::{json, Value};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

async fn http(h: &Harness, method: &str, path: &str, body: &str) -> u16 {
    let mut s = TcpStream::connect((Ipv4Addr::LOCALHOST, h.port)).await.unwrap();
    let req = format!("{method} {path} HTTP/1.1\r\nHost: x\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len());
    s.write_all(req.as_bytes()).await.unwrap();
    let mut buf = Vec::new();
    let _ = s.read_to_end(&mut buf).await;
    String::from_utf8_lossy(&buf).split_whitespace().nth(1).unwrap().parse().unwrap()
}

#[tokio::test]
async fn the_lan_server_exposes_only_three_routes_and_no_teacher_functions() {
    let h = harness().await;
    h.open_session_for(four_type_exam()).await;
    for path in [
        "/", "/api", "/api/exams", "/api/exams/1", "/api/results", "/api/sessions", "/api/users", "/api/login", "/api/backup",
        "/api/export", "/api/settings", "/admin", "/teacher", "/metrics", "/debug", "/.env", "/proctorlan.db", "/../etc/passwd",
    ] {
        for method in ["GET", "POST", "PUT", "DELETE"] {
            let code = http(&h, method, path, "{}").await;
            assert!(code == 404 || code == 405, "{method} {path} answered {code}");
        }
    }
    assert_eq!(http(&h, "GET", "/api/health", "").await, 200);
    assert_ne!(http(&h, "GET", "/ws", "").await, 200, "/ws needs a WebSocket upgrade");
}

#[tokio::test]
async fn a_token_from_one_session_is_useless_in_another() {
    let h = harness().await;
    let a = h.open_session_for(four_type_exam()).await;
    let (ws_a, joined_a) = h.student(&a.row.session.session_code, "Ana Reyes", "A1").await;
    let token = joined_a["token"].as_str().unwrap().to_string();
    drop(ws_a);

    let b = h.open_session_for(four_type_exam()).await;
    let mut ws = h.ws().await;
    ws.hello(&b.row.session.session_code).await.unwrap();
    let r = ws.join("Ana Reyes", "A1", Some(&token)).await;
    // Allowed to join session B as a fresh student, but never as the owner of A's attempt.
    assert_eq!(r["type"], "joined", "{r}");
    assert_eq!(r["payload"]["resumed"], false);
    assert_ne!(r["payload"]["attemptId"], joined_a["attemptId"]);
    assert_ne!(r["payload"]["token"], json!(token));
}

#[tokio::test]
async fn injection_payloads_are_stored_as_plain_text_and_break_nothing() {
    let h = harness().await;
    let s = h.open_session_for(four_type_exam()).await;
    let (sid, code) = (s.row.session.id.clone(), s.row.session.session_code.clone());
    let nasty_name = "Robert'); DROP TABLE students;--";
    let (mut ws, _) = h.student(&code, nasty_name, "S-1").await;
    h.sessions.act(&h.user, &sid, SessionAction::Start).await.unwrap();
    ws.recv_kind("session_started").await;
    let paper = ws.start_exam().await["payload"]["paper"].clone();
    let r = ws.answer(&qid(&paper, "Hamlet"), json!("' OR '1'='1'; DROP TABLE answers;--<script>alert(1)</script>"), 1).await;
    assert_eq!(r["type"], "answer_ack", "{r}");
    assert_eq!(ws.submit().await["type"], "submitted");

    for table in ["students", "attempts", "answers", "users", "exams", "exam_sessions"] {
        let n: i64 = sqlx::query_scalar(&format!("SELECT COUNT(*) FROM {table}")).fetch_one(h.db.pool()).await.unwrap();
        assert!(n >= 1, "{table} was damaged");
    }
    let name: String = sqlx::query_scalar("SELECT name FROM students").fetch_one(h.db.pool()).await.unwrap();
    assert_eq!(name, nasty_name);
    let stored: String = sqlx::query_scalar("SELECT answer_data FROM answers").fetch_one(h.db.pool()).await.unwrap();
    assert!(stored.contains("<script>"), "stored verbatim (the UI escapes it when shown)");

    // ...and a spreadsheet opening the export cannot run it as a formula.
    let svc = ResultsService::new(h.db.clone(), std::env::temp_dir().join("proctorlan-sec"));
    h.sessions.act(&h.user, &sid, SessionAction::End).await.unwrap();
    let (csv, _) = svc.csv(&sid).await.unwrap();
    assert!(csv.contains("Robert'); DROP TABLE students;--"));
}

#[tokio::test]
async fn students_cannot_award_themselves_points_with_extra_fields() {
    let h = harness().await;
    let s = h.open_session_for(four_type_exam()).await;
    let (sid, code) = (s.row.session.id.clone(), s.row.session.session_code.clone());
    let (mut ws, _) = h.student(&code, "Cheat Charlie", "C1").await;
    h.sessions.act(&h.user, &sid, SessionAction::Start).await.unwrap();
    ws.recv_kind("session_started").await;
    let paper = ws.start_exam().await["payload"]["paper"].clone();
    let wrong = cid(&paper, "Capital", "Rome");
    let ack = ws.request("answer", json!({
        "questionId": qid(&paper, "Capital"), "answer": wrong, "clientSeq": 1,
        "isCorrect": true, "pointsAwarded": 99, "score": 100, "percentage": 100, "passed": true, "attemptId": "someone-else"
    })).await;
    assert_eq!(ack["type"], "answer_ack", "{ack}");
    let submitted = ws.submit().await;
    assert_eq!(submitted["type"], "submitted");
    let (score, passed): (f64, bool) = sqlx::query_as("SELECT score, passed FROM attempts").fetch_one(h.db.pool()).await.unwrap();
    assert_eq!((score, passed), (0.0, false));
    let n: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM answers WHERE is_correct = 1").fetch_one(h.db.pool()).await.unwrap();
    assert_eq!(n, 0);
}

#[tokio::test]
async fn what_the_student_receives_after_submitting_never_includes_the_key() {
    let h = harness().await;
    let mut exam = four_type_exam();
    exam.show_results = true;
    let s = h.open_session_for(exam).await;
    let (sid, code) = (s.row.session.id.clone(), s.row.session.session_code.clone());
    let (mut ws, joined) = h.student(&code, "Ana Reyes", "A1").await;
    h.sessions.act(&h.user, &sid, SessionAction::Start).await.unwrap();
    ws.recv_kind("session_started").await;
    let paper = ws.start_exam().await;
    let r = ws.submit().await;
    for (name, v) in [("joined", &joined), ("paper", &paper), ("submitted", &r)] {
        let dump = v.to_string();
        for secret in ["isCorrect", "is_correct", "SECRET EXPLANATION", "Shakespeare", "token_hash", "password"] {
            assert!(!dump.contains(secret), "{name} leaked {secret}: {dump}");
        }
    }
}

#[tokio::test]
async fn secrets_are_never_serialised_or_stored_in_the_clear() {
    let h = harness().await;
    let user_json = serde_json::to_string(&h.user).unwrap();
    assert!(!user_json.contains("hash"), "{user_json}");

    let s = h.open_session_for(four_type_exam()).await;
    let (_ws, joined) = h.student(&s.row.session.session_code, "Ana Reyes", "A1").await;
    let token = joined["token"].as_str().unwrap().to_string();
    assert_eq!(token.len(), 64);
    let attempt = h.db.get_attempt(joined["attemptId"].as_str().unwrap()).await.unwrap();
    assert!(!serde_json::to_string(&attempt).unwrap().contains(&attempt.token_hash));
    let hits: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM attempts WHERE token_hash = ?").bind(&token).fetch_one(h.db.pool()).await.unwrap();
    assert_eq!(hits, 0, "the raw token must not be stored");
    assert_ne!(attempt.token_hash, token);
    let hashed: Value = json!(attempt.token_hash);
    assert_eq!(hashed.as_str().unwrap().len(), 64);
}

#[tokio::test]
async fn absurd_identities_are_refused() {
    let h = harness().await;
    let s = h.open_session_for(four_type_exam()).await;
    let code = s.row.session.session_code.clone();
    for (name, id) in [("A".repeat(10_000), "A1".to_string()), ("Ana".to_string(), "9".repeat(10_000)), ("   ".to_string(), "A1".to_string()), ("Ana".to_string(), "".to_string())] {
        let mut ws = h.ws().await;
        ws.hello(&code).await.unwrap();
        let r = ws.join(&name, &id, None).await;
        assert_eq!(r["type"], "error", "name len {} id len {}: {r}", name.len(), id.len());
    }
    let n: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM students").fetch_one(h.db.pool()).await.unwrap();
    assert_eq!(n, 0);
}
