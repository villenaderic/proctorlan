//! Headless LAN server for development and interop tests (no UI, in-memory database).
//!
//!   cargo run --example dev_server
//!
//! Prints one JSON line `{"port":…,"code":"…","sessionId":"…"}`, then reads commands from stdin:
//! `start`, `pause`, `resume`, `end`, `roster`, `new` (opens a fresh session, prints its code), `quit`.
//! Each prints one JSON line. Commands act on the most recently opened session.

use std::io::BufRead;
use std::net::Ipv4Addr;
use std::sync::Arc;

use proctorlan_lib::database::Database;
use proctorlan_lib::models::*;
use proctorlan_lib::server::{hub::Hub, new_status, LanServer};
use proctorlan_lib::services::exam_service::ExamService;
use proctorlan_lib::services::session_service::{SessionAction, SessionService};

#[tokio::main]
async fn main() {
    let db = Database::open_in_memory().await.unwrap();
    let user = db.create_user("teacher", "hash", "Teacher", UserRole::Admin).await.unwrap();
    let (hub, status) = (Hub::new(), new_status());
    let lan = Arc::new(LanServer::new(db.clone(), hub.clone(), status.clone()));
    let st = lan.start(Ipv4Addr::LOCALHOST, 0, false).await.unwrap();
    let sessions = SessionService::new(db.clone(), hub, status);

    let ch = |t: &str, ok| NewChoice { choice_text: t.into(), is_correct: ok };
    let exams = ExamService::new(db.clone());
    let make_session = || async {
        let e = exams.create(&user, NewExam {
            title: "Dev Exam".into(), description: "Interop check".into(), instructions: "Do your best.".into(), duration_minutes: 30, passing_score: 60.0,
            randomize_questions: false, randomize_choices: false, allow_review: true, auto_submit: true, show_results: true,
            questions: vec![
                NewQuestion { question_text: "1+1?".into(), question_type: QuestionType::MultipleChoice, points: 1.0, required: true, explanation: None, choices: vec![ch("2", true), ch("3", false)] },
                NewQuestion { question_text: "Capital of France?".into(), question_type: QuestionType::Identification, points: 1.0, required: true, explanation: None, choices: vec![ch("Paris", true)] },
            ],
        }).await.unwrap();
        exams.set_active(&user, &e.exam.id, true).await.unwrap();
        sessions.create(&user, &e.exam.id).await.unwrap()
    };
    let s = make_session().await;
    println!("{}", serde_json::json!({ "port": st.port, "code": s.row.session.session_code, "sessionId": s.row.session.id }));

    let mut id = s.row.session.id.clone();
    let stdin = std::io::stdin();
    for line in stdin.lock().lines() {
        let line = line.unwrap();
        let act = match line.trim() {
            "start" => Some(SessionAction::Start),
            "pause" => Some(SessionAction::Pause),
            "resume" => Some(SessionAction::Resume),
            "end" => Some(SessionAction::End),
            "roster" => {
                println!("{}", serde_json::to_string(&sessions.roster(&id).await.unwrap()).unwrap());
                None
            }
            "new" => {
                let n = make_session().await;
                id = n.row.session.id.clone();
                println!("{}", serde_json::json!({ "code": n.row.session.session_code, "sessionId": id }));
                None
            }
            "quit" => break,
            _ => None,
        };
        if let Some(a) = act {
            let r = sessions.act(&user, &id, a).await;
            println!("{}", serde_json::json!({ "ok": r.is_ok() }));
        }
    }
}
