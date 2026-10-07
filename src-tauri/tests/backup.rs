//! Backup, restore and exam file transfer against real database files.

mod common;

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use common::*;
use proctorlan_lib::database::Database;
use proctorlan_lib::models::*;
use proctorlan_lib::services::backup_service::{apply_pending_restore, validate_backup, BackupService, PENDING_FILE};
use proctorlan_lib::services::exam_service::ExamService;
use proctorlan_lib::services::exam_transfer::ExamTransfer;
use sqlx::sqlite::SqliteConnectOptions;
use sqlx::{Connection, SqliteConnection};

struct Env {
    dir: PathBuf,
    db: Database,
    user: User,
}

impl Env {
    async fn new() -> Env {
        let dir = std::env::temp_dir().join(format!("proctorlan-bk-{}", SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap().as_nanos()));
        std::fs::create_dir_all(&dir).unwrap();
        let db = Database::open(&dir.join("proctorlan.db")).await.unwrap();
        let user = db.create_user("teacher", "hash", "Teacher", UserRole::Admin).await.unwrap();
        Env { dir, db, user }
    }
    fn svc(&self) -> BackupService { BackupService::new(self.db.clone(), self.dir.clone()) }
    async fn exam(&self, title: &str) -> ExamFull {
        let mut e = four_type_exam();
        e.title = title.into();
        ExamService::new(self.db.clone()).create(&self.user, e).await.unwrap()
    }
    async fn titles(db: &Database) -> Vec<String> {
        let mut t: Vec<String> = db.list_exams().await.unwrap().into_iter().map(|e| e.exam.title).collect();
        t.sort();
        t
    }
}
impl Drop for Env {
    fn drop(&mut self) { let _ = std::fs::remove_dir_all(&self.dir); }
}

#[tokio::test]
async fn a_backup_is_a_verified_snapshot_listed_with_its_kind() {
    let env = Env::new().await;
    env.exam("Algebra").await;
    let b = env.svc().create(Some(&env.user.id), "manual", None).await.unwrap();
    assert_eq!(b.kind, "manual");
    assert!(b.size_bytes > 4096 && b.file_name.starts_with("proctorlan-manual-") && b.file_name.ends_with(".db"));
    let c = validate_backup(Path::new(&b.path)).await.unwrap();
    assert_eq!((c.exams, c.attempts), (1, 0));
    assert!(c.schema_version >= 1);
    let list = env.svc().list().await.unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].file_name, b.file_name);
}

#[tokio::test]
async fn backup_can_go_to_another_folder_such_as_a_usb_drive() {
    let env = Env::new().await;
    env.exam("Algebra").await;
    let usb = env.dir.join("usb-stick").join("nested");
    let b = env.svc().create(None, "manual", Some(usb.to_str().unwrap())).await.unwrap();
    assert!(Path::new(&b.path).starts_with(&usb));
    assert!(validate_backup(Path::new(&b.path)).await.is_ok());
    assert!(env.svc().list().await.unwrap().is_empty(), "the default folder is untouched");
}

#[tokio::test]
async fn restore_is_staged_then_applied_at_startup_and_keeps_what_it_replaces() {
    let env = Env::new().await;
    env.exam("Before").await;
    let svc = env.svc();
    let b = svc.create(None, "manual", None).await.unwrap();
    env.exam("After the backup").await;
    assert_eq!(Env::titles(&env.db).await, vec!["After the backup", "Before"]);

    let c = svc.stage_named(None, &b.file_name).await.unwrap();
    assert_eq!(c.exams, 1);
    assert!(env.dir.join(PENDING_FILE).exists());
    assert!(svc.overview().await.unwrap().pending_restore.is_some());
    assert_eq!(Env::titles(&env.db).await.len(), 2, "nothing changes until restart");

    env.db.close().await; // the app exits
    assert!(apply_pending_restore(&env.dir).await.unwrap());
    assert!(!env.dir.join(PENDING_FILE).exists());

    let reopened = Database::open(&env.dir.join("proctorlan.db")).await.unwrap();
    assert_eq!(Env::titles(&reopened).await, vec!["Before"]);

    // the replaced data is not lost: a pre-restore snapshot holds both exams
    let svc2 = BackupService::new(reopened.clone(), env.dir.clone());
    let safety = svc2.list().await.unwrap().into_iter().find(|b| b.kind == "prerestore").expect("safety backup");
    assert_eq!(validate_backup(Path::new(&safety.path)).await.unwrap().exams, 2);
    assert!(!svc2.overview().await.unwrap().pending_restore.is_some());
}

