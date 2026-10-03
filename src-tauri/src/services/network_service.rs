//! Network settings: which LAN interface and port the student server uses.

use std::sync::Arc;

use serde::Serialize;

use crate::config;
use crate::database::Database;
use crate::errors::{AppError, AppResult};
use crate::models::User;
use crate::networking::interfaces::{choose_bind_ip, list_interfaces, NetInterface};
use crate::server::{LanServer, ServerStatus};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkInfo {
    pub status: ServerStatus,
    pub interfaces: Vec<NetInterface>,
    /// Saved choice (empty = automatic).
    pub preferred_ip: Option<String>,
    pub configured_port: u16,
    pub default_port: u16,
    /// Students join with `ip:port` + session code.
    pub join_address: Option<String>,
}

pub struct NetworkService {
    db: Database,
    server: Arc<LanServer>,
}

pub fn validate_port(port: i64) -> AppResult<u16> {
    if !(1024..=65_535).contains(&port) {
        return Err(AppError::Validation("Port must be between 1024 and 65535.".into()));
    }
    Ok(port as u16)
}

impl NetworkService {
    pub fn new(db: Database, server: Arc<LanServer>) -> Self {
        Self { db, server }
    }

    async fn saved(&self) -> AppResult<(Option<String>, u16)> {
        let ip = self.db.get_setting(config::SETTING_INTERFACE).await?.filter(|s| !s.is_empty());
        let port = self.db.get_setting(config::SETTING_PORT).await?.and_then(|p| p.parse::<i64>().ok()).and_then(|p| validate_port(p).ok()).unwrap_or(config::DEFAULT_PORT);
        Ok((ip, port))
    }

    /// Starts the server with saved settings. Failures are recorded in the status, not fatal.
    pub async fn start_saved(&self) -> AppResult<ServerStatus> {
        let (pref, port) = self.saved().await?;
        let ip = choose_bind_ip(&list_interfaces(), pref.as_deref());
        self.server.start(ip, port, true).await
    }

    pub async fn info(&self) -> AppResult<NetworkInfo> {
        let (preferred_ip, configured_port) = self.saved().await?;
        let status = self.server.status();
        let join_address = status.running.then(|| format!("{}:{}", status.ip, status.port));
        Ok(NetworkInfo { status, interfaces: list_interfaces(), preferred_ip, configured_port, default_port: config::DEFAULT_PORT, join_address })
    }

    /// Saves and applies new settings. Refused while a session is open: restarting would drop
    /// every student's connection mid-exam.
    pub async fn apply(&self, user: &User, ip: Option<String>, port: i64) -> AppResult<NetworkInfo> {
        let port = validate_port(port)?;
        let interfaces = list_interfaces();
        let ip = ip.filter(|s| !s.trim().is_empty());
        if let Some(p) = &ip {
            if !interfaces.iter().any(|i| &i.ip == p) {
                return Err(AppError::Validation("That network address is no longer available on this computer.".into()));
            }
        }
        let open = self.db.list_sessions().await?.iter().any(|s| s.status != crate::models::SessionStatus::Ended);
        if open {
            return Err(AppError::Conflict("End the open session before changing network settings. Restarting now would disconnect students.".into()));
        }
        self.db.set_setting(config::SETTING_INTERFACE, ip.as_deref().unwrap_or("")).await?;
        self.db.set_setting(config::SETTING_PORT, &port.to_string()).await?;
        self.db.audit(Some(&user.id), "network.update", "settings", None, None).await?;
        let bind = choose_bind_ip(&interfaces, ip.as_deref());
        // A failed start is reported to the UI through the returned status and error.
        let _ = self.server.start(bind, port, true).await;
        self.info().await
    }
}
