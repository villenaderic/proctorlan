//! One WebSocket connection. Phase 5 scope: hello → welcome, heartbeat, timer sync, session broadcasts.
//! Student join (name/ID), answers and proctor events arrive in Phases 6–9 as new message types.

use std::net::IpAddr;

use axum::extract::ws::{CloseFrame, Message, WebSocket};
use chrono::Utc;
use serde_json::json;
use tokio::sync::broadcast::error::RecvError;
use tokio::time::{interval, timeout, Instant};

use crate::config;
use crate::models::ExamSession;
use crate::server::hub::OnlineGuard;
use crate::server::AppState;
use crate::services::exam_engine::{self, EngineError};
use crate::services::join_service::{self, JoinError};
use crate::services::timer;
use crate::websocket::protocol::{client, parse_frame, server as msg, Envelope};

const CLOSE_POLICY: u16 = 1008;
const CLOSE_TOO_BIG: u16 = 1009;

async fn send(socket: &mut WebSocket, env: &Envelope) -> bool {
    socket.send(Message::Text(env.to_text().into())).await.is_ok()
}

async fn close_with(socket: &mut WebSocket, code: u16, reason: &'static str) {
    let _ = socket.send(Message::Close(Some(CloseFrame { code, reason: reason.into() }))).await;
}

fn timer_payload(s: &ExamSession) -> serde_json::Value {
    let now = Utc::now();
    json!({
        "status": s.status,
        "endsAt": s.ends_at,
        "remainingSeconds": timer::remaining_seconds(
            s.ends_at.as_deref().and_then(timer::parse),
            s.paused_at.as_deref().and_then(timer::parse),
            now,
        ),
        "serverTime": timer::format(now),
    })
}

/// Reads the next text frame. `Ok(None)` = peer closed; `Err(code)` = protocol violation.
async fn next_text(socket: &mut WebSocket) -> Result<Option<String>, &'static str> {
    loop {
        match socket.recv().await {
            None | Some(Err(_)) => return Ok(None),
            Some(Ok(Message::Text(t))) => return Ok(Some(t.as_str().to_string())),
            Some(Ok(Message::Close(_))) => return Ok(None),
            Some(Ok(Message::Binary(_))) => return Err("binary_not_supported"),
            Some(Ok(_)) => continue, // ping/pong handled by the library
        }
    }
}

