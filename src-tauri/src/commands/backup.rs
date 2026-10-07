//! Teacher-only backup, restore and exam file commands (Tauri IPC; never exposed on the LAN).

use tauri::{AppHandle, State};

use crate::errors::AppResult;
use crate::models::*;
use crate::services::auth_service::AuthService;
use crate::services::backup_service::BackupService;
use crate::services::exam_transfer::ExamTransfer;

#[tauri::command]
pub async fn backup_overview(auth: State<'_, AuthService>, svc: State<'_, BackupService>) -> AppResult<BackupOverview> {
    auth.require_user().await?;
    svc.overview().await
}

#[tauri::command]
pub async fn create_backup(auth: State<'_, AuthService>, svc: State<'_, BackupService>, dest_dir: Option<String>) -> AppResult<BackupInfo> {
    let user = auth.require_user().await?;
    svc.create(Some(&user.id), "manual", dest_dir.as_deref()).await
}

#[tauri::command]
pub async fn delete_backup(auth: State<'_, AuthService>, svc: State<'_, BackupService>, name: String) -> AppResult<()> {
    let user = auth.require_user().await?;
    svc.delete(Some(&user.id), &name).await
}

#[tauri::command]
pub async fn stage_restore(auth: State<'_, AuthService>, svc: State<'_, BackupService>, name: Option<String>, path: Option<String>) -> AppResult<BackupContents> {
    let user = auth.require_user().await?;
    match (name, path) {
        (Some(n), _) => svc.stage_named(Some(&user.id), &n).await,
        (None, Some(p)) => svc.stage_path(Some(&user.id), &p).await,
        _ => Err(crate::errors::AppError::Validation("Choose a backup to restore.".into())),
    }
}

#[tauri::command]
pub async fn cancel_restore(auth: State<'_, AuthService>, svc: State<'_, BackupService>) -> AppResult<()> {
    auth.require_user().await?;
    svc.cancel_pending().await
}

#[tauri::command]
pub async fn set_auto_backup(auth: State<'_, AuthService>, svc: State<'_, BackupService>, enabled: bool) -> AppResult<()> {
    auth.require_user().await?;
    svc.set_auto(enabled).await
}

/// Restarts the app so a staged restore is applied.
#[tauri::command]
pub async fn restart_app(auth: State<'_, AuthService>, app: AppHandle) -> AppResult<()> {
    auth.require_user().await?;
    app.restart();
}

#[tauri::command]
pub async fn export_exam_file(auth: State<'_, AuthService>, svc: State<'_, ExamTransfer>, id: String) -> AppResult<ExportInfo> {
    auth.require_user().await?;
    svc.export(&id).await
}

#[tauri::command]
pub async fn import_exam_file(auth: State<'_, AuthService>, svc: State<'_, ExamTransfer>, path: String) -> AppResult<ExamFull> {
    let user = auth.require_user().await?;
    svc.import(&user, &path).await
}
