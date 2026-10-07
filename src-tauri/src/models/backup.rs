use serde::Serialize;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct BackupInfo {
    pub file_name: String,
    /// manual | auto | prerestore | corrupt
    pub kind: String,
    pub size_bytes: u64,
    pub created_at: String,
    pub path: String,
}

/// What a backup file contains, read without modifying it.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct BackupContents {
    pub schema_version: i64,
    pub exams: i64,
    pub sessions: i64,
    pub students: i64,
    pub attempts: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupOverview {
    pub directory: String,
    pub backups: Vec<BackupInfo>,
    pub auto_enabled: bool,
    /// A restore that will be applied the next time the app starts.
    pub pending_restore: Option<BackupContents>,
}
