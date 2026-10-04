//! Shared harness: a real LAN server on an ephemeral 127.0.0.1 port plus a WebSocket test client.
#![allow(dead_code)]

use std::net::Ipv4Addr;
use std::sync::Arc;
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use proctorlan_lib::database::Database;
use proctorlan_lib::models::*;
use proctorlan_lib::server::{hub::Hub, new_status, LanServer};
use proctorlan_lib::services::exam_service::ExamService;
use proctorlan_lib::services::session_service::{SessionService, SessionSnapshot};
use serde_json::{json, Value};
use tokio::net::TcpStream;
use tokio_tungstenite::tungstenite::Message;

pub struct Harness {
    pub db: Database,
    pub user: User,
    pub hub: Arc<Hub>,
    pub lan: Arc<LanServer>,
    pub sessions: SessionService,
    pub port: u16,
}

pub fn ch(t: &str, ok: bool) -> NewChoice { NewChoice { choice_text: t.into(), is_correct: ok } }

pub fn question(ty: QuestionType, text: &str, points: f64, choices: Vec<NewChoice>) -> NewQuestion {
    NewQuestion { question_text: text.into(), question_type: ty, points, required: true, explanation: Some("SECRET EXPLANATION".into()), choices }
}

/// 4 questions, one of each type, 1+1+2+1 = 5 points.
pub fn four_type_exam() -> NewExam {
    NewExam {
        title: "Engine Exam".into(), description: "d".into(), instructions: "i".into(), duration_minutes: 30, passing_score: 60.0,
        randomize_questions: false, randomize_choices: false, allow_review: true, auto_submit: true, show_results: false,
        questions: vec![
            question(QuestionType::MultipleChoice, "Capital of France?", 1.0, vec![ch("Paris", true), ch("Rome", false), ch("Oslo", false)]),
            question(QuestionType::TrueFalse, "CSS styles pages.", 1.0, vec![ch("True", true), ch("False", false)]),
            question(QuestionType::MultipleSelect, "Pick the languages", 2.0, vec![ch("Rust", true), ch("Python", true), ch("HTML", false)]),
            question(QuestionType::Identification, "Who wrote Hamlet?", 1.0, vec![ch("Shakespeare", true), ch("William Shakespeare", true)]),
        ],
    }
}

pub async fn harness() -> Harness {
    let db = Database::open_in_memory().await.unwrap();
    let user = db.create_user("teacher", "hash", "Teacher", UserRole::Admin).await.unwrap();
    let hub = Hub::new();
    let status = new_status();
    let lan = Arc::new(LanServer::new(db.clone(), hub.clone(), status.clone()));
    let st = lan.start(Ipv4Addr::LOCALHOST, 0, false).await.unwrap();
    let sessions = SessionService::new(db.clone(), hub.clone(), status);
    Harness { db, user, hub, lan, sessions, port: st.port }
}

impl Harness {
    pub async fn open_session_for(&self, exam: NewExam) -> SessionSnapshot {
        let exams = ExamService::new(self.db.clone());
        let e = exams.create(&self.user, exam).await.unwrap();
        exams.set_active(&self.user, &e.exam.id, true).await.unwrap();
        self.sessions.create(&self.user, &e.exam.id).await.unwrap()
    }

    pub async fn ws(&self) -> Ws {
        let (ws, _) = tokio_tungstenite::connect_async(format!("ws://127.0.0.1:{}/ws", self.port)).await.unwrap();
        Ws(ws)
    }

    /// Connects, says hello and joins. Returns the socket and the `joined` payload.
    pub async fn student(&self, code: &str, name: &str, id: &str) -> (Ws, Value) {
        let mut ws = self.ws().await;
        ws.hello(code).await.unwrap();
        let j = ws.join(name, id, None).await;
        assert_eq!(j["type"], "joined", "{j}");
        (ws, j["payload"].clone())
    }

    /// Moves the session deadline, e.g. into the past to simulate time running out.
    pub async fn set_ends_at(&self, session_id: &str, ts: &str) {
        sqlx::query("UPDATE exam_sessions SET ends_at = ? WHERE id = ?").bind(ts).bind(session_id).execute(self.db.pool()).await.unwrap();
    }

    pub async fn full_exam(&self, exam_id: &str) -> ExamFull {
        self.db.get_exam_full(exam_id).await.unwrap()
    }
}

pub struct Ws(pub tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<TcpStream>>);

impl Ws {
    pub async fn send(&mut self, v: Value) { self.0.send(Message::Text(v.to_string().into())).await.unwrap(); }

    /// Next JSON frame, or None if the server closed the socket.
    pub async fn recv(&mut self) -> Option<Value> {
        loop {
            match tokio::time::timeout(Duration::from_secs(6), self.0.next()).await.expect("timed out waiting for server") {
                Some(Ok(Message::Text(t))) => return Some(serde_json::from_str(t.as_str()).unwrap()),
                Some(Ok(Message::Close(_))) | None | Some(Err(_)) => return None,
                Some(Ok(_)) => continue,
            }
        }
    }

    /// Receives frames until one of `kind` arrives (skips timer_sync and others).
    pub async fn recv_kind(&mut self, kind: &str) -> Value {
        for _ in 0..20 {
            let v = self.recv().await.unwrap_or_else(|| panic!("socket closed while waiting for {kind}"));
            if v["type"] == kind { return v; }
        }
        panic!("never received {kind}");
    }

    pub async fn hello(&mut self, code: &str) -> Option<Value> {
        self.send(json!({ "id": "h1", "type": "hello", "payload": { "sessionCode": code } })).await;
        self.recv().await
    }

    pub async fn join(&mut self, name: &str, id: &str, token: Option<&str>) -> Value {
        self.send(json!({ "id": "j1", "type": "join", "payload": { "studentName": name, "studentId": id, "token": token } })).await;
        self.recv().await.expect("server closed during join")
    }

    pub async fn request(&mut self, kind: &str, payload: Value) -> Value {
        self.send(json!({ "id": "r1", "type": kind, "payload": payload })).await;
        self.recv_reply().await
    }

    /// Next frame that answers our request (skips unsolicited pushes).
    pub async fn recv_reply(&mut self) -> Value {
        for _ in 0..20 {
            let v = self.recv().await.expect("socket closed while waiting for a reply");
            if v["payload"]["inReplyTo"] == "r1" { return v; }
        }
        panic!("no reply");
    }

    pub async fn start_exam(&mut self) -> Value { self.request("start_exam", json!({})).await }

    pub async fn answer(&mut self, qid: &str, answer: Value, seq: i64) -> Value {
        self.request("answer", json!({ "questionId": qid, "answer": answer, "clientSeq": seq })).await
    }

    pub async fn submit(&mut self) -> Value { self.request("submit", json!({})).await }
}

/// Finds a question id in the paper by (part of) its text.
pub fn qid(paper: &Value, text: &str) -> String {
    paper["questions"].as_array().unwrap().iter()
        .find(|q| q["text"].as_str().unwrap().contains(text)).unwrap_or_else(|| panic!("no question {text}"))["id"].as_str().unwrap().to_string()
}

/// Id of the choice with this text in the paper.
pub fn cid(paper: &Value, question_text: &str, choice_text: &str) -> String {
    let q = paper["questions"].as_array().unwrap().iter().find(|q| q["text"].as_str().unwrap().contains(question_text)).unwrap();
    q["choices"].as_array().unwrap().iter().find(|c| c["text"] == choice_text).unwrap_or_else(|| panic!("no choice {choice_text}"))["id"].as_str().unwrap().to_string()
}
