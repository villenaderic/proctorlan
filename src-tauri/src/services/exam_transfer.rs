//! Exam files: export an exam to JSON and import one back (to share exams or move them between computers).
//! An exported file contains the answer key, so it must be treated like the exam itself.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::database::Database;
use crate::errors::{AppError, AppResult};
use crate::models::*;
use crate::services::csv_export::slug;
use crate::services::exam_service::ExamService;

pub const FORMAT: &str = "proctorlan-exam";
pub const VERSION: u32 = 1;
pub const MAX_IMPORT_BYTES: u64 = 5 * 1024 * 1024;

#[derive(Serialize, Deserialize)]
struct ExamFile {
    format: String,
    version: u32,
    exam: NewExam,
}

pub struct ExamTransfer {
    db: Database,
    exams: ExamService,
    exports_dir: PathBuf,
}

/// Parses and checks an exam file's text. Pure, so it is easy to test with hostile input.
pub fn parse_exam_file(text: &str) -> AppResult<NewExam> {
    let f: ExamFile = serde_json::from_str(text.trim_start_matches('\u{FEFF}'))
        .map_err(|_| AppError::Validation("That file is not a ProctorLAN exam file.".into()))?;
    if f.format != FORMAT {
        return Err(AppError::Validation("That file is not a ProctorLAN exam file.".into()));
    }
    if f.version > VERSION {
        return Err(AppError::Validation("That exam file was made by a newer version of ProctorLAN. Update the app first.".into()));
    }
    Ok(f.exam)
}

impl ExamTransfer {
    pub fn new(db: Database, exports_dir: PathBuf) -> Self {
        Self { exams: ExamService::new(db.clone()), db, exports_dir }
    }

    pub async fn export(&self, id: &str) -> AppResult<ExportInfo> {
        let full = self.db.get_exam_full(id).await?;
        let file = ExamFile { format: FORMAT.into(), version: VERSION, exam: NewExam::from(&full) };
        let text = serde_json::to_string_pretty(&file).map_err(|e| AppError::Internal(e.to_string()))?;
        tokio::fs::create_dir_all(&self.exports_dir).await?;
        let stamp = chrono::Utc::now().format("%Y%m%d-%H%M%S");
        let file_name = format!("exam-{}-{}.json", slug(&full.exam.title), stamp);
        let path = self.exports_dir.join(&file_name);
        tokio::fs::write(&path, text).await?;
        Ok(ExportInfo { path: path.display().to_string(), file_name, rows: full.questions.len() })
    }

    /// Creates a new, inactive exam from a file. Never overwrites an existing exam.
    pub async fn import(&self, user: &User, path: &str) -> AppResult<ExamFull> {
        let p = PathBuf::from(path.trim().trim_matches('"'));
        if !p.is_absolute() {
            return Err(AppError::Validation("Enter the full path to the exam file.".into()));
        }
        let text = read_limited(&p).await?;
        let exam = parse_exam_file(&text)?;
        self.exams.create(user, exam).await
    }
}

async fn read_limited(p: &Path) -> AppResult<String> {
    let meta = tokio::fs::metadata(p).await.map_err(|_| AppError::NotFound("That file was not found.".into()))?;
    if !meta.is_file() || meta.len() > MAX_IMPORT_BYTES {
        return Err(AppError::Validation("That file is not a usable exam file (missing or larger than 5 MB).".into()));
    }
    let bytes = tokio::fs::read(p).await?;
    String::from_utf8(bytes).map_err(|_| AppError::Validation("That file is not a ProctorLAN exam file.".into()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_foreign_and_newer_files() {
        assert!(parse_exam_file("not json").is_err());
        assert!(parse_exam_file("{\"format\":\"other\",\"version\":1,\"exam\":{}}").is_err());
        let newer = r#"{"format":"proctorlan-exam","version":99,"exam":{"title":"x","durationMinutes":5,"passingScore":50}}"#;
        assert!(parse_exam_file(newer).is_err());
        let ok = r#"{"format":"proctorlan-exam","version":1,"exam":{"title":"x","durationMinutes":5,"passingScore":50}}"#;
        assert_eq!(parse_exam_file(ok).unwrap().title, "x");
        assert!(parse_exam_file(&format!("\u{FEFF}{ok}")).is_ok());
    }
}
