//! Proctoring events over real sockets: what is recorded, what is ignored, what clients may not forge.

mod common;

use std::time::Duration;

use common::*;
use proctorlan_lib::models::*;
use proctorlan_lib::services::exam_engine;
use proctorlan_lib::services::session_service::SessionAction;
use serde_json::{json, Value};

/// Session running; student joined and opened the exam.
async fn in_exam(h: &Harness) -> (String, String, Ws, Value, String) {
    let s = h.open_session_for(four_type_exam()).await;
    let (sid, code) = (s.row.session.id.clone(), s.row.session.session_code.clone());
    let mut ws = h.ws().await;
    ws.hello(&code).await.unwrap();
    let joined = ws.join("Ana Reyes", "A1", None).await;
    let token = joined["payload"]["token"].as_str().unwrap().to_string();
    h.sessions.act(&h.user, &sid, SessionAction::Start).await.unwrap();
    ws.recv_kind("session_started").await;
    let paper = ws.start_exam().await["payload"]["paper"].clone();
    (sid, code, ws, paper, token)
}

async fn events(h: &Harness, sid: &str) -> Vec<SessionEvent> {
    let mut v = h.db.list_session_events(sid, 500).await.unwrap();
    v.reverse(); // oldest first, easier to read
    v
}

fn kinds(evs: &[SessionEvent]) -> Vec<ProctorEventType> { evs.iter().map(|e| e.event_type).collect() }

