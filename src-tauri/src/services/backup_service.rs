//! Database backup and restore.
//!
//! Backups are consistent single-file snapshots made with `VACUUM INTO` (safe while the app is running).
//! Restoring a live SQLite database is unreliable on Windows (open file handles), so a restore is only
//! *staged* as `restore-pending.db`; it is validated again and swapped in at the next start, before the
//! database is opened. The database being replaced is snapshotted first, so a restore is never destructive.

use std::path::{Path, PathBuf};

use chrono::{DateTime, Duration as ChronoDuration, Utc};
use sqlx::sqlite::{SqliteConnectOptions, SqliteConnection};
use sqlx::Connection;

use crate::config;
use crate::database::Database;
use crate::errors::{AppError, AppResult};
use crate::models::*;

pub const PENDING_FILE: &str = "restore-pending.db";
const PREFIX: &str = "proctorlan-";
const KINDS: [&str; 4] = ["manual", "auto", "prerestore", "corrupt"];
const SETTING_AUTO: &str = "backup.auto";
/// Automatic backups kept (manual and safety backups are never pruned).
pub const KEEP_AUTO: usize = 10;
/// A new automatic backup is made at start-up when the newest backup is older than this.
pub const AUTO_INTERVAL_HOURS: i64 = 24;

pub fn backups_dir(data_dir: &Path) -> PathBuf {
    data_dir.join("backups")
}

fn stamp() -> String {
    Utc::now().format("%Y%m%d-%H%M%S").to_string()
}

fn file_name(kind: &str) -> String {
    format!("{PREFIX}{kind}-{}.db", stamp())
}

/// `proctorlan-<kind>-<timestamp>.db` → kind.
pub fn kind_of(name: &str) -> Option<&'static str> {
    let rest = name.strip_prefix(PREFIX)?.strip_suffix(".db")?;
    KINDS.iter().copied().find(|k| rest.starts_with(&format!("{k}-")))
}

/// Only plain file names inside the backups folder are accepted (no separators, no traversal).
fn safe_name(name: &str) -> AppResult<()> {
    if name.is_empty() || name.contains(['/', '\\']) || name.contains("..") || kind_of(name).is_none() {
        return Err(AppError::Validation("That is not a ProctorLAN backup name.".into()));
    }
    Ok(())
}

fn latest_schema() -> i64 {
    sqlx::migrate!("./migrations").iter().map(|m| m.version).max().unwrap_or(0)
}

async fn connect_ro(path: &Path) -> AppResult<SqliteConnection> {
    let opts = SqliteConnectOptions::new().filename(path).read_only(true).create_if_missing(false);
    Ok(SqliteConnection::connect_with(&opts).await?)
}

/// `VACUUM INTO` a new file (which must not exist yet).
async fn snapshot_to(conn: &mut SqliteConnection, dest: &Path) -> AppResult<()> {
    if dest.exists() {
        return Err(AppError::Conflict("A backup with that name already exists.".into()));
    }
    let target = dest.to_str().ok_or_else(|| AppError::Validation("The backup path is not valid text.".into()))?;
    sqlx::query("VACUUM INTO ?").bind(target).execute(&mut *conn).await?;
    Ok(())
}

/// Opens a file read-only and checks that it is a healthy ProctorLAN database this version can use.
pub async fn validate_backup(path: &Path) -> AppResult<BackupContents> {
    let meta = tokio::fs::metadata(path).await.map_err(|_| AppError::NotFound("That backup file was not found.".into()))?;
    if !meta.is_file() || meta.len() < 4096 {
        return Err(AppError::Validation("That file is not a ProctorLAN backup.".into()));
    }
    let bad = || AppError::Validation("That file is not a valid ProctorLAN backup (it may be damaged or from another program).".into());
    let mut c = connect_ro(path).await.map_err(|_| bad())?;
    let integrity: String = sqlx::query_scalar("PRAGMA integrity_check").fetch_one(&mut c).await.map_err(|_| bad())?;
    if integrity != "ok" {
        return Err(AppError::Validation("That backup failed its integrity check and cannot be used.".into()));
    }
    let schema: Option<i64> = sqlx::query_scalar("SELECT MAX(version) FROM _sqlx_migrations").fetch_one(&mut c).await.map_err(|_| bad())?;
    let schema = schema.ok_or_else(bad)?;
    if schema > latest_schema() {
        return Err(AppError::Validation("That backup was made by a newer version of ProctorLAN. Update the app first.".into()));
    }
    let mut n = [0i64; 4];
    for (i, t) in ["exams", "exam_sessions", "students", "attempts"].iter().enumerate() {
        n[i] = sqlx::query_scalar(&format!("SELECT COUNT(*) FROM {t}")).fetch_one(&mut c).await.map_err(|_| bad())?;
    }
    c.close().await.ok();
    Ok(BackupContents { schema_version: schema, exams: n[0], sessions: n[1], students: n[2], attempts: n[3] })
}