pub async fn handle_socket(mut socket: WebSocket, st: AppState, ip: IpAddr) {
    if st.hub.is_blocked(ip) {
        let _ = send(&mut socket, &Envelope::error("too_many_attempts", "Too many wrong codes. Wait a minute and try again.")).await;
        close_with(&mut socket, CLOSE_POLICY, "blocked").await;
        return;
    }

    // 1. The first frame must be a valid hello, soon.
    let hello = match timeout(config::WS_HELLO_TIMEOUT, next_text(&mut socket)).await {
        Err(_) => return close_with(&mut socket, CLOSE_POLICY, "hello timeout").await,
        Ok(Ok(Some(t))) => t,
        Ok(Ok(None)) => return,
        Ok(Err(code)) => {
            let _ = send(&mut socket, &Envelope::error(code, "Unsupported message.")).await;
            return close_with(&mut socket, CLOSE_POLICY, "unsupported").await;
        }
    };
    let hello = match parse_frame(&hello, config::MAX_WS_MESSAGE_BYTES) {
        Ok(e) if e.kind == client::HELLO => e,
        Ok(_) => {
            let _ = send(&mut socket, &Envelope::error("hello_required", "Send hello first.")).await;
            return close_with(&mut socket, CLOSE_POLICY, "hello required").await;
        }
        Err(code) => {
            let _ = send(&mut socket, &Envelope::error(code, "That message could not be read.")).await;
            let c = if code == "message_too_large" { CLOSE_TOO_BIG } else { CLOSE_POLICY };
            return close_with(&mut socket, c, "bad message").await;
        }
    };

    // 2. Resolve the session code. Wrong guesses are counted per address.
    let code = hello.payload.get("sessionCode").and_then(|v| v.as_str()).unwrap_or("");
    let session = match st.db.find_open_session_by_code(code).await {
        Ok(Some(s)) => s,
        Ok(None) => {
            st.hub.note_failure(ip);
            let _ = send(&mut socket, &Envelope::reply(msg::ERROR, &hello, None, json!({ "code": "session_not_found", "message": "No open session has that code." }))).await;
            return close_with(&mut socket, CLOSE_POLICY, "session not found").await;
        }
        Err(e) => {
            tracing::error!(error = %e, "session lookup failed");
            let _ = send(&mut socket, &Envelope::error("internal", "The server hit an error.")).await;
            return;
        }
    };
    let Some(_guard) = st.hub.try_join(&session.id) else {
        let _ = send(&mut socket, &Envelope::error("session_full", "This session is full.")).await;
        return close_with(&mut socket, CLOSE_POLICY, "session full").await;
    };

    let mut payload = timer_payload(&session);
    if let Some(o) = payload.as_object_mut() {
        o.insert("sessionId".into(), json!(session.id));
        o.insert("heartbeatIntervalMs".into(), json!(config::HEARTBEAT_INTERVAL.as_millis() as u64));
    }
    if !send(&mut socket, &Envelope::reply(msg::WELCOME, &hello, Some(&session.id), payload)).await {
        return;
    }
    tracing::info!(session_id = %session.id, %ip, "client connected");

    // 3. Main loop.
    let mut events = st.hub.subscribe();
    let mut sync = interval(config::TIMER_SYNC_INTERVAL);
    sync.tick().await; // first tick fires immediately; welcome already carried the time
    let idle = config::disconnect_timeout();
    let mut last_heard = Instant::now();
    // Set once this connection has joined as a student; the guard marks them online.
    let mut joined: Option<(String, OnlineGuard)> = None;

    loop {
        let deadline = last_heard + idle;
        tokio::select! {
            frame = socket.recv() => {
                last_heard = Instant::now();
                let text = match frame {
                    None | Some(Err(_)) | Some(Ok(Message::Close(_))) => break,
                    Some(Ok(Message::Text(t))) => t,
                    Some(Ok(Message::Binary(_))) => {
                        let _ = send(&mut socket, &Envelope::error("binary_not_supported", "Unsupported message.")).await;
                        continue;
                    }
                    Some(Ok(_)) => continue,
                };
                let env = match parse_frame(text.as_str(), config::MAX_WS_MESSAGE_BYTES) {
                    Ok(e) => e,
                    Err(code) => {
                        if !send(&mut socket, &Envelope::error(code, "That message could not be read.")).await { break; }
                        continue;
                    }
                };
                match env.kind.as_str() {
                    client::HEARTBEAT => {
                        let ack = Envelope::reply(msg::HEARTBEAT_ACK, &env, Some(&session.id), json!({ "serverTime": timer::format(Utc::now()) }));
                        if !send(&mut socket, &ack).await { break; }
                    }
                    client::JOIN => {
                        let reply = handle_join(&st, &session, &env, &mut joined).await;
                        if !send(&mut socket, &reply).await { break; }
                    }
                    client::START_EXAM | client::ANSWER | client::SUBMIT => {
                        let reply = match joined.as_ref() {
                            None => Envelope::reply(msg::ERROR, &env, Some(&session.id), json!({ "code": "join_required", "message": "Join the session first." })),
                            Some((attempt_id, _)) => handle_exam_message(&st, &session.id, attempt_id, &env).await,
                        };
                        if !send(&mut socket, &reply).await { break; }
                    }
                    other => {
                        let e = Envelope::reply(msg::ERROR, &env, Some(&session.id), json!({ "code": "unknown_type", "message": format!("Unknown message type '{}'.", other.chars().take(32).collect::<String>()) }));
                        if !send(&mut socket, &e).await { break; }
                    }
                }
            }
            ev = events.recv() => match ev {
                // Envelopes addressed to one attempt (removed, submitted) reach only that student's sockets.
                Ok(e) if e.session_id.as_deref() == Some(session.id.as_str()) && e.payload.get("attemptId").is_some() => {
                    let mine = joined.as_ref().is_some_and(|(id, _)| e.payload["attemptId"].as_str() == Some(id.as_str()));
                    if mine {
                        let removed = e.kind == msg::REMOVED;
                        if !send(&mut socket, &e).await { break; }
                        if removed {
                            close_with(&mut socket, CLOSE_POLICY, "removed").await;
                            break;
                        }
                    }
                }
                Ok(e) if e.session_id.as_deref() == Some(session.id.as_str()) => {
                    let ended = e.kind == msg::SESSION_ENDED;
                    if !send(&mut socket, &e).await { break; }
                    if ended { /* keep the socket: Phase 7 sends the final result over it */ }
                }
                Ok(_) => {}
                // Missed broadcasts: resync from the database instead of guessing.
                Err(RecvError::Lagged(_)) => {
                    if let Ok(s) = st.db.get_session(&session.id).await {
                        if !send(&mut socket, &Envelope::new(msg::TIMER_SYNC, Some(&session.id), timer_payload(&s))).await { break; }
                    }
                }
                Err(RecvError::Closed) => break,
            },
            _ = sync.tick() => {
                // Read the session fresh so a restart or teacher action is never contradicted.
                match st.db.get_session(&session.id).await {
                    Ok(s) => if !send(&mut socket, &Envelope::new(msg::TIMER_SYNC, Some(&session.id), timer_payload(&s))).await { break; },
                    Err(_) => break,
                }
            }
            _ = tokio::time::sleep_until(deadline) => {
                tracing::info!(session_id = %session.id, %ip, "client silent too long, dropping connection");
                close_with(&mut socket, CLOSE_POLICY, "heartbeat timeout").await;
                break;
            }
        }
    }
    tracing::info!(session_id = %session.id, %ip, "client disconnected");
}

