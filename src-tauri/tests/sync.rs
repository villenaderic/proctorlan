//! Synchronisation: batch answer upload after offline periods, takeover of stale connections,
//! reconnect storms and flood protection. Real sockets, no mocks.

mod common;

use std::time::Duration;

use common::*;
use proctorlan_lib::models::*;
use proctorlan_lib::services::session_service::SessionAction;
use serde_json::{json, Value};

async fn running() -> (Harness, String, String, Ws, Value, String) {
    let h = harness().await;
    let s = h.open_session_for(four_type_exam()).await;
    let (sid, code) = (s.row.session.id.clone(), s.row.session.session_code.clone());
    let mut ws = h.ws().await;
    ws.hello(&code).await.unwrap();
    let joined = ws.join("Ana Reyes", "A1", None).await;
    let token = joined["payload"]["token"].as_str().unwrap().to_string();
    h.sessions.act(&h.user, &sid, SessionAction::Start).await.unwrap();
    ws.recv_kind("session_started").await;
    let paper = ws.start_exam().await["payload"]["paper"].clone();
    (h, sid, code, ws, paper, token)
}

fn item(q: &str, a: Value, seq: i64) -> Value { json!({ "questionId": q, "answer": a, "clientSeq": seq }) }

async fn sync(ws: &mut Ws, items: Vec<Value>) -> Value { ws.request("answers_sync", json!({ "answers": items })).await }

fn statuses(r: &Value) -> Vec<String> {
    r["payload"]["results"].as_array().unwrap().iter().map(|x| x["status"].as_str().unwrap().to_string()).collect()
}

#[tokio::test]
async fn a_batch_stores_every_answer_with_a_verdict_per_item() {
    let (h, _, _, mut ws, paper, _) = running().await;
    let r = sync(&mut ws, vec![
        item(&qid(&paper, "Capital"), json!(cid(&paper, "Capital", "Paris")), 1),
        item(&qid(&paper, "CSS"), json!(cid(&paper, "CSS", "True")), 2),
        item(&qid(&paper, "Hamlet"), json!("Shakespeare"), 3),
    ]).await;
    assert_eq!(r["type"], "answers_synced", "{r}");
    assert_eq!(statuses(&r), ["stored", "stored", "stored"]);
    let results = r["payload"]["results"].as_array().unwrap();
    assert_eq!(results[1]["questionId"], qid(&paper, "CSS"));
    assert_eq!(results[1]["clientSeq"], 2);
    let n: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM answers").fetch_one(h.db.pool()).await.unwrap();
    assert_eq!(n, 3);
}

#[tokio::test]
async fn replaying_the_same_batch_changes_nothing() {
    let (h, _, _, mut ws, paper, _) = running().await;
    let batch = vec![
        item(&qid(&paper, "Capital"), json!(cid(&paper, "Capital", "Rome")), 10),
        item(&qid(&paper, "Hamlet"), json!("Marlowe"), 11),
    ];
    assert_eq!(statuses(&sync(&mut ws, batch.clone()).await), ["stored", "stored"]);
    assert_eq!(statuses(&sync(&mut ws, batch).await), ["duplicate", "duplicate"]);
    let n: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM answers").fetch_one(h.db.pool()).await.unwrap();
    assert_eq!(n, 2);
}

#[tokio::test]
async fn out_of_order_delivery_keeps_the_newest_answer() {
    let (h, _, _, mut ws, paper, _) = running().await;
    let q = qid(&paper, "Capital");
    let (paris, rome) = (cid(&paper, "Capital", "Paris"), cid(&paper, "Capital", "Rome"));
    // The student changed their mind (seq 7 → 9) but the messages arrive newest first.
    let r = sync(&mut ws, vec![item(&q, json!(paris), 9), item(&q, json!(rome), 7)]).await;
    assert_eq!(statuses(&r), ["stored", "duplicate"]);
    let stored: String = sqlx::query_scalar("SELECT answer_data FROM answers").fetch_one(h.db.pool()).await.unwrap();
    assert_eq!(stored, format!("\"{paris}\""));
}

#[tokio::test]
async fn one_bad_item_does_not_block_the_rest() {
    let (h, _, _, mut ws, paper, _) = running().await;
    let r = sync(&mut ws, vec![
        item(&qid(&paper, "CSS"), json!(cid(&paper, "CSS", "True")), 1),
        item(&qid(&paper, "Capital"), json!("not-a-choice"), 2),
        item("no-such-question", json!("x"), 3),
        item(&qid(&paper, "Hamlet"), json!("Shakespeare"), 4),
    ]).await;
    assert_eq!(statuses(&r), ["stored", "invalid_answer", "invalid_answer", "stored"]);
    let n: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM answers").fetch_one(h.db.pool()).await.unwrap();
    assert_eq!(n, 2);
}

