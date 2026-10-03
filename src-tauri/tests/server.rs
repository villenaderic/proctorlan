//! LAN server tests over real sockets on 127.0.0.1 (ephemeral ports). No mocks.

use std::net::Ipv4Addr;
use std::sync::Arc;
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use proctorlan_lib::database::Database;
use proctorlan_lib::errors::AppError;
use proctorlan_lib::models::*;
use proctorlan_lib::server::{hub::Hub, new_status, LanServer};
use proctorlan_lib::services::exam_service::ExamService;
use proctorlan_lib::services::network_service::{validate_port, NetworkService};
use proctorlan_lib::services::session_service::{SessionAction, SessionService, SessionSnapshot};
use serde_json::{json, Value};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio_tungstenite::tungstenite::Message;

struct Harness {
    db: Database,
    user: User,
    lan: Arc<LanServer>,
    sessions: SessionService,
    port: u16,
}

fn ch(t: &str, ok: bool) -> NewChoice { NewChoice { choice_text: t.into(), is_correct: ok } }

fn exam() -> NewExam {
    NewExam {
        title: "LAN Exam".into(), description: "d".into(), instructions: "i".into(), duration_minutes: 30, passing_score: 60.0,
        randomize_questions: false, randomize_choices: false, allow_review: true, auto_submit: true, show_results: false,
        questions: vec![NewQuestion {
            question_text: "Pick one".into(), question_type: QuestionType::MultipleChoice, points: 1.0, required: true, explanation: None,
            choices: vec![ch("A", true), ch("B", false)],
        }],
    }
}

async fn harness() -> Harness {
    let db = Database::open_in_memory().await.unwrap();
    let user = db.create_user("teacher", "hash", "Teacher", UserRole::Admin).await.unwrap();
    let hub = Hub::new();
    let status = new_status();
    let lan = Arc::new(LanServer::new(db.clone(), hub.clone(), status.clone()));
    let st = lan.start(Ipv4Addr::LOCALHOST, 0, false).await.unwrap();
    let sessions = SessionService::new(db.clone(), hub, status);
    Harness { db, user, lan, sessions, port: st.port }
}