/// Called at start-up *before* the database is opened. Returns true when a staged restore was applied.
pub async fn apply_pending_restore(data_dir: &Path) -> AppResult<bool> {
    let pending = data_dir.join(PENDING_FILE);
    if !pending.exists() {
        return Ok(false);
    }
    if let Err(e) = validate_backup(&pending).await {
        tracing::warn!(error = %e, "staged restore is not usable; discarding it");
        let _ = tokio::fs::remove_file(&pending).await;
        return Ok(false);
    }
    let db_path = data_dir.join(config::DB_FILE_NAME);
    let dir = backups_dir(data_dir);
    tokio::fs::create_dir_all(&dir).await?;
    if db_path.exists() {
        // Keep everything that is about to be replaced (including data still in the WAL).
        let safety = dir.join(file_name("prerestore"));
        let saved = async {
            let opts = SqliteConnectOptions::new().filename(&db_path).create_if_missing(false);
            let mut c = SqliteConnection::connect_with(&opts).await?;
            let r = snapshot_to(&mut c, &safety).await;
            c.close().await.ok();
            r
        }.await;
        if let Err(e) = saved {
            tracing::warn!(error = %e, "could not snapshot the current database; keeping the raw file instead");
            tokio::fs::rename(&db_path, dir.join(file_name("corrupt"))).await?;
        }
    }
    for ext in ["", "-wal", "-shm"] {
        let f = PathBuf::from(format!("{}{ext}", db_path.display()));
        match tokio::fs::remove_file(&f).await {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.into()),
        }
    }
    tokio::fs::rename(&pending, &db_path).await?;
    tracing::info!("restore applied");
    Ok(true)
}

pub struct BackupService {
    db: Database,
    data_dir: PathBuf,
}

impl BackupService {
    pub fn new(db: Database, data_dir: PathBuf) -> Self {
        Self { db, data_dir }
    }

    fn dir(&self) -> PathBuf {
        backups_dir(&self.data_dir)
    }

    fn info(path: &Path) -> Option<BackupInfo> {
        let name = path.file_name()?.to_str()?.to_string();
        let kind = kind_of(&name)?;
        let meta = std::fs::metadata(path).ok()?;
        let modified: DateTime<Utc> = meta.modified().ok()?.into();
        Some(BackupInfo { file_name: name, kind: kind.into(), size_bytes: meta.len(), created_at: modified.to_rfc3339(), path: path.display().to_string() })
    }

    pub async fn list(&self) -> AppResult<Vec<BackupInfo>> {
        let dir = self.dir();
        let mut out = vec![];
        let mut rd = match tokio::fs::read_dir(&dir).await {
            Ok(r) => r,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(out),
            Err(e) => return Err(e.into()),
        };
        while let Some(entry) = rd.next_entry().await? {
            if let Some(i) = Self::info(&entry.path()) { out.push(i); }
        }
        out.sort_by(|a, b| b.created_at.cmp(&a.created_at).then(b.file_name.cmp(&a.file_name)));
        Ok(out)
    }

    pub async fn auto_enabled(&self) -> AppResult<bool> {
        Ok(self.db.get_setting(SETTING_AUTO).await?.as_deref() != Some("0"))
    }

    pub async fn set_auto(&self, on: bool) -> AppResult<()> {
        self.db.set_setting(SETTING_AUTO, if on { "1" } else { "0" }).await
    }

    pub async fn overview(&self) -> AppResult<BackupOverview> {
        let pending = self.data_dir.join(PENDING_FILE);
        Ok(BackupOverview {
            directory: self.dir().display().to_string(),
            backups: self.list().await?,
            auto_enabled: self.auto_enabled().await?,
            pending_restore: if pending.exists() { validate_backup(&pending).await.ok() } else { None },
        })
    }

    /// Snapshot of the live database. `dest_dir` lets the teacher write straight to a USB drive or share.
    pub async fn create(&self, user_id: Option<&str>, kind: &str, dest_dir: Option<&str>) -> AppResult<BackupInfo> {
        let dir = match dest_dir.map(str::trim).filter(|d| !d.is_empty()) {
            Some(d) => PathBuf::from(d),
            None => self.dir(),
        };
        tokio::fs::create_dir_all(&dir).await.map_err(|_| AppError::Validation("That folder could not be used. Check that it exists and is writable.".into()))?;
        let mut path = dir.join(file_name(kind));
        // Two backups in the same second must not overwrite or refuse each other.
        let mut n = 2;
        while path.exists() {
            path = dir.join(file_name(kind).replace(".db", &format!("-{n}.db")));
            n += 1;
        }
        let mut conn = self.db.pool().acquire().await?;
        snapshot_to(&mut conn, &path).await?;
        drop(conn);
        validate_backup(&path).await.map_err(|e| AppError::Internal(format!("fresh backup failed verification: {e}")))?;
        self.db.audit(user_id, "backup.create", "backup", None, Some(&format!("{{\"kind\":\"{kind}\"}}"))).await.ok();
        Self::info(&path).ok_or_else(|| AppError::Internal("backup file missing after write".into()))
    }

