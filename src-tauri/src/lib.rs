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
        .setup(|app| {
            use tauri::Manager;
            let dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&dir)?;
            let db_path = dir.join(config::DB_FILE_NAME);
            tracing::info!(path = %db_path.display(), "opening database");
            let db = tauri::async_runtime::block_on(database::Database::open(&db_path))?;
            app.manage(services::auth_service::AuthService::new(db.clone()));
            app.manage(services::exam_service::ExamService::new(db.clone()));
            app.manage(db);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::app_info,
            commands::db_status,
            commands::dashboard_stats,
            commands::auth::auth_status,
            commands::auth::setup_admin,
            commands::auth::login,
            commands::auth::logout,
            commands::auth::change_password,
            commands::auth::update_display_name,
            commands::exams::list_exams,
            commands::exams::get_exam,
            commands::exams::validate_exam,
            commands::exams::create_exam,
            commands::exams::update_exam,
            commands::exams::set_exam_active,
            commands::exams::duplicate_exam,
            commands::exams::delete_exam,
        ])
        .run(tauri::generate_context!())
        .expect("error while running ProctorLAN");
}
