//! A full classroom: 100 students (the supported maximum) join, answer and submit at the same time.

mod common;

use std::time::Instant;

use common::*;
use futures_util::future::join_all;
use proctorlan_lib::services::results_service::ResultsService;
use proctorlan_lib::services::session_service::SessionAction;
use serde_json::json;

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn one_hundred_students_take_an_exam_at_once() {
    let h = harness().await;
    let s = h.open_session_for(four_type_exam()).await;
    let (sid, code) = (s.row.session.id.clone(), s.row.session.session_code.clone());
    let t0 = Instant::now();

    // 100 students join together.
    let joined = join_all((0..100).map(|i| {
        let (h, code) = (&h, code.clone());
        async move { h.student(&code, &format!("Student {i:03}"), &format!("S-{i:03}")).await }
    })).await;
    let join_time = t0.elapsed();
    assert_eq!(h.sessions.snapshot(&sid).await.unwrap().online, 100);

    // The 101st is refused politely.
    let mut extra = h.ws().await;
    let r = extra.hello(&code).await.unwrap();
    assert_eq!(r["payload"]["code"], "session_full", "{r}");
    assert!(extra.recv().await.is_none(), "and the socket is closed");

    h.sessions.act(&h.user, &sid, SessionAction::Start).await.unwrap();

    // Everyone opens the exam, answers every question (even-numbered students answer correctly), and submits.
    let t1 = Instant::now();
    let results = join_all(joined.into_iter().enumerate().map(|(i, (mut ws, _))| async move {
        ws.recv_kind("session_started").await;
        let paper = ws.start_exam().await["payload"]["paper"].clone();
        let right = i % 2 == 0;
        let pick = |q: &str, good: &str, bad: &str| json!(cid(&paper, q, if right { good } else { bad }));
        let answers = [
            (qid(&paper, "Capital"), pick("Capital", "Paris", "Rome")),
            (qid(&paper, "CSS"), pick("CSS", "True", "False")),
            (qid(&paper, "languages"), json!([cid(&paper, "languages", "Rust"), cid(&paper, "languages", if right { "Python" } else { "HTML" })])),
            (qid(&paper, "Hamlet"), json!(if right { "shakespeare" } else { "marlowe" })),
        ];
        for (n, (q, a)) in answers.into_iter().enumerate() {
            assert_eq!(ws.answer(&q, a, n as i64 + 1).await["type"], "answer_ack");
        }
        ws.submit().await
    })).await;
    let exam_time = t1.elapsed();

    assert!(results.iter().all(|r| r["type"] == "submitted"), "some submissions failed");
    let svc = ResultsService::new(h.db.clone(), std::env::temp_dir().join("proctorlan-load"));
    h.sessions.act(&h.user, &sid, SessionAction::End).await.unwrap();
    let res = svc.session_results(&sid).await.unwrap();
    assert_eq!(res.rows.len(), 100);
    assert_eq!(res.stats.submitted, 100);
    assert_eq!(res.stats.passed, 50, "the 50 even-numbered students answered correctly");
    assert_eq!((res.stats.highest, res.stats.lowest), (100.0, 0.0));
    let answers: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM answers").fetch_one(h.db.pool()).await.unwrap();
    assert_eq!(answers, 400, "no answer lost, none duplicated");

    println!("LOAD: join {:?}, exam (open+4 answers+submit) {:?}", join_time, exam_time);
    assert!(join_time.as_secs() < 10, "joining 100 students took {join_time:?}");
    assert!(exam_time.as_secs() < 20, "the exam round took {exam_time:?}");
}
