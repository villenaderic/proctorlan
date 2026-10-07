//! Results, analytics and CSV export over real graded attempts.

mod common;

use common::*;
use proctorlan_lib::models::*;
use proctorlan_lib::services::results_service::ResultsService;
use proctorlan_lib::services::session_service::SessionAction;
use serde_json::json;

struct Scenario {
    _h: Harness,
    svc: ResultsService,
    sid: String,
    dir: std::path::PathBuf,
}

/// Three students: Ana gets everything right, Ben gets only Q1, Cy (a risky name) gets nothing right.
async fn scenario() -> Scenario {
    let h = harness().await;
    let s = h.open_session_for(four_type_exam()).await;
    let (sid, code) = (s.row.session.id.clone(), s.row.session.session_code.clone());
    let (mut ana, _) = h.student(&code, "Ana Reyes", "A1").await;
    let (mut ben, _) = h.student(&code, "Ben, Jr.", "B2").await;
    let (mut cy, _) = h.student(&code, "=HYPERLINK(\"http://evil\")", "C3").await;
    h.sessions.act(&h.user, &sid, SessionAction::Start).await.unwrap();
    for w in [&mut ana, &mut ben, &mut cy] { w.recv_kind("session_started").await; }

    let pa = ana.start_exam().await["payload"]["paper"].clone();
    let pb = ben.start_exam().await["payload"]["paper"].clone();
    let pc = cy.start_exam().await["payload"]["paper"].clone();

    ana.answer(&qid(&pa, "Capital"), json!(cid(&pa, "Capital", "Paris")), 1).await;
    ana.answer(&qid(&pa, "CSS"), json!(cid(&pa, "CSS", "True")), 2).await;
    ana.answer(&qid(&pa, "languages"), json!([cid(&pa, "languages", "Rust"), cid(&pa, "languages", "Python")]), 3).await;
    ana.answer(&qid(&pa, "Hamlet"), json!("shakespeare"), 4).await;
    assert_eq!(ana.submit().await["type"], "submitted");

    ben.answer(&qid(&pb, "Capital"), json!(cid(&pb, "Capital", "Paris")), 1).await;
    ben.answer(&qid(&pb, "Hamlet"), json!("Marlowe"), 2).await;
    assert_eq!(ben.submit().await["type"], "submitted");

    cy.answer(&qid(&pc, "Capital"), json!(cid(&pc, "Capital", "Rome")), 1).await;
    cy.answer(&qid(&pc, "Hamlet"), json!("  marlowe "), 2).await;
    cy.answer(&qid(&pc, "languages"), json!([cid(&pc, "languages", "HTML")]), 3).await;
    assert_eq!(cy.submit().await["type"], "submitted");

    h.sessions.act(&h.user, &sid, SessionAction::End).await.unwrap();
    let dir = std::env::temp_dir().join(format!("proctorlan-results-{}", std::process::id() as u64 * 1000 + rand_suffix()));
    let svc = ResultsService::new(h.db.clone(), dir.clone());
    Scenario { _h: h, svc, sid, dir }
}

fn rand_suffix() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().subsec_nanos() as u64 % 1000
}

#[tokio::test]
async fn session_results_have_scores_stats_and_distribution() {
    let s = scenario().await;
    let r = s.svc.session_results(&s.sid).await.unwrap();
    assert_eq!((r.question_count, r.total_points, r.passing_score), (4, 5.0, 60.0));
    assert_eq!(r.rows.len(), 3);
    let by = |n: &str| r.rows.iter().find(|x| x.name.starts_with(n)).unwrap();
    assert_eq!((by("Ana").score, by("Ana").percentage, by("Ana").passed), (Some(5.0), Some(100.0), Some(true)));
    assert_eq!((by("Ben").score, by("Ben").percentage, by("Ben").passed), (Some(1.0), Some(20.0), Some(false)));
    assert_eq!(by("=HYPERLINK").score, Some(0.0));
    assert_eq!(r.stats.submitted, 3);
    assert_eq!((r.stats.passed, r.stats.failed), (1, 2));
    assert_eq!(r.stats.pass_rate, 33.33);
    assert_eq!((r.stats.highest, r.stats.lowest, r.stats.median), (100.0, 0.0, 20.0));
    assert_eq!(r.distribution.iter().map(|b| b.count).sum::<i64>(), 3);
    assert_eq!(r.distribution[9].count, 1);
    assert_eq!(r.distribution[2].count, 1);
    assert_eq!(r.distribution[0].count, 1);
    assert!(by("Ana").time_taken_seconds.is_some());
}

#[tokio::test]
async fn question_analytics_count_correct_answers_picks_and_common_mistakes() {
    let s = scenario().await;
    let r = s.svc.session_results(&s.sid).await.unwrap();
    let q = |t: &str| r.questions.iter().find(|q| q.text.contains(t)).unwrap();
    let capital = q("Capital");
    assert_eq!((capital.attempts, capital.answered, capital.correct, capital.percent_correct), (3, 3, 2, 66.67));
    let picked = |name: &str| capital.options.iter().find(|o| o.text == name).unwrap().picked;
    assert_eq!((picked("Paris"), picked("Rome"), picked("Oslo")), (2, 1, 0));
    assert!(capital.options.iter().find(|o| o.text == "Paris").unwrap().is_correct);
    let css = q("CSS");
    assert_eq!((css.answered, css.correct), (1, 1)); // two students never answered it
    let hamlet = q("Hamlet");
    assert_eq!(hamlet.correct, 1);
    assert_eq!(hamlet.common_wrong.len(), 1);
    assert_eq!(hamlet.common_wrong[0].count, 2); // "Marlowe" and "  marlowe " are the same mistake
    let langs = q("languages");
    assert_eq!(langs.correct, 1);
    assert_eq!(langs.options.iter().find(|o| o.text == "Rust").unwrap().picked, 1);
}