/// Handles `join {studentName, studentId, token?}`. Never reveals questions or answers.
async fn handle_join(st: &AppState, session: &ExamSession, env: &Envelope, joined: &mut Option<(String, OnlineGuard)>) -> Envelope {
    let err = |e: JoinError| Envelope::reply(msg::ERROR, env, Some(&session.id), json!({ "code": e.code(), "message": e.message() }));
    if joined.is_some() {
        return err(JoinError::Invalid("This connection has already joined.".into()));
    }
    let text = |k: &str| env.payload.get(k).and_then(|v| v.as_str()).unwrap_or("").to_string();
    let token = env.payload.get("token").and_then(|v| v.as_str()).filter(|t| !t.is_empty() && t.len() <= 128);

    // Re-read the session: it may have ended since this socket said hello.
    let current = match st.db.get_session(&session.id).await {
        Ok(s) => s,
        Err(_) => return err(JoinError::Internal),
    };
    let outcome = match join_service::join(&st.db, &current, &text("studentName"), &text("studentId"), token).await {
        Ok(o) => o,
        Err(e) => return err(e),
    };
    let exam = match st.db.get_exam_full(&current.exam_id).await {
        Ok(e) => e,
        Err(_) => return err(JoinError::Internal),
    };
    *joined = Some((outcome.attempt.id.clone(), st.hub.mark_online(&outcome.attempt.id, &session.id)));

    let mut payload = timer_payload(&current);
    if let Some(o) = payload.as_object_mut() {
        o.insert("attemptId".into(), json!(outcome.attempt.id));
        o.insert("resumed".into(), json!(outcome.new_token.is_none()));
        o.insert("token".into(), json!(outcome.new_token));
        o.insert("studentName".into(), json!(outcome.student.name));
        o.insert("studentId".into(), json!(outcome.student.student_number));
        o.insert("attemptStatus".into(), json!(outcome.attempt.status));
        o.insert("exam".into(), json!({
            "title": exam.exam.title,
            "description": exam.exam.description,
            "instructions": exam.exam.instructions,
            "durationMinutes": exam.exam.duration_minutes,
            "questionCount": exam.questions.len(),
        }));
    }
    tracing::info!(session_id = %session.id, attempt_id = %outcome.attempt.id, resumed = outcome.new_token.is_none(), "student joined");
    Envelope::reply(msg::JOINED, env, Some(&session.id), payload)
}

/// start_exam / answer / submit. The server re-reads the session every time: clients are never trusted
/// about status or time.
async fn handle_exam_message(st: &AppState, session_id: &str, attempt_id: &str, env: &Envelope) -> Envelope {
    let fail = |e: EngineError| Envelope::reply(msg::ERROR, env, Some(session_id), json!({ "code": e.code(), "message": e.message() }));
    let session = match st.db.get_session(session_id).await {
        Ok(s) => s,
        Err(_) => return fail(EngineError::Internal),
    };
    match env.kind.as_str() {
        client::START_EXAM => match exam_engine::start(&st.db, &session, attempt_id).await {
            Ok((paper, _)) => {
                let mut payload = timer_payload(&session);
                if let Some(o) = payload.as_object_mut() {
                    o.insert("paper".into(), serde_json::to_value(&paper).unwrap_or_default());
                }
                Envelope::reply(msg::EXAM_PAPER, env, Some(session_id), payload)
            }
            Err(e) => fail(e),
        },
        client::ANSWER => {
            let qid = env.payload.get("questionId").and_then(|v| v.as_str()).unwrap_or("");
            let seq = env.payload.get("clientSeq").and_then(|v| v.as_i64()).unwrap_or(-1);
            let answer = env.payload.get("answer").cloned().unwrap_or(serde_json::Value::Null);
            match exam_engine::save_answer(&st.db, &session, attempt_id, qid, &answer, seq).await {
                Ok(stored) => Envelope::reply(msg::ANSWER_ACK, env, Some(session_id), json!({ "questionId": qid, "clientSeq": seq, "stored": stored })),
                Err(e) => {
                    // Echo the question so a client retry queue can drop or keep the right entry.
                    let mut r = fail(e);
                    if let Some(o) = r.payload.as_object_mut() {
                        o.insert("questionId".into(), json!(qid));
                        o.insert("clientSeq".into(), json!(seq));
                    }
                    r
                }
            }
        }
        _ => match exam_engine::submit(&st.db, &session, attempt_id).await {
            Ok(result) => Envelope::reply(msg::SUBMITTED, env, Some(session_id), json!({ "attemptId": attempt_id, "auto": false, "result": result })),
            Err(e) => fail(e),
        },
    }
}
