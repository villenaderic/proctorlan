//! Database row types and input DTOs. Teacher-side types (they contain answer keys);
//! student-facing payloads are built separately in Phase 6 and never include `is_correct`.

use serde::{Deserialize, Serialize};
use sqlx::FromRow;

mod backup;
mod results;
pub use backup::*;
pub use results::*;

macro_rules! text_enum {
    ($(#[$m:meta])* $name:ident, $rule:literal, { $($variant:ident),+ $(,)? }) => {
        $(#[$m])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
        #[sqlx(rename_all = $rule)]
        #[serde(rename_all = $rule)]
        pub enum $name { $($variant),+ }
    };
}

text_enum!(UserRole, "snake_case", { Admin, Teacher });
text_enum!(ExamStatus, "snake_case", { Active, Inactive });
text_enum!(QuestionType, "snake_case", { MultipleChoice, TrueFalse, MultipleSelect, Identification });
text_enum!(SessionStatus, "SCREAMING_SNAKE_CASE", { Created, Waiting, Running, Paused, Ended });
text_enum!(AttemptStatus, "SCREAMING_SNAKE_CASE", { Joined, InProgress, Submitted, AutoSubmitted });
text_enum!(ProctorEventType, "SCREAMING_SNAKE_CASE", { FocusLost, FocusRestored, Disconnected, Reconnected, Submission, Timeout });

impl SessionStatus {
    /// Allowed lifecycle: CREATED → WAITING → RUNNING ⇄ PAUSED → ENDED (any non-ended state may end).
    pub fn can_transition_to(self, next: SessionStatus) -> bool {
        use SessionStatus::*;
        matches!(
            (self, next),
            (Created, Waiting) | (Waiting, Running) | (Running, Paused) | (Paused, Running)
                | (Created, Ended) | (Waiting, Ended) | (Running, Ended) | (Paused, Ended)
        )
    }

    /// Students may join sessions in these states.
    pub fn accepts_students(self) -> bool {
        matches!(self, SessionStatus::Waiting | SessionStatus::Running | SessionStatus::Paused)
    }
}

impl AttemptStatus {
    pub fn is_final(self) -> bool {
        matches!(self, AttemptStatus::Submitted | AttemptStatus::AutoSubmitted)
    }
}

#[derive(Debug, Clone, Serialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct User {
    pub id: String,
    pub username: String,
    #[serde(skip)]
    pub password_hash: String,
    pub display_name: String,
    pub role: UserRole,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct Exam {
    pub id: String,
    pub title: String,
    pub description: String,
    pub instructions: String,
    pub duration_minutes: i64,
    pub passing_score: f64,
    pub randomize_questions: bool,
    pub randomize_choices: bool,
    pub allow_review: bool,
    pub auto_submit: bool,
    pub show_results: bool,
    pub status: ExamStatus,
    pub created_by: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct ExamSummary {
    #[sqlx(flatten)]
    #[serde(flatten)]
    pub exam: Exam,
    pub question_count: i64,
    pub total_points: f64,
    pub session_count: i64,
}

#[derive(Debug, Clone, Serialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct Question {
    pub id: String,
    pub exam_id: String,
    pub question_text: String,
    pub question_type: QuestionType,
    pub points: f64,
    pub sort_order: i64,
    pub required: bool,
    pub explanation: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct Choice {
    pub id: String,
    pub question_id: String,
    pub choice_text: String,
    pub is_correct: bool,
    pub sort_order: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QuestionFull {
    #[serde(flatten)]
    pub question: Question,
    pub choices: Vec<Choice>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExamFull {
    #[serde(flatten)]
    pub exam: Exam,
    pub questions: Vec<QuestionFull>,
    /// Sessions that used this exam; when > 0 the exam is locked (cannot be edited or deleted).
    pub session_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewChoice {
    pub choice_text: String,
    pub is_correct: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewQuestion {
    pub question_text: String,
    pub question_type: QuestionType,
    pub points: f64,
    #[serde(default = "default_true")]
    pub required: bool,
    pub explanation: Option<String>,
    #[serde(default)]
    pub choices: Vec<NewChoice>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewExam {
    pub title: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub instructions: String,
    pub duration_minutes: i64,
    pub passing_score: f64,
    #[serde(default)]
    pub randomize_questions: bool,
    #[serde(default)]
    pub randomize_choices: bool,
    #[serde(default = "default_true")]
    pub allow_review: bool,
    #[serde(default = "default_true")]
    pub auto_submit: bool,
    #[serde(default)]
    pub show_results: bool,
    #[serde(default)]
    pub questions: Vec<NewQuestion>,
}

fn default_true() -> bool { true }

impl From<&ExamFull> for NewExam {
    fn from(f: &ExamFull) -> Self {
        NewExam {
            title: f.exam.title.clone(),
            description: f.exam.description.clone(),
            instructions: f.exam.instructions.clone(),
            duration_minutes: f.exam.duration_minutes,
            passing_score: f.exam.passing_score,
            randomize_questions: f.exam.randomize_questions,
            randomize_choices: f.exam.randomize_choices,
            allow_review: f.exam.allow_review,
            auto_submit: f.exam.auto_submit,
            show_results: f.exam.show_results,
            questions: f.questions.iter().map(|q| NewQuestion {
                question_text: q.question.question_text.clone(),
                question_type: q.question.question_type,
                points: q.question.points,
                required: q.question.required,
                explanation: q.question.explanation.clone(),
                choices: q.choices.iter().map(|c| NewChoice { choice_text: c.choice_text.clone(), is_correct: c.is_correct }).collect(),
            }).collect(),
        }
    }
}

#[derive(Debug, Clone, Serialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct ExamSession {
    pub id: String,
    pub exam_id: String,
    pub session_code: String,
    pub status: SessionStatus,
    pub host_ip: String,
    pub host_port: i64,
    pub started_at: Option<String>,
    pub ends_at: Option<String>,
    pub paused_at: Option<String>,
    pub paused_total_seconds: i64,
    pub created_at: String,
    pub ended_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct SessionRow {
    #[sqlx(flatten)]
    #[serde(flatten)]
    pub session: ExamSession,
    pub exam_title: String,
}

#[derive(Debug, Clone, Serialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct Student {
    pub id: String,
    pub student_number: String,
    pub name: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct Attempt {
    pub id: String,
    pub session_id: String,
    pub student_id: String,
    pub status: AttemptStatus,
    #[serde(skip)]
    pub token_hash: String,
    pub started_at: Option<String>,
    pub submitted_at: Option<String>,
    pub score: Option<f64>,
    pub total_points: Option<f64>,
    pub percentage: Option<f64>,
    pub passed: Option<bool>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct Answer {
    pub id: String,
    pub attempt_id: String,
    pub question_id: String,
    pub answer_data: String,
    pub client_seq: i64,
    pub is_correct: Option<bool>,
    pub points_awarded: Option<f64>,
    pub answered_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct ProctorEvent {
    pub id: String,
    pub attempt_id: String,
    pub event_type: ProctorEventType,
    pub description: String,
    pub metadata: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct Stats {
    pub exams: i64,
    pub active_sessions: i64,
    pub students: i64,
    pub completed_attempts: i64,
}

#[cfg(test)]
mod tests {
    use super::SessionStatus::*;

    #[test]
    fn transitions() {
        assert!(Created.can_transition_to(Waiting));
        assert!(Running.can_transition_to(Paused) && Paused.can_transition_to(Running));
        assert!(!Created.can_transition_to(Running));
        assert!(!Ended.can_transition_to(Waiting) && !Ended.can_transition_to(Ended));
        assert!(Waiting.accepts_students() && !Created.accepts_students() && !Ended.accepts_students());
    }
}

/// One row of the teacher's live roster (online flag is filled in from the connection hub).
#[derive(Debug, Clone, Serialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct RosterRow {
    pub attempt_id: String,
    pub student_number: String,
    pub name: String,
    pub status: AttemptStatus,
    pub joined_at: String,
    pub submitted_at: Option<String>,
    /// Questions with a non-empty saved answer.
    pub answered: i64,
    pub percentage: Option<f64>,
    pub passed: Option<bool>,
    /// Proctoring summary (see services/proctoring.rs).
    pub focus_lost_count: i64,
    pub focus_lost_ms: i64,
    pub disconnect_count: i64,
    #[sqlx(skip)]
    pub online: bool,
}

/// A proctoring event with the student's identity, for the teacher's live feed.
#[derive(Debug, Clone, Serialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct SessionEvent {
    pub id: String,
    pub attempt_id: String,
    pub student_name: String,
    pub student_number: String,
    pub event_type: ProctorEventType,
    pub description: String,
    pub metadata: Option<String>,
    pub created_at: String,
}
