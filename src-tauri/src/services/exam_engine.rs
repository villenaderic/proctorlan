//! Exam delivery, answer saving, submission and expiry. All authority lives here:
//! the server decides what a student may see, whether an answer is still accepted, and the score.

use std::collections::{BTreeMap, HashSet};

use chrono::Utc;
use serde::Serialize;
use serde_json::{json, Value};

use crate::config;
use crate::database::Database;
use crate::errors::AppError;
use crate::models::*;
use crate::server::hub::Hub;
use crate::services::grading;
use crate::services::timer;
use crate::websocket::protocol::{server as msg, Envelope};

#[derive(Debug, PartialEq, Eq)]
pub enum EngineError {
    /// Session is waiting, paused or ended: no questions or answers right now.
    NotRunning,
    /// The student has not opened the exam yet.
    NotStarted,
    AlreadySubmitted,
    TimeUp,
    Invalid(String),
    Internal,
}

impl EngineError {
    pub fn code(&self) -> &'static str {
        match self {
            EngineError::NotRunning => "exam_not_running",
            EngineError::NotStarted => "exam_not_started",
            EngineError::AlreadySubmitted => "already_submitted",
            EngineError::TimeUp => "time_up",
            EngineError::Invalid(_) => "invalid_answer",
            EngineError::Internal => "internal",
        }
    }
    pub fn message(&self) -> String {
        match self {
            EngineError::NotRunning => "The exam is not running right now.".into(),
            EngineError::NotStarted => "Open the exam before answering.".into(),
            EngineError::AlreadySubmitted => "You have already submitted this exam.".into(),
            EngineError::TimeUp => "Time is up. Answers are no longer accepted.".into(),
            EngineError::Invalid(m) => m.clone(),
            EngineError::Internal => "The server hit an error.".into(),
        }
    }
}

fn internal(e: AppError) -> EngineError {
    tracing::error!(error = %e, "exam engine error");
    EngineError::Internal
}

// ------------------------------------------------------------------ the student's paper

