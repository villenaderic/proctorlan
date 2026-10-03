//! Tauri commands exposed to the React frontend.

pub mod auth;
pub mod exams;
pub mod network;
pub mod sessions;

use serde::Serialize;
use tauri::{AppHandle, Manager, State};

use crate::config::APP_NAME;
use crate::database::Database;
use crate::errors::{AppError, AppResult};
use crate::models::Stats;
use crate::services::auth_service::AuthService;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    pub name: &'static str,
    pub version: &'static str,
    pub os: &'static str,
    pub arch: &'static str,
    pub data_dir: String,
}

#[tauri::command]
pub fn app_info(app: AppHandle) -> AppResult<AppInfo> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| AppError::Internal(e.to_string()))?;
    Ok(AppInfo {
        name: APP_NAME,
        version: env!("CARGO_PKG_VERSION"),
        os: std::env::consts::OS,
        arch: std::env::consts::ARCH,
        data_dir: dir.display().to_string(),
    })
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DbStatus {
    pub ready: bool,
    pub migrations_applied: i64,
}

/// Public health check: proves migrations ran and the database is reachable. No exam data.
#[tauri::command]
pub async fn db_status(db: State<'_, Database>) -> AppResult<DbStatus> {
    Ok(DbStatus { ready: true, migrations_applied: db.migrations_applied().await? })
}

/// Teacher-only dashboard numbers, read from SQLite.
#[tauri::command]
pub async fn dashboard_stats(auth: State<'_, AuthService>, db: State<'_, Database>) -> AppResult<Stats> {
    auth.require_user().await?;
    db.stats().await
}