impl Harness {
    async fn open_session(&self) -> SessionSnapshot {
        let exams = ExamService::new(self.db.clone());
        let e = exams.create(&self.user, exam()).await.unwrap();
        exams.set_active(&self.user, &e.exam.id, true).await.unwrap();
        self.sessions.create(&self.user, &e.exam.id).await.unwrap()
    }
    async fn http(&self, method: &str, path: &str, body: &str) -> (u16, Value) {
        let mut s = TcpStream::connect((Ipv4Addr::LOCALHOST, self.port)).await.unwrap();
        let req = format!("{method} {path} HTTP/1.1\r\nHost: x\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len());
        s.write_all(req.as_bytes()).await.unwrap();
        let mut buf = Vec::new();
        let _ = s.read_to_end(&mut buf).await;
        let text = String::from_utf8_lossy(&buf).to_string();
        let code: u16 = text.split_whitespace().nth(1).unwrap().parse().unwrap();
        let json = text.split("\r\n\r\n").nth(1).and_then(|b| serde_json::from_str(b.trim()).ok()).unwrap_or(Value::Null);
        (code, json)
    }
    async fn ws(&self) -> Ws {
        let (ws, _) = tokio_tungstenite::connect_async(format!("ws://127.0.0.1:{}/ws", self.port)).await.unwrap();
        Ws(ws)
    }
}

struct Ws(tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<TcpStream>>);

impl Ws {
    async fn send(&mut self, v: Value) { self.0.send(Message::Text(v.to_string().into())).await.unwrap(); }
    async fn raw(&mut self, s: String) { self.0.send(Message::Text(s.into())).await.unwrap(); }
    /// Next JSON text frame, or None if the server closed the socket.
    async fn recv(&mut self) -> Option<Value> {
        loop {
            match tokio::time::timeout(Duration::from_secs(5), self.0.next()).await.expect("timed out waiting for server") {
                Some(Ok(Message::Text(t))) => return Some(serde_json::from_str(t.as_str()).unwrap()),
                Some(Ok(Message::Close(_))) | None | Some(Err(_)) => return None,
                Some(Ok(_)) => continue,
            }
        }
    }
    async fn hello(&mut self, code: &str) -> Option<Value> {
        self.send(json!({ "id": "h1", "type": "hello", "payload": { "sessionCode": code } })).await;
        self.recv().await
    }
}

async fn wait_connected(h: &Harness, id: &str, n: usize) {
    for _ in 0..50 {
        if h.sessions.snapshot(id).await.unwrap().connected == n { return; }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    panic!("connected count never reached {n}");
}

#[tokio::test]
async fn health_endpoint_identifies_the_app_without_data() {
    let h = harness().await;
    let (code, body) = h.http("GET", "/api/health", "").await;
    assert_eq!(code, 200);
    assert_eq!(body["app"], "ProctorLAN");
    assert_eq!(body["ok"], true);
    assert!(body.get("sessions").is_none());
}

#[tokio::test]
async fn lookup_finds_open_sessions_and_never_leaks_answers_or_codes() {
    let h = harness().await;
    let s = h.open_session().await;
    let code = s.row.session.session_code.clone();
    let (status, body) = h.http("POST", "/api/sessions/lookup", &json!({ "sessionCode": code.to_lowercase() }).to_string()).await;
    assert_eq!(status, 200);
    assert_eq!(body["examTitle"], "LAN Exam");
    assert_eq!(body["status"], "WAITING");
    let dump = body.to_string();
    assert!(!dump.contains("isCorrect") && !dump.contains(&code));
    let (status, _) = h.http("POST", "/api/sessions/lookup", &json!({ "sessionCode": "ZZZZZ" }).to_string()).await;
    assert_eq!(status, 404);
}

#[tokio::test]
async fn too_many_wrong_codes_block_the_address() {
    let h = harness().await;
    h.open_session().await;
    let mut last = 0;
    for _ in 0..12 {
        last = h.http("POST", "/api/sessions/lookup", r#"{"sessionCode":"NOPE1"}"#).await.0;
    }
    assert_eq!(last, 429);
    // The block also applies to WebSocket joins from that address.
    let mut ws = h.ws().await;
    let first = ws.recv().await.unwrap();
    assert_eq!(first["payload"]["code"], "too_many_attempts");
}

#[tokio::test]
async fn oversized_http_body_is_rejected() {
    let h = harness().await;
    let big = format!(r#"{{"sessionCode":"{}"}}"#, "A".repeat(300 * 1024));
    let (code, _) = h.http("POST", "/api/sessions/lookup", &big).await;
    assert_eq!(code, 413);
}

#[tokio::test]
async fn websocket_hello_returns_welcome_with_server_time_and_no_secrets() {
    let h = harness().await;
    let s = h.open_session().await;
    let mut ws = h.ws().await;
    let welcome = ws.hello(&s.row.session.session_code).await.unwrap();
    assert_eq!(welcome["type"], "welcome");
    assert_eq!(welcome["payload"]["inReplyTo"], "h1");
    assert_eq!(welcome["payload"]["sessionId"], s.row.session.id);
    assert_eq!(welcome["payload"]["status"], "WAITING");
    assert!(welcome["payload"]["serverTime"].is_string());
    assert!(welcome["payload"]["remainingSeconds"].is_null());
    assert_eq!(welcome["payload"]["heartbeatIntervalMs"], 5000);
    assert!(!welcome.to_string().contains("isCorrect"));
}

#[tokio::test]
async fn heartbeat_is_acknowledged() {
    let h = harness().await;
    let s = h.open_session().await;
    let mut ws = h.ws().await;
    ws.hello(&s.row.session.session_code).await.unwrap();
    ws.send(json!({ "id": "hb7", "type": "heartbeat" })).await;
    let ack = ws.recv().await.unwrap();
    assert_eq!(ack["type"], "heartbeat_ack");
    assert_eq!(ack["payload"]["inReplyTo"], "hb7");
}

#[tokio::test]
async fn wrong_code_gets_an_error_and_the_socket_closes() {
    let h = harness().await;
    h.open_session().await;
    let mut ws = h.ws().await;
    let err = ws.hello("XXXXX").await.unwrap();
    assert_eq!(err["type"], "error");
    assert_eq!(err["payload"]["code"], "session_not_found");
    assert!(ws.recv().await.is_none(), "server must close after a bad code");
}

#[tokio::test]
async fn first_frame_must_be_hello() {
    let h = harness().await;
    let s = h.open_session().await;
    let mut ws = h.ws().await;
    ws.send(json!({ "type": "heartbeat" })).await;
    assert_eq!(ws.recv().await.unwrap()["payload"]["code"], "hello_required");
    assert!(ws.recv().await.is_none());
    assert_eq!(h.sessions.snapshot(&s.row.session.id).await.unwrap().connected, 0);
}

#[tokio::test]
async fn malformed_and_unknown_messages_do_not_kill_a_good_connection() {
    let h = harness().await;
    let s = h.open_session().await;
    let mut ws = h.ws().await;
    ws.hello(&s.row.session.session_code).await.unwrap();
    ws.raw("{this is not json".into()).await;
    assert_eq!(ws.recv().await.unwrap()["payload"]["code"], "malformed_message");
    ws.send(json!({ "type": "drop_all_tables" })).await;
    assert_eq!(ws.recv().await.unwrap()["payload"]["code"], "unknown_type");
    ws.send(json!({ "type": "heartbeat" })).await;
    assert_eq!(ws.recv().await.unwrap()["type"], "heartbeat_ack");
}

#[tokio::test]
async fn oversized_websocket_message_closes_the_connection() {
    let h = harness().await;
    let s = h.open_session().await;
    let mut ws = h.ws().await;
    ws.hello(&s.row.session.session_code).await.unwrap();
    let _ = ws.0.send(Message::Text("x".repeat(100 * 1024).into())).await;
    assert!(ws.recv().await.is_none());
    wait_connected(&h, &s.row.session.id, 0).await;
}

#[tokio::test]
async fn connected_count_follows_connections() {
    let h = harness().await;
    let s = h.open_session().await;
    let id = s.row.session.id.clone();
    let mut a = h.ws().await;
    let mut b = h.ws().await;
    a.hello(&s.row.session.session_code).await.unwrap();
    b.hello(&s.row.session.session_code).await.unwrap();
    wait_connected(&h, &id, 2).await;
    drop(a);
    wait_connected(&h, &id, 1).await;
    drop(b);
    wait_connected(&h, &id, 0).await;
}

#[tokio::test]
async fn teacher_actions_are_broadcast_with_a_server_deadline() {
    let h = harness().await;
    let s = h.open_session().await;
    let id = s.row.session.id.clone();
    let mut ws = h.ws().await;
    ws.hello(&s.row.session.session_code).await.unwrap();

    let snap = h.sessions.act(&h.user, &id, SessionAction::Start).await.unwrap();
    assert_eq!(snap.row.session.status, SessionStatus::Running);
    let ev = ws.recv().await.unwrap();
    assert_eq!(ev["type"], "session_started");
    assert_eq!(ev["sessionId"], id);
    assert_eq!(ev["payload"]["endsAt"], snap.row.session.ends_at.clone().unwrap());
    let remaining = ev["payload"]["remainingSeconds"].as_i64().unwrap();
    assert!((1795..=1800).contains(&remaining), "{remaining}");

    h.sessions.act(&h.user, &id, SessionAction::Pause).await.unwrap();
    let paused = ws.recv().await.unwrap();
    assert_eq!(paused["type"], "session_paused");
    let frozen = paused["payload"]["remainingSeconds"].as_i64().unwrap();
    tokio::time::sleep(Duration::from_millis(1200)).await;
    assert_eq!(h.sessions.snapshot(&id).await.unwrap().remaining_seconds, Some(frozen), "pause must freeze the clock");

    h.sessions.act(&h.user, &id, SessionAction::Resume).await.unwrap();
    assert_eq!(ws.recv().await.unwrap()["type"], "session_resumed");
    h.sessions.act(&h.user, &id, SessionAction::End).await.unwrap();
    assert_eq!(ws.recv().await.unwrap()["type"], "session_ended");
}

#[tokio::test]
async fn events_for_other_sessions_are_not_delivered() {
    let h = harness().await;
    let a = h.open_session().await;
    let b = h.open_session().await;
    let mut ws = h.ws().await;
    ws.hello(&a.row.session.session_code).await.unwrap();
    h.sessions.act(&h.user, &b.row.session.id, SessionAction::Start).await.unwrap();
    h.sessions.act(&h.user, &a.row.session.id, SessionAction::Start).await.unwrap();
    let ev = ws.recv().await.unwrap();
    assert_eq!(ev["sessionId"], a.row.session.id, "must skip session B's broadcast");
}

#[tokio::test]
async fn session_rules_are_enforced_for_the_teacher() {
    let h = harness().await;
    let s = h.open_session().await;
    let id = s.row.session.id.clone();
    assert!(matches!(h.sessions.act(&h.user, &id, SessionAction::Resume).await, Err(AppError::Conflict(_))));
    assert!(matches!(h.sessions.act(&h.user, &id, SessionAction::Pause).await, Err(AppError::Conflict(_))));
    h.sessions.act(&h.user, &id, SessionAction::Start).await.unwrap();
    assert!(matches!(h.sessions.act(&h.user, &id, SessionAction::Start).await, Err(AppError::Conflict(_))));
    h.sessions.act(&h.user, &id, SessionAction::End).await.unwrap();
    assert!(matches!(h.sessions.act(&h.user, &id, SessionAction::Start).await, Err(AppError::Conflict(_))));
    // Ended sessions stop accepting joins.
    let mut ws = h.ws().await;
    assert_eq!(ws.hello(&s.row.session.session_code).await.unwrap()["payload"]["code"], "session_not_found");
}

#[tokio::test]
async fn sessions_need_an_active_ready_exam_and_a_running_server() {
    let h = harness().await;
    let exams = ExamService::new(h.db.clone());
    let e = exams.create(&h.user, exam()).await.unwrap();
    let err = h.sessions.create(&h.user, &e.exam.id).await.unwrap_err();
    assert!(matches!(&err, AppError::Conflict(m) if m.contains("Activate")), "{err}");
    exams.set_active(&h.user, &e.exam.id, true).await.unwrap();
    h.lan.stop().await;
    let err = h.sessions.create(&h.user, &e.exam.id).await.unwrap_err();
    assert!(matches!(&err, AppError::Conflict(m) if m.contains("not running")), "{err}");
}

#[tokio::test]
async fn stopping_the_server_closes_the_port() {
    let h = harness().await;
    h.lan.stop().await;
    assert!(!h.lan.status().running);
    assert!(TcpStream::connect((Ipv4Addr::LOCALHOST, h.port)).await.is_err());
}

#[tokio::test]
async fn port_in_use_is_reported_clearly_not_fatal() {
    let h = harness().await;
    let other = LanServer::new(h.db.clone(), Hub::new(), new_status());
    let err = other.start(Ipv4Addr::LOCALHOST, h.port, false).await.unwrap_err();
    assert!(matches!(&err, AppError::Conflict(m) if m.contains("already in use")), "{err}");
    let st = other.status();
    assert!(!st.running);
    assert!(st.error.unwrap().contains("already in use"));
}

#[tokio::test]
async fn network_settings_are_validated_and_locked_during_an_open_session() {
    let h = harness().await;
    assert!(validate_port(80).is_err() && validate_port(70_000).is_err());
    assert_eq!(validate_port(38_123).unwrap(), 38_123);
    let net = NetworkService::new(h.db.clone(), h.lan.clone());
    assert!(matches!(net.apply(&h.user, None, 80).await, Err(AppError::Validation(_))));
    assert!(matches!(net.apply(&h.user, Some("203.0.113.9".into()), 40_000).await, Err(AppError::Validation(_))));
    h.open_session().await;
    let err = net.apply(&h.user, None, 40_001).await.unwrap_err();
    assert!(matches!(&err, AppError::Conflict(m) if m.contains("End the open session")), "{err}");
    // Nothing was saved by the refused change.
    assert_eq!(h.db.get_setting("network.port").await.unwrap(), None);
}

// ---------------------------------------------------------------- Phase 6: student join

impl Ws {
    async fn join(&mut self, name: &str, id: &str, token: Option<&str>) -> Value {
        self.send(json!({ "id": "j1", "type": "join", "payload": { "studentName": name, "studentId": id, "token": token } })).await;
        self.recv().await.expect("server closed during join")
    }
}

async fn joined_socket(h: &Harness, code: &str, name: &str, id: &str) -> (Ws, Value) {
    let mut ws = h.ws().await;
    ws.hello(code).await.unwrap();
    let j = ws.join(name, id, None).await;
    (ws, j)
}

#[tokio::test]
async fn join_creates_an_attempt_and_returns_a_token_and_exam_info_but_no_questions() {
    let h = harness().await;
    let s = h.open_session().await;
    let (_ws, j) = joined_socket(&h, &s.row.session.session_code, "  Ana   Reyes ", "2024-001a").await;
    assert_eq!(j["type"], "joined", "{j}");
    let p = &j["payload"];
    assert_eq!(p["studentName"], "Ana Reyes");
    assert_eq!(p["studentId"], "2024-001A");
    assert_eq!(p["resumed"], false);
    assert_eq!(p["token"].as_str().unwrap().len(), 64);
    assert_eq!(p["exam"]["title"], "LAN Exam");
    assert_eq!(p["exam"]["questionCount"], 1);
    assert_eq!(p["exam"]["durationMinutes"], 30);
    let dump = j.to_string();
    assert!(!dump.contains("Pick one") && !dump.contains("isCorrect") && !dump.contains("questions"), "no question content before the exam starts");

    // Only a hash is stored, never the token.
    let stored: String = sqlx::query_scalar("SELECT token_hash FROM attempts").fetch_one(h.db.pool()).await.unwrap();
    assert_ne!(stored, p["token"].as_str().unwrap());
    assert_eq!(stored.len(), 64);
}

#[tokio::test]
async fn roster_tracks_who_is_online() {
    let h = harness().await;
    let s = h.open_session().await;
    let id = s.row.session.id.clone();
    let (ws, _) = joined_socket(&h, &s.row.session.session_code, "Ana Reyes", "A1").await;
    let (_ws2, _) = joined_socket(&h, &s.row.session.session_code, "Ben Cruz", "B2").await;
    let roster = h.sessions.roster(&id).await.unwrap();
    assert_eq!(roster.len(), 2);
    assert!(roster.iter().all(|r| r.online && r.status == AttemptStatus::Joined));
    assert_eq!(h.sessions.snapshot(&id).await.unwrap().online, 2);
    drop(ws);
    for _ in 0..50 {
        if h.sessions.snapshot(&id).await.unwrap().online == 1 { break; }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    let roster = h.sessions.roster(&id).await.unwrap();
    assert_eq!(roster.iter().filter(|r| r.online).count(), 1);
    assert!(!roster.iter().find(|r| r.name == "Ana Reyes").unwrap().online, "a dropped student is offline but stays on the roster");
    assert_eq!(h.sessions.snapshot(&id).await.unwrap().joined, 2);
}

#[tokio::test]
async fn same_student_id_cannot_be_claimed_without_the_token() {
    let h = harness().await;
    let s = h.open_session().await;
    let code = s.row.session.session_code.clone();
    let (ws, first) = joined_socket(&h, &code, "Ana Reyes", "A1").await;
    let token = first["payload"]["token"].as_str().unwrap().to_string();
    let attempt = first["payload"]["attemptId"].as_str().unwrap().to_string();
    drop(ws);

    // An impostor (or a lost token) is refused and nothing changes.
    let mut imp = h.ws().await;
    imp.hello(&code).await.unwrap();
    assert_eq!(imp.join("Someone Else", "a1", None).await["payload"]["code"], "already_joined");
    assert_eq!(imp.join("Someone Else", "A1", Some("wrong-token")).await["payload"]["code"], "already_joined");
    let name: String = sqlx::query_scalar("SELECT name FROM students WHERE student_number='A1'").fetch_one(h.db.pool()).await.unwrap();
    assert_eq!(name, "Ana Reyes", "a refused join must not rename the student");

    // The real student resumes the same attempt with the saved token.
    let mut back = h.ws().await;
    back.hello(&code).await.unwrap();
    let again = back.join("Ana Reyes", "A1", Some(&token)).await;
    assert_eq!(again["type"], "joined");
    assert_eq!(again["payload"]["resumed"], true);
    assert_eq!(again["payload"]["attemptId"], attempt);
    assert!(again["payload"]["token"].is_null(), "a token is only issued once");
    let n: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM attempts").fetch_one(h.db.pool()).await.unwrap();
    assert_eq!(n, 1);
}

#[tokio::test]
async fn simultaneous_joins_for_one_student_id_create_one_attempt() {
    let h = harness().await;
    let s = h.open_session().await;
    let code = s.row.session.session_code.clone();
    let mut a = h.ws().await;
    let mut b = h.ws().await;
    a.hello(&code).await.unwrap();
    b.hello(&code).await.unwrap();
    let (ra, rb) = tokio::join!(a.join("Ana", "Z9", None), b.join("Ana", "Z9", None));
    let kinds = [ra["type"].as_str().unwrap(), rb["type"].as_str().unwrap()];
    assert_eq!(kinds.iter().filter(|k| **k == "joined").count(), 1, "{ra} {rb}");
    let n: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM attempts").fetch_one(h.db.pool()).await.unwrap();
    assert_eq!(n, 1);
}

#[tokio::test]
async fn bad_identities_and_double_joins_get_clear_errors() {
    let h = harness().await;
    let s = h.open_session().await;
    let mut ws = h.ws().await;
    ws.hello(&s.row.session.session_code).await.unwrap();
    for (name, id) in [("", "A1"), ("Ana", ""), ("Ana", "bad id!"), ("Ana", "'; DROP TABLE students;--")] {
        let r = ws.join(name, id, None).await;
        assert_eq!(r["payload"]["code"], "invalid_identity", "{name:?} {id:?}");
        assert!(r["payload"]["message"].as_str().unwrap().len() > 5);
    }
    assert_eq!(ws.join("Ana", "A1", None).await["type"], "joined");
    assert_eq!(ws.join("Ana", "A2", None).await["payload"]["code"], "invalid_identity", "one join per connection");
    let n: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM attempts").fetch_one(h.db.pool()).await.unwrap();
    assert_eq!(n, 1);
}

#[tokio::test]
async fn joining_is_refused_once_the_session_has_ended() {
    let h = harness().await;
    let s = h.open_session().await;
    let mut ws = h.ws().await;
    ws.hello(&s.row.session.session_code).await.unwrap();
    h.sessions.act(&h.user, &s.row.session.id, SessionAction::End).await.unwrap();
    assert_eq!(ws.recv().await.unwrap()["type"], "session_ended");
    assert_eq!(ws.join("Ana", "A1", None).await["payload"]["code"], "session_closed");
}

#[tokio::test]
async fn students_can_join_while_the_exam_is_running_or_paused() {
    let h = harness().await;
    let s = h.open_session().await;
    let id = s.row.session.id.clone();
    h.sessions.act(&h.user, &id, SessionAction::Start).await.unwrap();
    let (_a, j) = joined_socket(&h, &s.row.session.session_code, "Late Larry", "L1").await;
    assert_eq!(j["payload"]["status"], "RUNNING");
    assert!(j["payload"]["remainingSeconds"].as_i64().unwrap() > 1700);
    h.sessions.act(&h.user, &id, SessionAction::Pause).await.unwrap();
    let (_b, j) = joined_socket(&h, &s.row.session.session_code, "Paused Pat", "P1").await;
    assert_eq!(j["payload"]["status"], "PAUSED");
}

#[tokio::test]
async fn teacher_can_remove_an_unstarted_student_who_is_then_kicked_and_can_rejoin() {
    let h = harness().await;
    let s = h.open_session().await;
    let code = s.row.session.session_code.clone();
    let (mut ws, j) = joined_socket(&h, &code, "Ana Reyes", "A1").await;
    let attempt = j["payload"]["attemptId"].as_str().unwrap().to_string();
    let (mut bystander, _) = joined_socket(&h, &code, "Ben Cruz", "B2").await;

    h.sessions.remove_student(&h.user, &attempt).await.unwrap();
    assert_eq!(ws.recv().await.unwrap()["type"], "removed");
    assert!(ws.recv().await.is_none(), "removed student's socket is closed");
    // Others are untouched.
    bystander.send(json!({ "type": "heartbeat" })).await;
    assert_eq!(bystander.recv().await.unwrap()["type"], "heartbeat_ack");

    let (_ws2, j2) = joined_socket(&h, &code, "Ana Reyes", "A1").await;
    assert_eq!(j2["payload"]["resumed"], false, "a fresh attempt is created");
    assert_eq!(h.sessions.roster(&s.row.session.id).await.unwrap().len(), 2);
}

#[tokio::test]
async fn a_student_who_has_started_cannot_be_removed() {
    let h = harness().await;
    let s = h.open_session().await;
    let (_ws, j) = joined_socket(&h, &s.row.session.session_code, "Ana", "A1").await;
    let attempt = j["payload"]["attemptId"].as_str().unwrap().to_string();
    h.db.set_attempt_status(&attempt, AttemptStatus::InProgress).await.unwrap();
    let err = h.sessions.remove_student(&h.user, &attempt).await.unwrap_err();
    assert!(matches!(&err, AppError::Conflict(m) if m.contains("already started")), "{err}");
    assert_eq!(h.sessions.roster(&s.row.session.id).await.unwrap().len(), 1);
}