#[tokio::test]
async fn restore_from_an_arbitrary_file_path_works_and_relative_paths_do_not() {
    let env = Env::new().await;
    env.exam("Keep me").await;
    let usb = env.dir.join("usb");
    let b = env.svc().create(None, "manual", Some(usb.to_str().unwrap())).await.unwrap();
    assert!(env.svc().stage_path(None, "proctorlan-manual.db").await.is_err());
    let quoted = format!("\"{}\"", b.path); // paths pasted from Explorer often carry quotes
    assert_eq!(env.svc().stage_path(None, &quoted).await.unwrap().exams, 1);
}

#[tokio::test]
async fn restore_is_refused_while_a_session_is_live() {
    let env = Env::new().await;
    let exam = env.exam("Live").await;
    let svc = env.svc();
    let b = svc.create(None, "manual", None).await.unwrap();
    let s = env.db.create_session(&exam.exam.id, "127.0.0.1", 38123).await.unwrap();
    env.db.transition_session(&s.id, SessionStatus::Waiting).await.unwrap();
    let err = svc.stage_named(None, &b.file_name).await.unwrap_err().to_string();
    assert!(err.contains("End all running sessions"), "{err}");
    assert!(!env.dir.join(PENDING_FILE).exists());
    env.db.transition_session(&s.id, SessionStatus::Ended).await.unwrap();
    assert!(svc.stage_named(None, &b.file_name).await.is_ok(), "allowed once the session has ended");
}

#[tokio::test]
async fn damaged_foreign_and_too_new_files_are_refused() {
    let env = Env::new().await;
    env.exam("x").await;
    let good = env.svc().create(None, "manual", None).await.unwrap();

    let junk = env.dir.join("junk.db");
    std::fs::write(&junk, vec![0u8; 20_000]).unwrap();
    assert!(validate_backup(&junk).await.is_err());

    let tiny = env.dir.join("tiny.db");
    std::fs::write(&tiny, b"hello").unwrap();
    assert!(validate_backup(&tiny).await.is_err());

    let truncated = env.dir.join("truncated.db");
    let bytes = std::fs::read(&good.path).unwrap();
    std::fs::write(&truncated, &bytes[..bytes.len() / 2]).unwrap();
    assert!(validate_backup(&truncated).await.is_err());

    let other = env.dir.join("other-app.db");
    {
        let mut c = SqliteConnection::connect_with(&SqliteConnectOptions::new().filename(&other).create_if_missing(true)).await.unwrap();
        sqlx::query("CREATE TABLE t (a TEXT)").execute(&mut c).await.unwrap();
        for i in 0..600 { sqlx::query("INSERT INTO t VALUES (?)").bind(format!("row {i} {}", "x".repeat(50))).execute(&mut c).await.unwrap(); }
        c.close().await.unwrap();
    }
    assert!(validate_backup(&other).await.is_err(), "a database from another program");

    let newer = env.dir.join("newer.db");
    std::fs::copy(&good.path, &newer).unwrap();
    {
        let mut c = SqliteConnection::connect_with(&SqliteConnectOptions::new().filename(&newer)).await.unwrap();
        sqlx::query("UPDATE _sqlx_migrations SET version = 9999 WHERE version = (SELECT MAX(version) FROM _sqlx_migrations)").execute(&mut c).await.unwrap();
        c.close().await.unwrap();
    }
    let e = validate_backup(&newer).await.unwrap_err().to_string();
    assert!(e.contains("newer version"), "{e}");

    assert!(env.svc().stage_path(None, junk.to_str().unwrap()).await.is_err());
    assert!(!env.dir.join(PENDING_FILE).exists(), "a refused file is never staged");
}

#[tokio::test]
async fn an_unusable_staged_file_is_discarded_at_startup_and_the_database_is_untouched() {
    let env = Env::new().await;
    env.exam("Precious").await;
    env.db.close().await;
    std::fs::write(env.dir.join(PENDING_FILE), vec![7u8; 10_000]).unwrap();
    assert!(!apply_pending_restore(&env.dir).await.unwrap());
    assert!(!env.dir.join(PENDING_FILE).exists());
    let db = Database::open(&env.dir.join("proctorlan.db")).await.unwrap();
    assert_eq!(Env::titles(&db).await, vec!["Precious"]);
    assert!(!apply_pending_restore(&env.dir).await.unwrap(), "nothing staged: nothing to do");
}

#[tokio::test]
async fn names_cannot_escape_the_backups_folder() {
    let env = Env::new().await;
    let outside = env.dir.join("secret.txt");
    std::fs::write(&outside, "x").unwrap();
    let svc = env.svc();
    for bad in ["../secret.txt", "..\\secret.txt", "proctorlan-manual-../../secret.txt", "secret.txt", "/etc/passwd"] {
        assert!(svc.delete(None, bad).await.is_err(), "{bad}");
        assert!(svc.stage_named(None, bad).await.is_err(), "{bad}");
    }
    assert!(outside.exists());
    let b = svc.create(None, "manual", None).await.unwrap();
    svc.delete(None, &b.file_name).await.unwrap();
    assert!(svc.list().await.unwrap().is_empty());
    assert!(svc.delete(None, &b.file_name).await.is_err());
}

