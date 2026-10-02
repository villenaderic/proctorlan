//! Teacher exam commands. Every command requires a signed-in teacher.

use tauri::State;

use crate::errors::AppResult;
use crate::models::{ExamFull, ExamSummary, NewExam};
use crate::services::auth_service::AuthService;
use crate::services::exam_service::ExamService;
use crate::services::exam_validation::ValidationIssue;

#[tauri::command]
pub async fn list_exams(auth: State<'_, AuthService>, exams: State<'_, ExamService>) -> AppResult<Vec<ExamSummary>> {
    auth.require_user().await?;
    exams.list().await
}

#[tauri::command]
pub async fn get_exam(auth: State<'_, AuthService>, exams: State<'_, ExamService>, id: String) -> AppResult<ExamFull> {
    auth.require_user().await?;
    exams.get(&id).await
}

#[tauri::command]
pub async fn validate_exam(auth: State<'_, AuthService>, exams: State<'_, ExamService>, exam: NewExam) -> AppResult<Vec<ValidationIssue>> {
    auth.require_user().await?;
    Ok(exams.check(&exam))
}

#[tauri::command]
pub async fn create_exam(auth: State<'_, AuthService>, exams: State<'_, ExamService>, exam: NewExam) -> AppResult<ExamFull> {
    let user = auth.require_user().await?;
    exams.create(&user, exam).await
}

#[tauri::command]
pub async fn update_exam(auth: State<'_, AuthService>, exams: State<'_, ExamService>, id: String, exam: NewExam) -> AppResult<ExamFull> {
    let user = auth.require_user().await?;
    exams.update(&user, &id, exam).await
}

#[tauri::command]
pub async fn set_exam_active(auth: State<'_, AuthService>, exams: State<'_, ExamService>, id: String, active: bool) -> AppResult<ExamFull> {
    let user = auth.require_user().await?;
    exams.set_active(&user, &id, active).await
}

#[tauri::command]
pub async fn duplicate_exam(auth: State<'_, AuthService>, exams: State<'_, ExamService>, id: String) -> AppResult<ExamFull> {
    let user = auth.require_user().await?;
    exams.duplicate(&user, &id).await
}

#[tauri::command]
pub async fn delete_exam(auth: State<'_, AuthService>, exams: State<'_, ExamService>, id: String) -> AppResult<()> {
    let user = auth.require_user().await?;
    exams.delete(&user, &id).await
}
