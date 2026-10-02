//! Teacher authentication commands (Tauri IPC only — never exposed on the LAN server).

use tauri::State;

use crate::errors::AppResult;
use crate::models::User;
use crate::services::auth_service::{AuthService, AuthStatus};

#[tauri::command]
pub async fn auth_status(auth: State<'_, AuthService>) -> AppResult<AuthStatus> {
    auth.status().await
}

#[tauri::command]
pub async fn setup_admin(auth: State<'_, AuthService>, username: String, password: String, display_name: String) -> AppResult<User> {
    auth.setup_admin(&username, &password, &display_name).await
}

#[tauri::command]
pub async fn login(auth: State<'_, AuthService>, username: String, password: String) -> AppResult<User> {
    auth.login(&username, &password).await
}

#[tauri::command]
pub async fn logout(auth: State<'_, AuthService>) -> AppResult<()> {
    auth.logout().await
}

#[tauri::command]
pub async fn change_password(auth: State<'_, AuthService>, current_password: String, new_password: String) -> AppResult<()> {
    auth.change_password(&current_password, &new_password).await
}

#[tauri::command]
pub async fn update_display_name(auth: State<'_, AuthService>, display_name: String) -> AppResult<User> {
    auth.update_display_name(&display_name).await
}