    pub async fn delete(&self, user_id: Option<&str>, name: &str) -> AppResult<()> {
        safe_name(name)?;
        let path = self.dir().join(name);
        tokio::fs::remove_file(&path).await.map_err(|_| AppError::NotFound("That backup no longer exists.".into()))?;
        self.db.audit(user_id, "backup.delete", "backup", None, None).await.ok();
        Ok(())
    }

    async fn guard_no_live_session(&self) -> AppResult<()> {
        if self.db.stats().await?.active_sessions > 0 {
            return Err(AppError::Conflict("End all running sessions before restoring a backup.".into()));
        }
        Ok(())
    }

    async fn stage(&self, user_id: Option<&str>, source: &Path) -> AppResult<BackupContents> {
        self.guard_no_live_session().await?;
        let contents = validate_backup(source).await?;
        let tmp = self.data_dir.join(format!("{PENDING_FILE}.tmp"));
        tokio::fs::copy(source, &tmp).await?;
        tokio::fs::rename(&tmp, self.data_dir.join(PENDING_FILE)).await?;
        self.db.audit(user_id, "backup.stage_restore", "backup", None, None).await.ok();
        Ok(contents)
    }

    /// Restore one of the backups in the backups folder (applied at next start).
    pub async fn stage_named(&self, user_id: Option<&str>, name: &str) -> AppResult<BackupContents> {
        safe_name(name)?;
        self.stage(user_id, &self.dir().join(name)).await
    }

    /// Restore from any file the teacher points to (e.g. a USB drive).
    pub async fn stage_path(&self, user_id: Option<&str>, path: &str) -> AppResult<BackupContents> {
        let p = PathBuf::from(path.trim().trim_matches('"'));
        if !p.is_absolute() {
            return Err(AppError::Validation("Enter the full path to the backup file.".into()));
        }
        self.stage(user_id, &p).await
    }

    pub async fn cancel_pending(&self) -> AppResult<()> {
        match tokio::fs::remove_file(self.data_dir.join(PENDING_FILE)).await {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e.into()),
        }
    }

    /// Start-up routine: back up if enabled, there is data worth keeping, and the newest backup is stale.
    pub async fn auto_backup_if_due(&self) -> AppResult<Option<BackupInfo>> {
        if !self.auto_enabled().await? {
            return Ok(None);
        }
        let s = self.db.stats().await?;
        if s.exams + s.students + s.completed_attempts == 0 {
            return Ok(None);
        }
        let newest = self.list().await?.into_iter().filter(|b| b.kind == "manual" || b.kind == "auto").next();
        let due = match newest {
            None => true,
            Some(b) => DateTime::parse_from_rfc3339(&b.created_at).map(|t| Utc::now() - t.with_timezone(&Utc) >= ChronoDuration::hours(AUTO_INTERVAL_HOURS)).unwrap_or(true),
        };
        if !due {
            return Ok(None);
        }
        let made = self.create(None, "auto", None).await?;
        self.prune_auto().await?;
        Ok(Some(made))
    }

    async fn prune_auto(&self) -> AppResult<()> {
        let autos: Vec<BackupInfo> = self.list().await?.into_iter().filter(|b| b.kind == "auto").collect();
        for old in autos.into_iter().skip(KEEP_AUTO) {
            let _ = tokio::fs::remove_file(&old.path).await;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_are_recognised_and_traversal_is_refused() {
        assert_eq!(kind_of("proctorlan-manual-20261006-101010.db"), Some("manual"));
        assert_eq!(kind_of("proctorlan-prerestore-20261006-101010.db"), Some("prerestore"));
        assert_eq!(kind_of("proctorlan-weird-1.db"), None);
        assert_eq!(kind_of("notes.db"), None);
        assert!(safe_name("proctorlan-auto-1.db").is_ok());
        for bad in ["../proctorlan-auto-1.db", "a/proctorlan-auto-1.db", "..\\x", "", "proctorlan.db", "restore-pending.db"] {
            assert!(safe_name(bad).is_err(), "{bad}");
        }
    }
}
