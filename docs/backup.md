# Backup, restore and exam files (Phase 11)

## What a backup is
A single `.db` file: a consistent snapshot of the whole database (exams, sessions, students, answers, results, settings, teacher accounts). It is made with SQLite's `VACUUM INTO`, which is safe while the app is running and while students are connected. Every new backup is opened and integrity-checked before it is reported as saved.

Files are named `proctorlan-<type>-<date>-<time>.db`. Types: **manual**, **auto**, **prerestore** (your data as it was just before a restore) and **corrupt** (a database that could not be read when a restore replaced it).

> A backup contains student records and the teacher's password **hash**. Treat it as private.

## Automatic backups
On start-up, if automatic backups are on, the database has data, and the newest manual/automatic backup is older than 24 hours, one automatic backup is made. The 10 newest automatic backups are kept; manual and safety backups are never deleted automatically. A failed automatic backup is logged and never stops the app from starting.

## Restoring
1. Backups → **Restore** on a saved backup, or **Restore from file…** with a full path (USB drive, another computer).
2. The file is opened read-only and checked: integrity, ProctorLAN schema, and that it was not made by a *newer* version. Anything damaged, foreign or too new is refused and nothing is staged.
3. A restore is **staged**, not applied, and is refused while a session is waiting, running or paused.
4. On the next start — before the database is opened — the staged file is checked again, your current database is snapshotted as a *prerestore* backup (including data still in the write-ahead log), and the backup takes its place. Older backups are upgraded to the current schema automatically.

Why a restart? Replacing an open SQLite file is unreliable on Windows. Staging avoids that risk entirely. **Cancel restore** removes the staged file.

## Exam files
Exams → the download icon exports one exam to `exports/exam-<title>-<time>.json`; **Import exam** reads one back as a new, **inactive** exam (it never overwrites). Exam files contain the **answer key**. Import checks the format and version, a 5 MB size limit, and applies the normal exam validation.

## Limits and honesty
- There is no file-picker dialog yet: backup destinations and restore/import sources are typed or pasted as full paths (Windows Explorer: Shift+right-click → *Copy as path*; quotes are removed for you). A native picker needs an extra Tauri plugin and can be added later.
- "Restart now" uses Tauri's restart; if your platform ignores it, close and reopen the app.
- Backups are not encrypted. Use disk or USB encryption if the files are sensitive.
- Only the whole database can be restored; there is no "restore one exam" (use exam files for that).
