//! Teacher-side exam workflows: validate → persist → audit. Commands are thin wrappers.

use crate::database::Database;
use crate::errors::{AppError, AppResult};
use crate::models::*;
use crate::services::exam_validation::{normalize, validate_exam, ValidationIssue};

pub struct ExamService {
    db: Database,
}

/// Turns the first few issues into one user-readable error.
fn reject(context: &str, issues: Vec<ValidationIssue>) -> AppResult<()> {
    let Some(first) = issues.first() else { return Ok(()) };
    let more = if issues.len() > 1 { format!(" (and {} more)", issues.len() - 1) } else { String::new() };
    Err(AppError::Validation(format!("{context}{}{more}", first.message)))
}

impl ExamService {
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    pub async fn list(&self) -> AppResult<Vec<ExamSummary>> {
        self.db.list_exams().await
    }

    pub async fn get(&self, id: &str) -> AppResult<ExamFull> {
        self.db.get_exam_full(id).await
    }

    /// Live feedback for the builder: everything that must hold before an exam can run.
    pub fn check(&self, input: &NewExam) -> Vec<ValidationIssue> {
        let mut copy = input.clone();
        normalize(&mut copy);
        validate_exam(&copy, true)
    }

    pub async fn create(&self, user: &User, mut input: NewExam) -> AppResult<ExamFull> {
        normalize(&mut input);
        reject("", validate_exam(&input, false))?;
        let id = self.db.create_exam(Some(&user.id), &input).await?;
        self.db.audit(Some(&user.id), "exam.create", "exam", Some(&id), None).await?;
        tracing::info!(exam_id = %id, "exam created");
        self.db.get_exam_full(&id).await
    }

    /// An active exam must stay runnable, so it is validated at the stricter "ready" level.
    pub async fn update(&self, user: &User, id: &str, mut input: NewExam) -> AppResult<ExamFull> {
        normalize(&mut input);
        let current = self.db.get_exam_full(id).await?;
        let must_be_ready = current.exam.status == ExamStatus::Active;
        reject(if must_be_ready { "This exam is active, so it must stay complete. " } else { "" }, validate_exam(&input, must_be_ready))?;
        self.db.replace_exam(id, &input).await?;
        self.db.audit(Some(&user.id), "exam.update", "exam", Some(id), None).await?;
        self.db.get_exam_full(id).await
    }

    pub async fn set_active(&self, user: &User, id: &str, active: bool) -> AppResult<ExamFull> {
        if active {
            let full = self.db.get_exam_full(id).await?;
            reject("This exam is not ready to run. ", validate_exam(&NewExam::from(&full), true))?;
        }
        self.db.set_exam_status(id, if active { ExamStatus::Active } else { ExamStatus::Inactive }).await?;
        let action = if active { "exam.activate" } else { "exam.deactivate" };
        self.db.audit(Some(&user.id), action, "exam", Some(id), None).await?;
        self.db.get_exam_full(id).await
    }

    pub async fn duplicate(&self, user: &User, id: &str) -> AppResult<ExamFull> {
        let new_id = self.db.duplicate_exam(id, Some(&user.id)).await?;
        self.db.audit(Some(&user.id), "exam.duplicate", "exam", Some(&new_id), Some(id)).await?;
        self.db.get_exam_full(&new_id).await
    }

    pub async fn delete(&self, user: &User, id: &str) -> AppResult<()> {
        self.db.delete_exam(id).await?;
        self.db.audit(Some(&user.id), "exam.delete", "exam", Some(id), None).await?;
        Ok(())
    }
}
