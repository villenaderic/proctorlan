//! Teacher-only session control (Tauri IPC; never exposed on the LAN).

use tauri::State;

use crate::errors::AppResult;
use crate::models::{RosterRow, SessionRow};
use crate::services::auth_service::AuthService;
use crate::services::session_service::{SessionAction, SessionService, SessionSnapshot};

#[tauri::command]
pub async fn list_sessions(auth: State<'_, AuthService>, svc: State<'_, SessionService>) -> AppResult<Vec<SessionRow>> {
    auth.require_user().await?;
    svc.list().await
}

#[tauri::command]
pub async fn get_session_snapshot(auth: State<'_, AuthService>, svc: State<'_, SessionService>, id: String) -> AppResult<SessionSnapshot> {
    auth.require_user().await?;
    svc.snapshot(&id).await
}

#[tauri::command]
pub async fn create_session(auth: State<'_, AuthService>, svc: State<'_, SessionService>, exam_id: String) -> AppResult<SessionSnapshot> {
    let user = auth.require_user().await?;
    svc.create(&user, &exam_id).await
}

#[tauri::command]
pub async fn session_action(auth: State<'_, AuthService>, svc: State<'_, SessionService>, id: String, action: SessionAction) -> AppResult<SessionSnapshot> {
    let user = auth.require_user().await?;
    svc.act(&user, &id, action).await
}

#[tauri::command]
pub async fn list_session_roster(auth: State<'_, AuthService>, svc: State<'_, SessionService>, id: String) -> AppResult<Vec<RosterRow>> {
    auth.require_user().await?;
    svc.roster(&id).await
}

#[tauri::command]
pub async fn remove_student(auth: State<'_, AuthService>, svc: State<'_, SessionService>, attempt_id: String) -> AppResult<()> {
    let user = auth.require_user().await?;
    svc.remove_student(&user, &attempt_id).await
}
