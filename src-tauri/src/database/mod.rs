//! SQLite access layer. All SQL is parameterised; multi-step writes run in transactions.

use std::{path::Path, str::FromStr, time::Duration};

use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous};
use sqlx::SqlitePool;

use crate::config;
use crate::errors::AppResult;

mod attempts;
mod exams;
pub mod ids;
mod misc;
mod results;
mod sessions;
mod users;

#[derive(Clone)]
pub struct Database {
    pool: SqlitePool,
}

impl Database {
    /// Open (creating if needed) the database file and apply pending migrations.
    pub async fn open(path: &Path) -> AppResult<Self> {
        let opts = SqliteConnectOptions::new()
            .filename(path)
            .create_if_missing(true)
            .journal_mode(SqliteJournalMode::Wal)
            .synchronous(SqliteSynchronous::Normal)
            .foreign_keys(true)
            .busy_timeout(config::DB_BUSY_TIMEOUT);
        let pool = SqlitePoolOptions::new()
            .max_connections(config::DB_MAX_CONNECTIONS)
            .connect_with(opts)
            .await?;
        Self::migrated(pool).await
    }

    /// Private in-memory database (tests). One connection only, never recycled, so the data survives.
    pub async fn open_in_memory() -> AppResult<Self> {
        let opts = SqliteConnectOptions::from_str("sqlite::memory:")?.foreign_keys(true);
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .idle_timeout(None::<Duration>)
            .max_lifetime(None::<Duration>)
            .connect_with(opts)
            .await?;
        Self::migrated(pool).await
    }

    async fn migrated(pool: SqlitePool) -> AppResult<Self> {
        sqlx::migrate!("./migrations").run(&pool).await?;
        tracing::info!("database ready, migrations applied");
        Ok(Self { pool })
    }

    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    pub async fn close(&self) {
        self.pool.close().await;
    }
}