#[tokio::test]
async fn automatic_backups_wait_for_data_respect_the_switch_and_are_pruned() {
    let env = Env::new().await;
    let svc = env.svc();
    assert!(svc.auto_backup_if_due().await.unwrap().is_none(), "an empty database is not worth backing up");

    env.exam("Real data").await;
    svc.set_auto(false).await.unwrap();
    assert!(svc.auto_backup_if_due().await.unwrap().is_none(), "switched off");
    svc.set_auto(true).await.unwrap();

    let first = svc.auto_backup_if_due().await.unwrap().expect("due");
    assert_eq!(first.kind, "auto");
    assert!(svc.auto_backup_if_due().await.unwrap().is_none(), "not again within a day");

    // Age 12 old automatic backups (and the first) beyond the 24 h interval, then run again.
    let dir = env.dir.join("backups");
    let old = SystemTime::now() - Duration::from_secs(3 * 24 * 3600);
    for i in 0..12u64 {
        let p = dir.join(format!("proctorlan-auto-2026010{}-0000{:02}.db", 1 + i / 10, i));
        std::fs::copy(&first.path, &p).unwrap();
        std::fs::File::options().write(true).open(&p).unwrap().set_modified(old - Duration::from_secs(i * 60)).unwrap();
    }
    std::fs::File::options().write(true).open(&first.path).unwrap().set_modified(old).unwrap();
    let manual = svc.create(None, "manual", None).await.unwrap();
    std::fs::File::options().write(true).open(&manual.path).unwrap().set_modified(old).unwrap();

    let fresh = svc.auto_backup_if_due().await.unwrap().expect("stale now");
    let list = svc.list().await.unwrap();
    let autos: Vec<&BackupInfo> = list.iter().filter(|b| b.kind == "auto").collect();
    assert_eq!(autos.len(), 10, "pruned to ten");
    assert!(autos.iter().any(|b| b.file_name == fresh.file_name), "the newest survives");
    assert!(list.iter().any(|b| b.kind == "manual"), "manual backups are never pruned");
}

#[tokio::test]
async fn exams_round_trip_through_a_file() {
    let env = Env::new().await;
    let original = env.exam("Shared exam").await;
    let t = ExamTransfer::new(env.db.clone(), env.dir.join("exports"));
    let info = t.export(&original.exam.id).await.unwrap();
    assert_eq!(info.rows, 4);
    assert!(info.file_name.starts_with("exam-shared-exam-") && info.file_name.ends_with(".json"));
    let text = std::fs::read_to_string(&info.path).unwrap();
    assert!(text.contains("proctorlan-exam") && text.contains("Shakespeare"));

    let copy = t.import(&env.user, &info.path).await.unwrap();
    assert_ne!(copy.exam.id, original.exam.id);
    assert_eq!(copy.exam.title, "Shared exam");
    assert_eq!(copy.exam.status, ExamStatus::Inactive);
    assert_eq!(copy.questions.len(), 4);
    let q = copy.questions.iter().find(|q| q.question.question_text.contains("languages")).unwrap();
    assert_eq!(q.choices.iter().filter(|c| c.is_correct).count(), 2);
    assert_eq!(Env::titles(&env.db).await.len(), 2, "import adds, it never overwrites");
}

#[tokio::test]
async fn bad_exam_files_are_refused_without_creating_anything() {
    let env = Env::new().await;
    let t = ExamTransfer::new(env.db.clone(), env.dir.join("exports"));
    let write = |name: &str, body: &str| { let p = env.dir.join(name); std::fs::write(&p, body).unwrap(); p.display().to_string() };
    assert!(t.import(&env.user, &write("a.json", "not json")).await.is_err());
    assert!(t.import(&env.user, &write("b.json", "{\"format\":\"x\",\"version\":1,\"exam\":{}}")).await.is_err());
    // right format but an invalid exam (no questions is allowed as a draft; zero duration is not)
    let invalid = r#"{"format":"proctorlan-exam","version":1,"exam":{"title":"  ","durationMinutes":0,"passingScore":500}}"#;
    assert!(t.import(&env.user, &write("c.json", invalid)).await.is_err());
    assert!(t.import(&env.user, "relative.json").await.is_err());
    assert!(t.import(&env.user, &env.dir.join("missing.json").display().to_string()).await.is_err());
    let big = env.dir.join("big.json");
    std::fs::write(&big, vec![b' '; 6 * 1024 * 1024]).unwrap();
    assert!(t.import(&env.user, big.to_str().unwrap()).await.is_err());
    assert!(Env::titles(&env.db).await.is_empty());
}