#[tokio::test]
async fn a_batch_during_a_pause_is_refused_whole_and_can_be_resent_after_resume() {
    let (h, sid, _, mut ws, paper, _) = running().await;
    h.sessions.act(&h.user, &sid, SessionAction::Pause).await.unwrap();
    ws.recv_kind("session_paused").await;
    let batch = vec![item(&qid(&paper, "CSS"), json!(cid(&paper, "CSS", "True")), 1), item(&qid(&paper, "Hamlet"), json!("x"), 2)];
    assert_eq!(statuses(&sync(&mut ws, batch.clone()).await), ["exam_not_running", "exam_not_running"]);
    assert_eq!(h.db.count_answers(&h.db.list_roster(&sid).await.unwrap()[0].attempt_id).await.unwrap(), 0);
    h.sessions.act(&h.user, &sid, SessionAction::Resume).await.unwrap();
    ws.recv_kind("session_resumed").await;
    assert_eq!(statuses(&sync(&mut ws, batch).await), ["stored", "stored"]);
}

#[tokio::test]
async fn a_batch_after_submission_is_rejected_and_changes_no_grade() {
    let (h, sid, _, mut ws, paper, _) = running().await;
    sync(&mut ws, vec![item(&qid(&paper, "CSS"), json!(cid(&paper, "CSS", "True")), 1)]).await;
    ws.submit().await;
    let before = h.db.list_roster(&sid).await.unwrap()[0].percentage;
    let r = sync(&mut ws, vec![item(&qid(&paper, "Capital"), json!(cid(&paper, "Capital", "Paris")), 2)]).await;
    assert_eq!(statuses(&r), ["already_submitted"]);
    assert_eq!(h.db.list_roster(&sid).await.unwrap()[0].percentage, before);
}

#[tokio::test]
async fn malformed_and_oversized_batches_are_refused() {
    let (_, _, _, mut ws, paper, _) = running().await;
    assert_eq!(ws.request("answers_sync", json!({})).await["payload"]["code"], "invalid_answer");
    assert_eq!(ws.request("answers_sync", json!({ "answers": "nope" })).await["payload"]["code"], "invalid_answer");
    let q = qid(&paper, "CSS");
    let big: Vec<Value> = (0..201).map(|i| item(&q, json!("x"), i)).collect();
    assert_eq!(sync(&mut ws, big).await["payload"]["code"], "invalid_answer");
    // Items missing fields are rejected individually, not crashed on.
    let r = sync(&mut ws, vec![json!({}), json!({ "questionId": q }), json!(5)]).await;
    assert_eq!(statuses(&r), ["invalid_answer", "invalid_answer", "invalid_answer"]);
}

#[tokio::test]
async fn a_student_cannot_sync_without_joining() {
    let (h, _, code, _, _, _) = running().await;
    let mut anon = h.ws().await;
    anon.hello(&code).await.unwrap();
    assert_eq!(anon.request("answers_sync", json!({ "answers": [] })).await["payload"]["code"], "join_required");
}

#[tokio::test]
async fn a_new_connection_takes_over_the_attempt_and_the_old_one_is_closed() {
    let (h, sid, code, mut old, _, token) = running().await;
    let mut fresh = h.ws().await;
    fresh.hello(&code).await.unwrap();
    let j = fresh.join("Ana Reyes", "A1", Some(&token)).await;
    assert_eq!(j["payload"]["resumed"], true);
    assert_eq!(old.recv_kind("replaced").await["type"], "replaced");
    assert!(old.recv().await.is_none(), "the stale socket is closed");
    // The new connection is unaffected and the student counts as online exactly once.
    assert_eq!(fresh.request("start_exam", json!({})).await["type"], "exam_paper");
    assert_eq!(h.sessions.snapshot(&sid).await.unwrap().online, 1);
}

#[tokio::test]
async fn other_students_are_not_kicked_by_someone_elses_rejoin() {
    let (h, _, code, mut ana, _, token) = running().await;
    let (mut ben, _) = h.student(&code, "Ben", "B2").await;
    let mut ana2 = h.ws().await;
    ana2.hello(&code).await.unwrap();
    ana2.join("Ana Reyes", "A1", Some(&token)).await;
    ana.recv_kind("replaced").await;
    ben.send(json!({ "id": "r1", "type": "heartbeat" })).await;
    assert_eq!(ben.recv_reply().await["type"], "heartbeat_ack");
}