#[derive(Debug, Serialize)]
pub struct PaperChoice {
    pub id: String,
    pub text: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PaperQuestion {
    pub id: String,
    pub text: String,
    #[serde(rename = "type")]
    pub kind: QuestionType,
    pub points: f64,
    pub required: bool,
    /// Empty for identification: those rows ARE the accepted answers and never leave the server.
    pub choices: Vec<PaperChoice>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Paper {
    pub questions: Vec<PaperQuestion>,
    /// Answers already saved on the server (question id → answer), so a rejoin restores them.
    pub answers: BTreeMap<String, Value>,
    pub allow_review: bool,
    pub total_points: f64,
}

fn fnv1a(s: &str) -> u64 {
    s.bytes().fold(0xcbf29ce484222325, |h, b| (h ^ b as u64).wrapping_mul(0x100000001b3))
}

/// Deterministic Fisher–Yates: the same seed always gives the same order, so a student who
/// reconnects sees the same paper. Different attempts get different orders.
pub fn seeded_shuffle<T>(items: &mut [T], seed: &str) {
    let mut state = fnv1a(seed);
    let mut next = || {
        // splitmix64
        state = state.wrapping_add(0x9e3779b97f4a7c15);
        let mut z = state;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d049bb133111eb);
        z ^ (z >> 31)
    };
    for i in (1..items.len()).rev() {
        let j = (next() % (i as u64 + 1)) as usize;
        items.swap(i, j);
    }
}

pub fn build_paper(exam: &ExamFull, attempt_id: &str, saved: &[Answer]) -> Paper {
    let mut order: Vec<&QuestionFull> = exam.questions.iter().collect();
    if exam.exam.randomize_questions {
        seeded_shuffle(&mut order, &format!("{attempt_id}:questions"));
    }
    let questions = order
        .into_iter()
        .map(|q| {
            let mut choices: Vec<&Choice> = match q.question.question_type {
                QuestionType::Identification => vec![],
                _ => q.choices.iter().collect(),
            };
            let shuffle = exam.exam.randomize_choices && matches!(q.question.question_type, QuestionType::MultipleChoice | QuestionType::MultipleSelect);
            if shuffle {
                seeded_shuffle(&mut choices, &format!("{attempt_id}:{}", q.question.id));
            }
            PaperQuestion {
                id: q.question.id.clone(),
                text: q.question.question_text.clone(),
                kind: q.question.question_type,
                points: q.question.points,
                required: q.question.required,
                choices: choices.into_iter().map(|c| PaperChoice { id: c.id.clone(), text: c.choice_text.clone() }).collect(),
            }
        })
        .collect();
    let answers = saved
        .iter()
        .filter_map(|a| serde_json::from_str::<Value>(&a.answer_data).ok().map(|v| (a.question_id.clone(), v)))
        .collect();
    Paper { questions, answers, allow_review: exam.exam.allow_review, total_points: exam.questions.iter().map(|q| q.question.points).sum() }
}

#[derive(Debug, Serialize, Clone, Copy)]
#[serde(rename_all = "camelCase")]
pub struct ResultSummary {
    pub score: f64,
    pub total_points: f64,
    pub percentage: f64,
    pub passed: bool,
}

/// What the student may see after submitting: nothing unless the teacher enabled results.
pub fn visible_result(exam: &Exam, attempt: &Attempt) -> Option<ResultSummary> {
    if !exam.show_results {
        return None;
    }
    Some(ResultSummary { score: attempt.score?, total_points: attempt.total_points?, percentage: attempt.percentage?, passed: attempt.passed? })
}

// ------------------------------------------------------------------ operations

fn time_is_up(session: &ExamSession, now: chrono::DateTime<Utc>) -> bool {
    let grace = chrono::Duration::from_std(config::ANSWER_GRACE).unwrap_or_default();
    session.ends_at.as_deref().and_then(timer::parse).is_some_and(|e| now > e + grace)
}

/// Opens the exam: marks the attempt IN_PROGRESS (first time) and builds the student's paper.
pub async fn start(db: &Database, session: &ExamSession, attempt_id: &str) -> Result<(Paper, Attempt), EngineError> {
    let attempt = db.get_attempt(attempt_id).await.map_err(internal)?;
    if attempt.status.is_final() {
        return Err(EngineError::AlreadySubmitted);
    }
    if session.status != SessionStatus::Running {
        return Err(EngineError::NotRunning);
    }
    if time_is_up(session, Utc::now()) {
        return Err(EngineError::TimeUp);
    }
    if attempt.status == AttemptStatus::Joined {
        db.set_attempt_status(attempt_id, AttemptStatus::InProgress).await.map_err(internal)?;
    }
    let exam = db.get_exam_full(&session.exam_id).await.map_err(internal)?;
    let saved = db.list_answers(attempt_id).await.map_err(internal)?;
    let attempt = db.get_attempt(attempt_id).await.map_err(internal)?;
    Ok((build_paper(&exam, attempt_id, &saved), attempt))
}

/// Saves one answer. `Ok(true)` stored, `Ok(false)` ignored as a duplicate/stale retry.
pub async fn save_answer(db: &Database, session: &ExamSession, attempt_id: &str, question_id: &str, raw: &Value, client_seq: i64) -> Result<bool, EngineError> {
    if client_seq < 0 {
        return Err(EngineError::Invalid("Invalid sequence number.".into()));
    }
    let attempt = db.get_attempt(attempt_id).await.map_err(internal)?;
    if attempt.status.is_final() {
        return Err(EngineError::AlreadySubmitted);
    }
    if session.status != SessionStatus::Running {
        return Err(EngineError::NotRunning);
    }
    if attempt.status == AttemptStatus::Joined {
        return Err(EngineError::NotStarted);
    }
    if time_is_up(session, Utc::now()) {
        return Err(EngineError::TimeUp);
    }
    let exam = db.get_exam_full(&session.exam_id).await.map_err(internal)?;
    let q = exam.questions.iter().find(|q| q.question.id == question_id).ok_or_else(|| EngineError::Invalid("That question is not part of this exam.".into()))?;
    let parsed = grading::parse_answer(q, raw).map_err(|m| EngineError::Invalid(m.into()))?;
    match db.upsert_answer(attempt_id, question_id, &parsed.to_json(), client_seq).await {
        Ok(stored) => Ok(stored),
        Err(AppError::Conflict(_)) => Err(EngineError::AlreadySubmitted),
        Err(AppError::Validation(m)) => Err(EngineError::Invalid(m)),
        Err(e) => Err(internal(e)),
    }
}

/// Student pressed Submit. Idempotent: submitting twice returns the first result.
pub async fn submit(db: &Database, session: &ExamSession, attempt_id: &str) -> Result<Option<ResultSummary>, EngineError> {
    let exam = db.get_exam_full(&session.exam_id).await.map_err(internal)?;
    let attempt = match db.finalize_attempt(attempt_id, AttemptStatus::Submitted, &exam).await.map_err(internal)? {
        Some(a) => {
            let _ = db.record_proctor_event(attempt_id, ProctorEventType::Submission, "Submitted by the student", None).await;
            a
        }
        None => db.get_attempt(attempt_id).await.map_err(internal)?,
    };
    Ok(visible_result(&exam.exam, &attempt))
}

/// Closes and grades every in-progress attempt (session end or time expiry) and tells each
/// student's connection. Returns how many were finalised.
/// Why attempts are being closed on the student's behalf (recorded in the proctoring timeline).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AutoReason {
    TimeExpired,
    SessionEnded,
}

