//! Teacher-only session control (Tauri IPC; never exposed on the LAN).

use tauri::State;

use crate::errors::AppResult;
use crate::models::SessionRow;
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