#[tokio::test]
async fn attempt_detail_shows_given_and_correct_answers_and_events() {
    let s = scenario().await;
    let r = s.svc.session_results(&s.sid).await.unwrap();
    let ben = r.rows.iter().find(|x| x.name.starts_with("Ben")).unwrap();
    let d = s.svc.attempt_detail(&ben.attempt_id).await.unwrap();
    assert_eq!((d.student_number.as_str(), d.percentage, d.passing_score), ("B2", Some(20.0), 60.0));
    assert_eq!(d.answers.len(), 4);
    let capital = &d.answers[0];
    assert_eq!((capital.given.clone(), capital.is_correct, capital.points_awarded), (vec!["Paris".to_string()], true, 1.0));
    let hamlet = d.answers.iter().find(|a| a.text.contains("Hamlet")).unwrap();
    assert_eq!(hamlet.given, vec!["Marlowe".to_string()]);
    assert!(!hamlet.is_correct && hamlet.correct.contains(&"Shakespeare".to_string()));
    let css = d.answers.iter().find(|a| a.text.contains("CSS")).unwrap();
    assert!(!css.answered && css.given.is_empty() && css.points_awarded == 0.0);
    assert_eq!(css.explanation.as_deref(), Some("SECRET EXPLANATION")); // teachers see explanations
    assert!(d.events.iter().any(|e| e.event_type == ProctorEventType::Submission));
    assert!(s.svc.attempt_detail("nope").await.is_err());
}

#[tokio::test]
async fn csv_is_excel_friendly_and_safe_against_formula_injection() {
    let s = scenario().await;
    let (text, _) = s.svc.csv(&s.sid).await.unwrap();
    assert!(text.starts_with('\u{FEFF}'));
    let lines: Vec<&str> = text.trim_end().split("\r\n").collect();
    assert_eq!(lines.len(), 4); // header + 3 students
    assert!(lines[0].contains("Student ID,Name,Status,Score,Total points,Percentage,Result"));
    assert!(lines[0].ends_with("Q1 (1 pt),Q2 (1 pt),Q3 (2 pt),Q4 (1 pt)"));
    let ana = lines.iter().find(|l| l.contains("Ana Reyes")).unwrap();
    assert!(ana.contains(",Submitted,5,5,100,Passed,"), "{ana}");
    assert!(ana.ends_with(",1,1,2,1"), "{ana}");
    assert!(lines.iter().any(|l| l.contains("\"Ben, Jr.\"")));
    assert!(!text.contains(",=HYPERLINK"), "formula must be neutralised");
    assert!(text.contains("'=HYPERLINK"));
}

#[tokio::test]
async fn export_writes_a_file_with_a_safe_name() {
    let s = scenario().await;
    let info = s.svc.export_csv(&s.sid).await.unwrap();
    assert_eq!(info.rows, 3);
    assert!(info.file_name.starts_with("engine-exam-") && info.file_name.ends_with(".csv"), "{}", info.file_name);
    assert!(!info.file_name.contains(['/', '\\', ' ']));
    let written = std::fs::read_to_string(&info.path).unwrap();
    assert!(written.contains("Ana Reyes"));
    assert!(info.path.starts_with(s.dir.to_str().unwrap()));
    let _ = std::fs::remove_dir_all(&s.dir);
}

#[tokio::test]
async fn results_listing_and_student_history() {
    let s = scenario().await;
    let list = s.svc.sessions().await.unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!((list[0].joined, list[0].submitted, list[0].passed), (3, 3, 1));
    assert!((list[0].average_percentage.unwrap() - 40.0).abs() < 0.01);
    assert_eq!(list[0].exam_title, "Engine Exam");

    let students = s.svc.students().await.unwrap();
    assert_eq!(students.len(), 3);
    let ana = students.iter().find(|x| x.student_number == "A1").unwrap();
    assert_eq!((ana.attempts, ana.average_percentage), (1, Some(100.0)));
    let hist = s.svc.student_history(&ana.id).await.unwrap();
    assert_eq!(hist.len(), 1);
    assert_eq!((hist[0].percentage, hist[0].passed), (Some(100.0), Some(true)));
    assert!(s.svc.student_history("missing").await.is_err());
}

#[tokio::test]
async fn unfinished_attempts_are_reported_but_not_scored() {
    let h = harness().await;
    let s = h.open_session_for(four_type_exam()).await;
    let (sid, code) = (s.row.session.id.clone(), s.row.session.session_code.clone());
    let (_ws, _) = h.student(&code, "Dee", "D4").await; // joined, never starts
    let svc = ResultsService::new(h.db.clone(), std::env::temp_dir().join("proctorlan-unused"));
    let r = svc.session_results(&sid).await.unwrap();
    assert_eq!((r.stats.submitted, r.stats.not_submitted), (0, 1));
    assert_eq!(r.rows[0].percentage, None);
    assert_eq!(r.questions[0].attempts, 0);
    assert_eq!(r.questions[0].percent_correct, 0.0);
    let (csv, _) = svc.csv(&sid).await.unwrap();
    assert!(csv.contains("Dee,Joined,,,,,"), "{csv}");
}