pub async fn finalize_session_attempts(db: &Database, hub: &Hub, session: &ExamSession, reason: AutoReason) -> Result<usize, AppError> {
    let exam = db.get_exam_full(&session.exam_id).await?;
    let mut n = 0;
    for a in db.list_in_progress_attempts(&session.id).await? {
        if let Some(done) = db.finalize_attempt(&a.id, AttemptStatus::AutoSubmitted, &exam).await? {
            n += 1;
            let _ = match reason {
                AutoReason::TimeExpired => db.record_proctor_event(&done.id, ProctorEventType::Timeout, "Time ran out; submitted automatically", None).await,
                AutoReason::SessionEnded => db.record_proctor_event(&done.id, ProctorEventType::Submission, "Submitted automatically when the teacher ended the session", None).await,
            };
            hub.publish(Envelope::new(
                msg::SUBMITTED,
                Some(&session.id),
                json!({ "attemptId": done.id, "auto": true, "result": visible_result(&exam.exam, &done) }),
            ));
        }
    }
    Ok(n)
}

/// One pass of the deadline check. Announces `time_up` once per deadline and, when the exam
/// has auto-submit on, submits everyone still working.
pub async fn expire_due(db: &Database, hub: &Hub, notified: &mut HashSet<(String, String)>) -> Result<(), AppError> {
    let due = db.list_expired_running_sessions(Utc::now()).await?;
    let live: HashSet<(String, String)> = due.iter().map(|s| (s.id.clone(), s.ends_at.clone().unwrap_or_default())).collect();
    notified.retain(|k| live.contains(k));
    for s in due {
        let key = (s.id.clone(), s.ends_at.clone().unwrap_or_default());
        if !notified.insert(key) {
            continue;
        }
        let exam = db.get_exam_full(&s.exam_id).await?;
        tracing::info!(session_id = %s.id, auto_submit = exam.exam.auto_submit, "time is up");
        if exam.exam.auto_submit {
            finalize_session_attempts(db, hub, &s, AutoReason::TimeExpired).await?;
        }
        hub.publish(Envelope::new(msg::TIME_UP, Some(&s.id), json!({ "endsAt": s.ends_at, "autoSubmit": exam.exam.auto_submit, "serverTime": timer::format(Utc::now()) })));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shuffle_is_deterministic_per_seed_and_a_permutation() {
        let base: Vec<u32> = (0..20).collect();
        let (mut a, mut b, mut c) = (base.clone(), base.clone(), base.clone());
        seeded_shuffle(&mut a, "attempt-1");
        seeded_shuffle(&mut b, "attempt-1");
        seeded_shuffle(&mut c, "attempt-2");
        assert_eq!(a, b);
        assert_ne!(a, c);
        assert_ne!(a, base);
        let mut sorted = a.clone();
        sorted.sort();
        assert_eq!(sorted, base);
        let mut empty: Vec<u32> = vec![];
        seeded_shuffle(&mut empty, "x");
        let mut one = vec![1];
        seeded_shuffle(&mut one, "x");
        assert_eq!(one, vec![1]);
    }

    #[test]
    fn time_is_up_respects_the_grace_window() {
        let s = |ends: Option<&str>| ExamSession {
            id: "s".into(), exam_id: "e".into(), session_code: "ABCDE".into(), status: SessionStatus::Running, host_ip: "x".into(), host_port: 1,
            started_at: None, ends_at: ends.map(str::to_string), paused_at: None, paused_total_seconds: 0, created_at: String::new(), ended_at: None,
        };
        let ends = timer::parse("2026-10-04T10:00:00Z").unwrap();
        let at = |secs: i64| ends + chrono::Duration::seconds(secs);
        assert!(!time_is_up(&s(Some("2026-10-04T10:00:00Z")), at(-5)));
        assert!(!time_is_up(&s(Some("2026-10-04T10:00:00Z")), at(2)), "within grace");
        assert!(time_is_up(&s(Some("2026-10-04T10:00:00Z")), at(4)));
        assert!(!time_is_up(&s(None), at(1000)), "no deadline yet");
    }
}
