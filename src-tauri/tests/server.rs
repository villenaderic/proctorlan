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
