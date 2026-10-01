-- ProctorLAN schema v1. Timestamps: RFC3339 UTC text. Ids: UUIDv4 text. Booleans: 0/1.

CREATE TABLE users (
  id            TEXT PRIMARY KEY,
  username      TEXT NOT NULL UNIQUE COLLATE NOCASE,
  password_hash TEXT NOT NULL,
  display_name  TEXT NOT NULL,
  role          TEXT NOT NULL CHECK (role IN ('admin','teacher')),
  created_at    TEXT NOT NULL,
  updated_at    TEXT NOT NULL
);

CREATE TABLE exams (
  id                  TEXT PRIMARY KEY,
  title               TEXT NOT NULL CHECK (length(trim(title)) > 0),
  description         TEXT NOT NULL DEFAULT '',
  instructions        TEXT NOT NULL DEFAULT '',
  duration_minutes    INTEGER NOT NULL CHECK (duration_minutes > 0),
  passing_score       REAL NOT NULL CHECK (passing_score >= 0 AND passing_score <= 100),
  randomize_questions INTEGER NOT NULL DEFAULT 0 CHECK (randomize_questions IN (0,1)),
  randomize_choices   INTEGER NOT NULL DEFAULT 0 CHECK (randomize_choices IN (0,1)),
  allow_review        INTEGER NOT NULL DEFAULT 1 CHECK (allow_review IN (0,1)),
  auto_submit         INTEGER NOT NULL DEFAULT 1 CHECK (auto_submit IN (0,1)),
  show_results        INTEGER NOT NULL DEFAULT 0 CHECK (show_results IN (0,1)),
  status              TEXT NOT NULL DEFAULT 'inactive' CHECK (status IN ('active','inactive')),
  created_by          TEXT REFERENCES users(id) ON DELETE SET NULL,
  created_at          TEXT NOT NULL,
  updated_at          TEXT NOT NULL
);

CREATE TABLE questions (
  id            TEXT PRIMARY KEY,
  exam_id       TEXT NOT NULL REFERENCES exams(id) ON DELETE CASCADE,
  question_text TEXT NOT NULL CHECK (length(trim(question_text)) > 0),
  question_type TEXT NOT NULL CHECK (question_type IN ('multiple_choice','true_false','multiple_select','identification')),
  points        REAL NOT NULL CHECK (points > 0),
  sort_order    INTEGER NOT NULL,
  required      INTEGER NOT NULL DEFAULT 1 CHECK (required IN (0,1)),
  explanation   TEXT,
  created_at    TEXT NOT NULL,
  updated_at    TEXT NOT NULL
);
CREATE INDEX idx_questions_exam ON questions(exam_id, sort_order);

-- For identification questions every row is an accepted answer (is_correct = 1).
CREATE TABLE choices (
  id          TEXT PRIMARY KEY,
  question_id TEXT NOT NULL REFERENCES questions(id) ON DELETE CASCADE,
  choice_text TEXT NOT NULL,
  is_correct  INTEGER NOT NULL DEFAULT 0 CHECK (is_correct IN (0,1)),
  sort_order  INTEGER NOT NULL
);
CREATE INDEX idx_choices_question ON choices(question_id, sort_order);

-- RESTRICT: an exam with session history cannot be deleted (protects results).
CREATE TABLE exam_sessions (
  id                  TEXT PRIMARY KEY,
  exam_id             TEXT NOT NULL REFERENCES exams(id) ON DELETE RESTRICT,
  session_code        TEXT NOT NULL,
  status              TEXT NOT NULL CHECK (status IN ('CREATED','WAITING','RUNNING','PAUSED','ENDED')),
  host_ip             TEXT NOT NULL,
  host_port           INTEGER NOT NULL CHECK (host_port BETWEEN 1 AND 65535),
  started_at          TEXT,
  ends_at             TEXT,
  paused_at           TEXT,
  paused_total_seconds INTEGER NOT NULL DEFAULT 0,
  created_at          TEXT NOT NULL,
  ended_at            TEXT
);
CREATE INDEX idx_sessions_exam ON exam_sessions(exam_id);
-- A code must be unique among sessions that can still be joined; ended codes may be reused.
CREATE UNIQUE INDEX ux_sessions_open_code ON exam_sessions(session_code) WHERE status <> 'ENDED';

CREATE TABLE students (
  id             TEXT PRIMARY KEY,
  student_number TEXT NOT NULL UNIQUE,
  name           TEXT NOT NULL CHECK (length(trim(name)) > 0),
  created_at     TEXT NOT NULL
);

CREATE TABLE attempts (
  id           TEXT PRIMARY KEY,
  session_id   TEXT NOT NULL REFERENCES exam_sessions(id) ON DELETE CASCADE,
  student_id   TEXT NOT NULL REFERENCES students(id) ON DELETE CASCADE,
  status       TEXT NOT NULL CHECK (status IN ('JOINED','IN_PROGRESS','SUBMITTED','AUTO_SUBMITTED')),
  token_hash   TEXT NOT NULL UNIQUE,   -- SHA-256 of the bearer token; the token itself is never stored
  started_at   TEXT,
  submitted_at TEXT,
  score        REAL,
  total_points REAL,
  percentage   REAL,
  passed       INTEGER CHECK (passed IN (0,1)),
  created_at   TEXT NOT NULL,
  UNIQUE (session_id, student_id)       -- one attempt per student per session
);
CREATE INDEX idx_attempts_session ON attempts(session_id);

CREATE TABLE answers (
  id             TEXT PRIMARY KEY,
  attempt_id     TEXT NOT NULL REFERENCES attempts(id) ON DELETE CASCADE,
  question_id    TEXT NOT NULL REFERENCES questions(id) ON DELETE CASCADE,
  answer_data    TEXT NOT NULL,         -- JSON, e.g. ["choice-id"] or "text"
  client_seq     INTEGER NOT NULL DEFAULT 0,  -- monotonic per student; makes sync idempotent
  is_correct     INTEGER CHECK (is_correct IN (0,1)),
  points_awarded REAL,
  answered_at    TEXT NOT NULL,
  updated_at     TEXT NOT NULL,
  UNIQUE (attempt_id, question_id)
);

CREATE TABLE proctor_events (
  id          TEXT PRIMARY KEY,
  attempt_id  TEXT NOT NULL REFERENCES attempts(id) ON DELETE CASCADE,
  event_type  TEXT NOT NULL CHECK (event_type IN ('FOCUS_LOST','FOCUS_RESTORED','DISCONNECTED','RECONNECTED','SUBMISSION','TIMEOUT')),
  description TEXT NOT NULL DEFAULT '',
  metadata    TEXT,
  created_at  TEXT NOT NULL
);
CREATE INDEX idx_events_attempt ON proctor_events(attempt_id, created_at);

CREATE TABLE application_settings (
  key   TEXT PRIMARY KEY,
  value TEXT NOT NULL
);

CREATE TABLE audit_logs (
  id          TEXT PRIMARY KEY,
  user_id     TEXT REFERENCES users(id) ON DELETE SET NULL,
  action      TEXT NOT NULL,
  entity_type TEXT NOT NULL,
  entity_id   TEXT,
  metadata    TEXT,
  created_at  TEXT NOT NULL
);
CREATE INDEX idx_audit_created ON audit_logs(created_at);
