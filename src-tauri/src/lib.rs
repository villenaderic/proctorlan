pub mod commands;
pub mod config;
pub mod errors;
// Reserved module boundaries, filled in by later phases (see docs/architecture.md).
pub mod auth;
pub mod database;
pub mod models;
pub mod networking;
pub mod proctoring;
pub mod server;
pub mod services;
pub mod websocket;

use tracing_subscriber::EnvFilter;

fn init_logging() {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(config::DEFAULT_LOG_FILTER));
    tracing_subscriber::fmt().with_env_filter(filter).init();
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    init_logging();
    tracing::info!(version = env!("CARGO_PKG_VERSION"), "{} starting", config::APP_NAME);
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![commands::app_info])
        .run(tauri::generate_context!())
        .expect("error while running ProctorLAN");
}