#[tokio::test]
async fn a_reconnect_storm_leaves_one_attempt_one_connection_and_all_answers() {
    let (h, sid, code, mut ws, paper, token) = running().await;
    let q = qid(&paper, "CSS");
    sync(&mut ws, vec![item(&q, json!(cid(&paper, "CSS", "True")), 1)]).await;
    let mut last = ws;
    for round in 0..15 {
        let mut next = h.ws().await;
        next.hello(&code).await.unwrap();
        let j = next.join("Ana Reyes", "A1", Some(&token)).await;
        assert_eq!(j["payload"]["resumed"], true, "round {round}");
        assert_eq!(next.start_exam().await["payload"]["paper"]["answers"][&q], json!(cid(&paper, "CSS", "True")));
        last = next; // the previous socket is dropped without a clean close
    }
    for _ in 0..50 {
        if h.sessions.snapshot(&sid).await.unwrap().online == 1 { break; }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    assert_eq!(h.sessions.snapshot(&sid).await.unwrap().online, 1);
    assert_eq!(h.db.count_attempts_in_session(&sid).await.unwrap(), 1);
    drop(last);
}

#[tokio::test]
async fn bursts_over_the_soft_limit_are_throttled_but_the_connection_survives() {
    let (_, _, _, mut ws, _, _) = running().await;
    for i in 0..70 {
        ws.send(json!({ "id": format!("b{i}"), "type": "heartbeat" })).await;
    }
    let (mut acks, mut limited) = (0, 0);
    while acks + limited < 70 {
        let v = ws.recv().await.expect("connection must survive a burst");
        match (v["type"].as_str().unwrap(), v["payload"]["code"].as_str()) {
            ("heartbeat_ack", _) => acks += 1,
            ("error", Some("rate_limited")) => limited += 1,
            _ => {}
        }
    }
    assert!(limited >= 10, "expected throttling, got {limited}");
    assert!(acks >= 30);
    tokio::time::sleep(Duration::from_millis(1100)).await;
    ws.send(json!({ "id": "r1", "type": "heartbeat" })).await;
    assert_eq!(ws.recv_reply().await["type"], "heartbeat_ack", "budget refills");
}

#[tokio::test]
async fn a_flood_over_the_hard_limit_closes_the_connection() {
    let (h, sid, _, mut ws, _, _) = running().await;
    for i in 0..400 {
        if ws.0.send(tokio_tungstenite::tungstenite::Message::Text(json!({ "id": i.to_string(), "type": "heartbeat" }).to_string().into())).await.is_err() { break; }
    }
    use futures_util::SinkExt;
    let mut closed = false;
    for _ in 0..600 {
        match tokio::time::timeout(Duration::from_secs(3), futures_util::StreamExt::next(&mut ws.0)).await {
            Ok(Some(Ok(tokio_tungstenite::tungstenite::Message::Close(_)))) | Ok(None) | Ok(Some(Err(_))) => { closed = true; break; }
            Ok(Some(Ok(_))) => continue,
            Err(_) => break,
        }
    }
    assert!(closed, "flooding client should be disconnected");
    for _ in 0..50 {
        if h.sessions.snapshot(&sid).await.unwrap().online == 0 { break; }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    assert_eq!(h.sessions.snapshot(&sid).await.unwrap().online, 0);
}

#[tokio::test]
async fn answers_survive_the_students_connection_vanishing_mid_exam() {
    let (h, sid, code, mut ws, paper, token) = running().await;
    sync(&mut ws, vec![item(&qid(&paper, "Capital"), json!(cid(&paper, "Capital", "Paris")), 1)]).await;
    drop(ws); // cable pulled
    // Teacher side still sees the student, now offline, with their saved answer count.
    for _ in 0..50 {
        if !h.sessions.roster(&sid).await.unwrap()[0].online { break; }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    let row = &h.sessions.roster(&sid).await.unwrap()[0];
    assert!(!row.online);
    assert_eq!((row.answered, row.status), (1, AttemptStatus::InProgress));
    // Back later: resume, continue, submit; the early answer still counts.
    let mut back = h.ws().await;
    back.hello(&code).await.unwrap();
    back.join("Ana Reyes", "A1", Some(&token)).await;
    sync(&mut back, vec![item(&qid(&paper, "CSS"), json!(cid(&paper, "CSS", "True")), 2)]).await;
    back.submit().await;
    let a = &h.db.list_roster(&sid).await.unwrap()[0];
    assert_eq!(a.percentage, Some(40.0));
}
