//! Teacher-only network settings (Tauri IPC).

use tauri::State;

use crate::errors::AppResult;
use crate::services::auth_service::AuthService;
use crate::services::network_service::{NetworkInfo, NetworkService};

#[tauri::command]
pub async fn network_info(auth: State<'_, AuthService>, svc: State<'_, NetworkService>) -> AppResult<NetworkInfo> {
    auth.require_user().await?;
    svc.info().await
}

#[tauri::command]
pub async fn update_network(auth: State<'_, AuthService>, svc: State<'_, NetworkService>, ip: Option<String>, port: i64) -> AppResult<NetworkInfo> {
    let user = auth.require_user().await?;
    svc.apply(&user, ip, port).await
}
