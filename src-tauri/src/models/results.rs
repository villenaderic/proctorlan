//! Teacher-side result and analytics types (they include answer keys; never sent to students).

use serde::Serialize;
use sqlx::FromRow;

use super::{AttemptStatus, ProctorEvent, QuestionType};

/// One finished (or unfinished) attempt in a session's results table.
#[derive(Debug, Clone, Serialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct ResultRow {
    pub attempt_id: String,
    pub student_number: String,
    pub name: String,
    pub status: AttemptStatus,
    pub started_at: Option<String>,
    pub submitted_at: Option<String>,
    pub score: Option<f64>,
    pub total_points: Option<f64>,
    pub percentage: Option<f64>,
    pub passed: Option<bool>,
    pub answered: i64,
    pub time_taken_seconds: Option<i64>,
    pub focus_lost_count: i64,
    pub focus_lost_ms: i64,
    pub disconnect_count: i64,
}

/// A session in the Results list.
#[derive(Debug, Clone, Serialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct ResultSessionRow {
    pub session_id: String,
    pub exam_title: String,
    pub session_code: String,
    pub status: String,
    pub created_at: String,
    pub ended_at: Option<String>,
    pub joined: i64,
    pub submitted: i64,
    pub passed: i64,
    pub average_percentage: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ScoreStats {
    pub submitted: i64,
    pub not_submitted: i64,
    pub passed: i64,
    pub failed: i64,
    pub pass_rate: f64,
    pub mean: f64,
    pub median: f64,
    pub highest: f64,
    pub lowest: f64,
    pub std_dev: f64,
    pub average_time_seconds: Option<i64>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Bucket {
    pub label: String,
    pub from: u32,
    pub to: u32,
    pub count: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OptionStat {
    pub text: String,
    pub is_correct: bool,
    pub picked: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WrongAnswer {
    pub text: String,
    pub count: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QuestionStat {
    pub question_id: String,
    pub position: i64,
    pub text: String,
    pub question_type: QuestionType,
    pub points: f64,
    /// Submitted attempts the question was shown to.
    pub attempts: i64,
    pub answered: i64,
    pub correct: i64,
    pub percent_correct: f64,
    /// Choice questions: how often each option was picked.
    pub options: Vec<OptionStat>,
    /// Identification: the most common wrong answers.
    pub common_wrong: Vec<WrongAnswer>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionResults {
    pub session_id: String,
    pub exam_title: String,
    pub session_code: String,
    pub status: String,
    pub created_at: String,
    pub ended_at: Option<String>,
    pub passing_score: f64,
    pub total_points: f64,
    pub question_count: i64,
    pub rows: Vec<ResultRow>,
    pub stats: ScoreStats,
    pub distribution: Vec<Bucket>,
    pub questions: Vec<QuestionStat>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AnswerDetail {
    pub position: i64,
    pub question_id: String,
    pub text: String,
    pub question_type: QuestionType,
    pub points: f64,
    pub points_awarded: f64,
    pub answered: bool,
    pub is_correct: bool,
    /// What the student gave, as readable text (empty if unanswered).
    pub given: Vec<String>,
    /// The correct (or accepted) answers.
    pub correct: Vec<String>,
    pub explanation: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AttemptDetail {
    pub attempt_id: String,
    pub session_id: String,
    pub exam_title: String,
    pub student_number: String,
    pub name: String,
    pub status: AttemptStatus,
    pub started_at: Option<String>,
    pub submitted_at: Option<String>,
    pub score: Option<f64>,
    pub total_points: Option<f64>,
    pub percentage: Option<f64>,
    pub passed: Option<bool>,
    pub passing_score: f64,
    pub answers: Vec<AnswerDetail>,
    pub events: Vec<ProctorEvent>,
}

#[derive(Debug, Clone, Serialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct StudentSummary {
    pub id: String,
    pub student_number: String,
    pub name: String,
    pub attempts: i64,
    pub average_percentage: Option<f64>,
    pub last_attempt_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct StudentAttemptRow {
    pub attempt_id: String,
    pub session_id: String,
    pub exam_title: String,
    pub status: AttemptStatus,
    pub submitted_at: Option<String>,
    pub score: Option<f64>,
    pub total_points: Option<f64>,
    pub percentage: Option<f64>,
    pub passed: Option<bool>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportInfo {
    pub path: String,
    pub file_name: String,
    pub rows: usize,
}
