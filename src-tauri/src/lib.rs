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
            // A restore staged in the previous run is applied now, before the database is opened.
            match tauri::async_runtime::block_on(services::backup_service::apply_pending_restore(&dir)) {
                Ok(true) => tracing::info!("a staged backup restore was applied"),
                Ok(false) => {}
                Err(e) => tracing::error!(error = %e, "could not apply the staged restore"),
            }
            let db_path = dir.join(config::DB_FILE_NAME);
            tracing::info!(path = %db_path.display(), "opening database");
            let db = tauri::async_runtime::block_on(database::Database::open(&db_path))?;
            app.manage(services::auth_service::AuthService::new(db.clone()));
            app.manage(services::exam_service::ExamService::new(db.clone()));

            // LAN server: a failed start (e.g. port in use) is reported in Settings, never fatal.
            let hub = server::hub::Hub::new();
            let status = server::new_status();
            let lan = std::sync::Arc::new(server::LanServer::new(db.clone(), hub.clone(), status.clone()));
            let network = services::network_service::NetworkService::new(db.clone(), lan.clone());
            tauri::async_runtime::block_on(async {
                if let Err(e) = network.start_saved().await {
                    tracing::warn!(error = %e, "LAN server did not start");
                }
            });
            let backups = services::backup_service::BackupService::new(db.clone(), dir.clone());
            match tauri::async_runtime::block_on(backups.auto_backup_if_due()) {
                Ok(Some(b)) => tracing::info!(file = %b.file_name, "automatic backup created"),
                Ok(None) => {}
                Err(e) => tracing::warn!(error = %e, "automatic backup failed"),
            }
            app.manage(backups);
            app.manage(services::exam_transfer::ExamTransfer::new(db.clone(), dir.join("exports")));
            app.manage(services::results_service::ResultsService::new(db.clone(), dir.join("exports")));
            app.manage(services::session_service::SessionService::new(db.clone(), hub, status));
            app.manage(network);
            app.manage(lan);
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
            commands::sessions::list_sessions,
            commands::sessions::get_session_snapshot,
            commands::sessions::create_session,
            commands::sessions::session_action,
            commands::sessions::list_session_roster,
            commands::sessions::remove_student,
            commands::sessions::list_session_events,
            commands::backup::backup_overview,
            commands::backup::create_backup,
            commands::backup::delete_backup,
            commands::backup::stage_restore,
            commands::backup::cancel_restore,
            commands::backup::set_auto_backup,
            commands::backup::restart_app,
            commands::backup::export_exam_file,
            commands::backup::import_exam_file,
            commands::results::list_result_sessions,
            commands::results::get_session_results,
            commands::results::get_attempt_detail,
            commands::results::export_session_csv,
            commands::results::list_students,
            commands::results::get_student_history,
            commands::network::network_info,
            commands::network::update_network,
        ])
        .run(tauri::generate_context!())
        .expect("error while running ProctorLAN");
}
