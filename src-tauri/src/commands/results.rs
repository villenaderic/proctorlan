//! Teacher-only results, student history and exports (Tauri IPC; never exposed on the LAN).

use tauri::State;

use crate::errors::AppResult;
use crate::models::*;
use crate::services::auth_service::AuthService;
use crate::services::results_service::ResultsService;

#[tauri::command]
pub async fn list_result_sessions(auth: State<'_, AuthService>, svc: State<'_, ResultsService>) -> AppResult<Vec<ResultSessionRow>> {
    auth.require_user().await?;
    svc.sessions().await
}

#[tauri::command]
pub async fn get_session_results(auth: State<'_, AuthService>, svc: State<'_, ResultsService>, id: String) -> AppResult<SessionResults> {
    auth.require_user().await?;
    svc.session_results(&id).await
}

#[tauri::command]
pub async fn get_attempt_detail(auth: State<'_, AuthService>, svc: State<'_, ResultsService>, attempt_id: String) -> AppResult<AttemptDetail> {
    auth.require_user().await?;
    svc.attempt_detail(&attempt_id).await
}

#[tauri::command]
pub async fn export_session_csv(auth: State<'_, AuthService>, svc: State<'_, ResultsService>, id: String) -> AppResult<ExportInfo> {
    auth.require_user().await?;
    svc.export_csv(&id).await
}

#[tauri::command]
pub async fn list_students(auth: State<'_, AuthService>, svc: State<'_, ResultsService>) -> AppResult<Vec<StudentSummary>> {
    auth.require_user().await?;
    svc.students().await
}

#[tauri::command]
pub async fn get_student_history(auth: State<'_, AuthService>, svc: State<'_, ResultsService>, student_id: String) -> AppResult<Vec<StudentAttemptRow>> {
    auth.require_user().await?;
    svc.student_history(&student_id).await
}