async fn wait_for<F: Fn(&[SessionEvent]) -> bool>(h: &Harness, sid: &str, cond: F) -> Vec<SessionEvent> {
    for _ in 0..80 {
        let e = events(h, sid).await;
        if cond(&e) { return e; }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    panic!("condition never met; events: {:?}", kinds(&events(h, sid).await));
}

#[tokio::test]
async fn leaving_and_returning_are_recorded_with_how_long_the_student_was_away() {
    let h = harness().await;
    let (sid, _, mut ws, _, _) = in_exam(&h).await;
    assert_eq!(ws.focus_lost().await["payload"]["recorded"], true);
    assert_eq!(ws.focus_restored(12_400).await["payload"]["recorded"], true);
    let evs = events(&h, &sid).await;
    assert_eq!(kinds(&evs), [ProctorEventType::FocusLost, ProctorEventType::FocusRestored]);
    assert_eq!(evs[0].student_name, "Ana Reyes");
    assert!(evs[1].description.contains("12 s"), "{}", evs[1].description);
    assert_eq!(serde_json::from_str::<Value>(evs[1].metadata.as_deref().unwrap()).unwrap()["lostForMs"], 12_400);

    ws.focus_lost().await;
    ws.focus_restored(60_000).await;
    let row = &h.sessions.roster(&sid).await.unwrap()[0];
    assert_eq!((row.focus_lost_count, row.focus_lost_ms, row.disconnect_count), (2, 72_400, 0));
}

#[tokio::test]
async fn duplicates_and_out_of_order_reports_are_ignored() {
    let h = harness().await;
    let (sid, _, mut ws, _, _) = in_exam(&h).await;
    assert_eq!(ws.focus_restored(500).await["payload"]["recorded"], false, "restored without lost");
    assert_eq!(ws.focus_lost().await["payload"]["recorded"], true);
    assert_eq!(ws.focus_lost().await["payload"]["recorded"], false, "already away");
    assert_eq!(ws.focus_restored(1000).await["payload"]["recorded"], true);
    assert_eq!(ws.focus_restored(1000).await["payload"]["recorded"], false, "already back");
    assert_eq!(events(&h, &sid).await.len(), 2);
}

#[tokio::test]
async fn clients_cannot_forge_server_side_event_types_or_garbage() {
    let h = harness().await;
    let (sid, _, mut ws, _, _) = in_exam(&h).await;
    for kind in ["DISCONNECTED", "RECONNECTED", "SUBMISSION", "TIMEOUT", "focus_lost", "", "'; DROP TABLE proctor_events;--"] {
        let r = ws.request("proctor_event", json!({ "type": kind })).await;
        assert_eq!(r["payload"]["code"], "invalid_event", "{kind}");
    }
    assert_eq!(ws.request("proctor_event", json!({})).await["payload"]["code"], "invalid_event");
    assert_eq!(ws.request("proctor_event", json!({ "type": 5 })).await["payload"]["code"], "invalid_event");
    assert!(events(&h, &sid).await.is_empty());
}

#[tokio::test]
async fn nothing_is_recorded_unless_the_student_is_actively_taking_a_running_exam() {
    let h = harness().await;
    let s = h.open_session_for(four_type_exam()).await;
    let (sid, code) = (s.row.session.id.clone(), s.row.session.session_code.clone());
    let (mut ws, _) = h.student(&code, "Ana", "A1").await;
    // Waiting room: not recorded.
    assert_eq!(ws.focus_lost().await["payload"]["recorded"], false);
    h.sessions.act(&h.user, &sid, SessionAction::Start).await.unwrap();
    ws.recv_kind("session_started").await;
    // Joined but has not opened the exam: not recorded.
    assert_eq!(ws.focus_lost().await["payload"]["recorded"], false);
    ws.start_exam().await;
    assert_eq!(ws.focus_lost().await["payload"]["recorded"], true);
    ws.focus_restored(100).await;
    ws.submit().await;
    // After submitting students may do what they like.
    assert_eq!(ws.focus_lost().await["payload"]["recorded"], false);
    assert_eq!(kinds(&events(&h, &sid).await), [ProctorEventType::FocusLost, ProctorEventType::FocusRestored, ProctorEventType::Submission]);
}

#[tokio::test]
async fn while_paused_leaving_is_not_recorded_but_coming_back_closes_an_open_interval() {
    let h = harness().await;
    let (sid, _, mut ws, _, _) = in_exam(&h).await;
    ws.focus_lost().await; // away just before the pause
    h.sessions.act(&h.user, &sid, SessionAction::Pause).await.unwrap();
    ws.recv_kind("session_paused").await;
    assert_eq!(ws.focus_restored(5000).await["payload"]["recorded"], true, "interval closed during the pause");
    assert_eq!(ws.focus_lost().await["payload"]["recorded"], false, "clock stopped: not interesting");
    h.sessions.act(&h.user, &sid, SessionAction::Resume).await.unwrap();
    ws.recv_kind("session_resumed").await;
    assert_eq!(ws.focus_lost().await["payload"]["recorded"], true, "fresh interval after resume");
}

#[tokio::test]
async fn absurd_durations_are_clamped() {
    let h = harness().await;
    let (sid, _, mut ws, _, _) = in_exam(&h).await;
    ws.focus_lost().await;
    ws.focus_restored(i64::MAX).await;
    ws.focus_lost().await;
    ws.focus_restored(-50).await;
    let evs = events(&h, &sid).await;
    let ms = |e: &SessionEvent| serde_json::from_str::<Value>(e.metadata.as_deref().unwrap()).unwrap()["lostForMs"].as_i64().unwrap();
    assert_eq!(ms(&evs[1]), 24 * 60 * 60 * 1000);
    assert_eq!(ms(&evs[3]), 0);
}

#[tokio::test]
async fn a_long_outage_records_disconnected_then_reconnected_with_the_duration() {
    let h = harness_with_grace(Duration::from_millis(300)).await;
    let (sid, code, ws, _, token) = in_exam(&h).await;
    drop(ws);
    let evs = wait_for(&h, &sid, |e| kinds(e) == [ProctorEventType::Disconnected]).await;
    assert!(evs[0].description.contains("Lost connection"));

    tokio::time::sleep(Duration::from_millis(250)).await;
    let mut back = h.ws().await;
    back.hello(&code).await.unwrap();
    assert_eq!(back.join("Ana Reyes", "A1", Some(&token)).await["payload"]["resumed"], true);
    let evs = events(&h, &sid).await;
    assert_eq!(kinds(&evs), [ProctorEventType::Disconnected, ProctorEventType::Reconnected]);
    assert!(serde_json::from_str::<Value>(evs[1].metadata.as_deref().unwrap()).unwrap()["offlineMs"].as_i64().unwrap() >= 250);
    assert_eq!(h.sessions.roster(&sid).await.unwrap()[0].disconnect_count, 1);
}

#[tokio::test]
async fn a_quick_reconnect_leaves_no_disconnect_trace() {
    let h = harness_with_grace(Duration::from_millis(600)).await;
    let (sid, code, ws, _, token) = in_exam(&h).await;
    drop(ws);
    let mut back = h.ws().await;
    back.hello(&code).await.unwrap();
    back.join("Ana Reyes", "A1", Some(&token)).await;
    tokio::time::sleep(Duration::from_millis(900)).await; // well past the grace period
    assert!(events(&h, &sid).await.is_empty(), "a blip is not a disconnect");
}

#[tokio::test]
async fn a_takeover_by_a_new_window_is_not_a_disconnect() {
    let h = harness_with_grace(Duration::from_millis(200)).await;
    let (sid, code, mut old, _, token) = in_exam(&h).await;
    let mut fresh = h.ws().await;
    fresh.hello(&code).await.unwrap();
    fresh.join("Ana Reyes", "A1", Some(&token)).await;
    old.recv_kind("replaced").await;
    tokio::time::sleep(Duration::from_millis(600)).await;
    assert!(events(&h, &sid).await.is_empty());
}

#[tokio::test]
async fn disconnects_are_not_recorded_for_students_who_are_not_mid_exam() {
    let h = harness_with_grace(Duration::from_millis(150)).await;
    let s = h.open_session_for(four_type_exam()).await;
    let (sid, code) = (s.row.session.id.clone(), s.row.session.session_code.clone());
    // 1) Sitting in the waiting room.
    let (waiting, _) = h.student(&code, "Waiting Wendy", "W1").await;
    drop(waiting);
    // 2) Submitted already.
    h.sessions.act(&h.user, &sid, SessionAction::Start).await.unwrap();
    let (mut done, _) = h.student(&code, "Done Dan", "D1").await;
    done.start_exam().await;
    done.submit().await;
    drop(done);
    tokio::time::sleep(Duration::from_millis(500)).await;
    let evs = events(&h, &sid).await;
    assert_eq!(kinds(&evs), [ProctorEventType::Submission], "only the submission, no disconnect noise");
}

#[tokio::test]
async fn submission_timeout_and_session_end_are_recorded_by_the_server() {
    let h = harness().await;
    // Student A submits; student B is auto-submitted on time-out; student C when the teacher ends.
    let s = h.open_session_for(four_type_exam()).await;
    let (sid, code) = (s.row.session.id.clone(), s.row.session.session_code.clone());
    let (mut a, _) = h.student(&code, "A", "A1").await;
    let (mut b, _) = h.student(&code, "B", "B1").await;
    h.sessions.act(&h.user, &sid, SessionAction::Start).await.unwrap();
    a.start_exam().await;
    b.start_exam().await;
    a.submit().await;
    h.set_ends_at(&sid, "2020-01-01T00:00:00.000Z").await;
    exam_engine::expire_due(&h.db, &h.hub, &mut Default::default()).await.unwrap();
    let evs = events(&h, &sid).await;
    assert!(evs.iter().any(|e| e.student_number == "A1" && e.event_type == ProctorEventType::Submission));
    assert!(evs.iter().any(|e| e.student_number == "B1" && e.event_type == ProctorEventType::Timeout));

    let s2 = h.open_session_for(four_type_exam()).await;
    let (sid2, code2) = (s2.row.session.id.clone(), s2.row.session.session_code.clone());
    let (mut c, _) = h.student(&code2, "C", "C1").await;
    h.sessions.act(&h.user, &sid2, SessionAction::Start).await.unwrap();
    c.start_exam().await;
    h.sessions.act(&h.user, &sid2, SessionAction::End).await.unwrap();
    let evs = events(&h, &sid2).await;
    assert_eq!(kinds(&evs), [ProctorEventType::Submission]);
    assert!(evs[0].description.contains("teacher ended"));
}

#[tokio::test]
async fn events_never_change_the_grade_and_are_capped_per_attempt() {
    let h = harness().await;
    let (sid, _, mut ws, paper, _) = in_exam(&h).await;
    ws.answer(&qid(&paper, "CSS"), json!(cid(&paper, "CSS", "True")), 1).await;
    for _ in 0..5 {
        ws.focus_lost().await;
        ws.focus_restored(2000).await;
    }
    ws.submit().await;
    let row = &h.sessions.roster(&sid).await.unwrap()[0];
    assert_eq!(row.percentage, Some(20.0), "proctoring is a signal, never a penalty");
    assert_eq!(row.focus_lost_count, 5);

    // Cap: fill the attempt to the limit directly, then a client event is ignored.
    let h2 = harness().await;
    let (sid2, _, mut ws2, _, _) = in_exam(&h2).await;
    let attempt = h2.db.list_roster(&sid2).await.unwrap()[0].attempt_id.clone();
    for _ in 0..1000 {
        h2.db.record_proctor_event(&attempt, ProctorEventType::FocusLost, "x", None).await.unwrap();
    }
    h2.db.record_proctor_event(&attempt, ProctorEventType::FocusRestored, "x", Some("{\"lostForMs\":1}")).await.unwrap_err_or_ok();
    assert_eq!(ws2.focus_lost().await["payload"]["recorded"], false);
}

trait IgnoreResult { fn unwrap_err_or_ok(self); }
impl<T, E> IgnoreResult for Result<T, E> { fn unwrap_err_or_ok(self) {} }

#[tokio::test]
async fn the_feed_is_per_session_newest_first_and_names_the_student() {
    let h = harness().await;
    let (sid, code, mut ana, _, _) = in_exam(&h).await;
    let (mut ben, _) = h.student(&code, "Ben Cruz", "B2").await;
    ben.start_exam().await;
    ana.focus_lost().await;
    tokio::time::sleep(Duration::from_millis(20)).await;
    ben.focus_lost().await;
    let feed = h.sessions.events(&sid).await.unwrap();
    assert_eq!(feed.len(), 2);
    assert_eq!((feed[0].student_name.as_str(), feed[1].student_name.as_str()), ("Ben Cruz", "Ana Reyes"), "newest first");
    // A different session's events are not mixed in.
    let other = h.open_session_for(four_type_exam()).await;
    assert!(h.sessions.events(&other.row.session.id).await.unwrap().is_empty());
}
