//! Results, analytics and exports. Read-only over graded data: nothing here changes a score.

use std::collections::HashMap;
use std::path::PathBuf;

use chrono::Utc;
use serde_json::Value;

use crate::database::Database;
use crate::errors::{AppError, AppResult};
use crate::models::*;
use crate::services::csv_export as csv;

pub struct ResultsService {
    db: Database,
    exports_dir: PathBuf,
}

fn r2(v: f64) -> f64 {
    (v * 100.0).round() / 100.0
}

/// Score statistics over submitted attempts. `not_submitted` is passed in (joined but never finished).
pub fn score_stats(rows: &[ResultRow], not_submitted: i64) -> ScoreStats {
    let done: Vec<&ResultRow> = rows.iter().filter(|r| r.status.is_final()).collect();
    if done.is_empty() {
        return ScoreStats { not_submitted, ..Default::default() };
    }
    let mut pct: Vec<f64> = done.iter().map(|r| r.percentage.unwrap_or(0.0)).collect();
    pct.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let n = pct.len() as f64;
    let mean = pct.iter().sum::<f64>() / n;
    let median = if pct.len() % 2 == 1 { pct[pct.len() / 2] } else { (pct[pct.len() / 2 - 1] + pct[pct.len() / 2]) / 2.0 };
    let var = pct.iter().map(|p| (p - mean).powi(2)).sum::<f64>() / n;
    let passed = done.iter().filter(|r| r.passed == Some(true)).count() as i64;
    let times: Vec<i64> = done.iter().filter_map(|r| r.time_taken_seconds).collect();
    ScoreStats {
        submitted: done.len() as i64,
        not_submitted,
        passed,
        failed: done.len() as i64 - passed,
        pass_rate: r2(passed as f64 / n * 100.0),
        mean: r2(mean),
        median: r2(median),
        highest: r2(*pct.last().unwrap()),
        lowest: r2(pct[0]),
        std_dev: r2(var.sqrt()),
        average_time_seconds: if times.is_empty() { None } else { Some(times.iter().sum::<i64>() / times.len() as i64) },
    }
}

/// Ten ten-point buckets: 0–9, 10–19 … 90–100 (100 belongs to the last).
pub fn distribution(rows: &[ResultRow]) -> Vec<Bucket> {
    let mut b: Vec<Bucket> = (0..10u32).map(|i| {
        let (from, to) = (i * 10, if i == 9 { 100 } else { i * 10 + 9 });
        Bucket { label: format!("{from}–{to}"), from, to, count: 0 }
    }).collect();
    for r in rows.iter().filter(|r| r.status.is_final()) {
        let p = r.percentage.unwrap_or(0.0).clamp(0.0, 100.0);
        b[((p / 10.0).floor() as usize).min(9)].count += 1;
    }
    b
}

fn given_ids(answer_data: &str) -> Vec<String> {
    match serde_json::from_str::<Value>(answer_data) {
        Ok(Value::String(s)) if !s.is_empty() => vec![s],
        Ok(Value::Array(a)) => a.iter().filter_map(|v| v.as_str().map(String::from)).collect(),
        _ => vec![],
    }
}

fn given_text(answer_data: &str) -> String {
    match serde_json::from_str::<Value>(answer_data) {
        Ok(Value::String(s)) => s,
        _ => String::new(),
    }
}

fn normalise(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ").to_lowercase()
}

pub fn question_stats(exam: &ExamFull, final_attempts: &[&str], answers: &[Answer]) -> Vec<QuestionStat> {
    let n = final_attempts.len() as i64;
    let mut by_q: HashMap<&str, Vec<&Answer>> = HashMap::new();
    for a in answers.iter().filter(|a| final_attempts.contains(&a.attempt_id.as_str())) {
        by_q.entry(a.question_id.as_str()).or_default().push(a);
    }
    exam.questions.iter().enumerate().map(|(i, q)| {
        let given = by_q.get(q.question.id.as_str()).cloned().unwrap_or_default();
        let answered = given.iter().filter(|a| match q.question.question_type {
            QuestionType::Identification => !given_text(&a.answer_data).trim().is_empty(),
            _ => !given_ids(&a.answer_data).is_empty(),
        }).count() as i64;
        let correct = given.iter().filter(|a| a.is_correct == Some(true)).count() as i64;
        let mut options = vec![];
        let mut common_wrong = vec![];
        if q.question.question_type == QuestionType::Identification {
            let mut counts: HashMap<String, (String, i64)> = HashMap::new();
            for a in given.iter().filter(|a| a.is_correct != Some(true)) {
                let t = given_text(&a.answer_data);
                if t.trim().is_empty() { continue; }
                counts.entry(normalise(&t)).or_insert((t.trim().to_string(), 0)).1 += 1;
            }
            let mut v: Vec<WrongAnswer> = counts.into_values().map(|(text, count)| WrongAnswer { text, count }).collect();
            v.sort_by(|a, b| b.count.cmp(&a.count).then(a.text.cmp(&b.text)));
            v.truncate(5);
            common_wrong = v;
        } else {
            let mut picks: HashMap<String, i64> = HashMap::new();
            for a in &given {
                for id in given_ids(&a.answer_data) { *picks.entry(id).or_default() += 1; }
            }
            options = q.choices.iter().map(|c| OptionStat { text: c.choice_text.clone(), is_correct: c.is_correct, picked: picks.get(&c.id).copied().unwrap_or(0) }).collect();
        }
        QuestionStat {
            question_id: q.question.id.clone(), position: i as i64 + 1, text: q.question.question_text.clone(),
            question_type: q.question.question_type, points: q.question.points, attempts: n, answered, correct,
            percent_correct: if n > 0 { r2(correct as f64 / n as f64 * 100.0) } else { 0.0 },
            options, common_wrong,
        }
    }).collect()
}

impl ResultsService {
    pub fn new(db: Database, exports_dir: PathBuf) -> Self {
        Self { db, exports_dir }
    }

    pub async fn sessions(&self) -> AppResult<Vec<ResultSessionRow>> {
        self.db.list_result_sessions().await
    }

    pub async fn session_results(&self, session_id: &str) -> AppResult<SessionResults> {
        let session = self.db.get_session_row(session_id).await?;
        let exam = self.db.get_exam_full(&session.session.exam_id).await?;
        let rows = self.db.list_result_rows(session_id).await?;
        let answers = self.db.list_session_answers(session_id).await?;
        let final_ids: Vec<&str> = rows.iter().filter(|r| r.status.is_final()).map(|r| r.attempt_id.as_str()).collect();
        let not_submitted = rows.len() as i64 - final_ids.len() as i64;
        let questions = question_stats(&exam, &final_ids, &answers);
        Ok(SessionResults {
            session_id: session.session.id.clone(),
            exam_title: session.exam_title.clone(),
            session_code: session.session.session_code.clone(),
            status: format!("{:?}", session.session.status).to_uppercase(),
            created_at: session.session.created_at.clone(),
            ended_at: session.session.ended_at.clone(),
            passing_score: exam.exam.passing_score,
            total_points: exam.questions.iter().map(|q| q.question.points).sum(),
            question_count: exam.questions.len() as i64,
            stats: score_stats(&rows, not_submitted),
            distribution: distribution(&rows),
            rows,
            questions,
        })
    }

    pub async fn attempt_detail(&self, attempt_id: &str) -> AppResult<AttemptDetail> {
        let attempt = self.db.get_attempt(attempt_id).await?;
        let student = self.db.get_student(&attempt.student_id).await?;
        let session = self.db.get_session_row(&attempt.session_id).await?;
        let exam = self.db.get_exam_full(&session.session.exam_id).await?;
        let stored = self.db.list_answers(attempt_id).await?;
        let by_q: HashMap<&str, &Answer> = stored.iter().map(|a| (a.question_id.as_str(), a)).collect();
        let answers = exam.questions.iter().enumerate().map(|(i, q)| {
            let a = by_q.get(q.question.id.as_str());
            let choice_text = |id: &str| q.choices.iter().find(|c| c.id == id).map(|c| c.choice_text.clone()).unwrap_or_else(|| id.to_string());
            let given: Vec<String> = match (a, q.question.question_type) {
                (None, _) => vec![],
                (Some(a), QuestionType::Identification) => { let t = given_text(&a.answer_data); if t.trim().is_empty() { vec![] } else { vec![t] } }
                (Some(a), _) => given_ids(&a.answer_data).iter().map(|id| choice_text(id)).collect(),
            };
            AnswerDetail {
                position: i as i64 + 1,
                question_id: q.question.id.clone(),
                text: q.question.question_text.clone(),
                question_type: q.question.question_type,
                points: q.question.points,
                points_awarded: a.and_then(|a| a.points_awarded).unwrap_or(0.0),
                answered: !given.is_empty(),
                is_correct: a.and_then(|a| a.is_correct).unwrap_or(false),
                given,
                correct: q.choices.iter().filter(|c| c.is_correct).map(|c| c.choice_text.clone()).collect(),
                explanation: q.question.explanation.clone(),
            }
        }).collect();
        Ok(AttemptDetail {
            attempt_id: attempt.id, session_id: attempt.session_id, exam_title: session.exam_title, student_number: student.student_number,
            name: student.name, status: attempt.status, started_at: attempt.started_at, submitted_at: attempt.submitted_at,
            score: attempt.score, total_points: attempt.total_points, percentage: attempt.percentage, passed: attempt.passed,
            passing_score: exam.exam.passing_score, answers, events: self.db.list_proctor_events(attempt_id).await?,
        })
    }

    pub async fn students(&self) -> AppResult<Vec<StudentSummary>> {
        self.db.list_student_summaries().await
    }

    pub async fn student_history(&self, student_id: &str) -> AppResult<Vec<StudentAttemptRow>> {
        self.db.get_student(student_id).await?;
        self.db.list_student_attempts(student_id).await
    }

    /// The CSV text for a session: one row per student, one column per question (points earned).
    pub async fn csv(&self, session_id: &str) -> AppResult<(String, SessionResults)> {
        let res = self.session_results(session_id).await?;
        let session = self.db.get_session_row(session_id).await?;
        let exam = self.db.get_exam_full(&session.session.exam_id).await?;
        let answers = self.db.list_session_answers(session_id).await?;
        let pts: HashMap<(&str, &str), f64> = answers.iter().map(|a| ((a.attempt_id.as_str(), a.question_id.as_str()), a.points_awarded.unwrap_or(0.0))).collect();

        let mut head: Vec<String> = ["Student ID", "Name", "Status", "Score", "Total points", "Percentage", "Result", "Started", "Submitted", "Minutes taken", "Questions answered", "Times left window", "Seconds away", "Disconnects"]
            .iter().map(|s| csv::raw_cell(s)).collect();
        for (i, q) in exam.questions.iter().enumerate() {
            head.push(csv::text_cell(&format!("Q{} ({} pt)", i + 1, csv::num(q.question.points))));
        }
        let mut rows = vec![head];
        for r in &res.rows {
            let status = match r.status { AttemptStatus::Joined => "Joined", AttemptStatus::InProgress => "In progress", AttemptStatus::Submitted => "Submitted", AttemptStatus::AutoSubmitted => "Auto-submitted" };
            let result = match r.passed { Some(true) => "Passed", Some(false) => "Not passed", None => "" };
            let opt = |v: Option<f64>| v.map(csv::num).unwrap_or_default();
            let mut row = vec![
                csv::text_cell(&r.student_number), csv::text_cell(&r.name), csv::raw_cell(status),
                opt(r.score), opt(r.total_points), opt(r.percentage), csv::raw_cell(result),
                csv::text_cell(r.started_at.as_deref().unwrap_or("")), csv::text_cell(r.submitted_at.as_deref().unwrap_or("")),
                r.time_taken_seconds.map(|s| csv::num(s as f64 / 60.0)).unwrap_or_default(),
                r.answered.to_string(), r.focus_lost_count.to_string(), csv::num(r.focus_lost_ms as f64 / 1000.0), r.disconnect_count.to_string(),
            ];
            for q in &exam.questions {
                row.push(if r.status.is_final() { csv::num(pts.get(&(r.attempt_id.as_str(), q.question.id.as_str())).copied().unwrap_or(0.0)) } else { String::new() });
            }
            rows.push(row);
        }
        Ok((csv::document(rows), res))
    }

    /// Writes the CSV into the exports folder and reports where it went.
    pub async fn export_csv(&self, session_id: &str) -> AppResult<ExportInfo> {
        let (text, res) = self.csv(session_id).await?;
        tokio::fs::create_dir_all(&self.exports_dir).await?;
        let stamp = Utc::now().format("%Y%m%d-%H%M%S");
        let file_name = format!("{}-{}-{}.csv", csv::slug(&res.exam_title), res.session_code.to_lowercase(), stamp);
        let path = self.exports_dir.join(&file_name);
        tokio::fs::write(&path, text.as_bytes()).await?;
        self.db.audit(None, "results.export_csv", "session", Some(session_id), None).await.ok();
        if !path.exists() {
            return Err(AppError::Internal("export file missing after write".into()));
        }
        Ok(ExportInfo { path: path.display().to_string(), file_name, rows: res.rows.len() })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(status: AttemptStatus, pct: Option<f64>, passed: Option<bool>, secs: Option<i64>) -> ResultRow {
        ResultRow {
            attempt_id: "a".into(), student_number: "1".into(), name: "n".into(), status, started_at: None, submitted_at: None, score: None,
            total_points: None, percentage: pct, passed, answered: 0, time_taken_seconds: secs, focus_lost_count: 0, focus_lost_ms: 0, disconnect_count: 0,
        }
    }

    #[test]
    fn stats_over_submitted_only() {
        let rows = vec![
            row(AttemptStatus::Submitted, Some(100.0), Some(true), Some(600)),
            row(AttemptStatus::AutoSubmitted, Some(50.0), Some(false), Some(1200)),
            row(AttemptStatus::Submitted, Some(80.0), Some(true), None),
            row(AttemptStatus::InProgress, None, None, None),
        ];
        let s = score_stats(&rows, 1);
        assert_eq!((s.submitted, s.not_submitted, s.passed, s.failed), (3, 1, 2, 1));
        assert_eq!(s.pass_rate, 66.67);
        assert_eq!((s.mean, s.median, s.highest, s.lowest), (76.67, 80.0, 100.0, 50.0));
        assert_eq!(s.average_time_seconds, Some(900));
        assert!(s.std_dev > 20.0 && s.std_dev < 21.0);
    }

    #[test]
    fn stats_when_nobody_finished() {
        let s = score_stats(&[row(AttemptStatus::Joined, None, None, None)], 1);
        assert_eq!(s, ScoreStats { not_submitted: 1, ..Default::default() });
    }

    #[test]
    fn median_of_even_count() {
        let rows = vec![row(AttemptStatus::Submitted, Some(40.0), Some(false), None), row(AttemptStatus::Submitted, Some(60.0), Some(true), None)];
        assert_eq!(score_stats(&rows, 0).median, 50.0);
    }

    #[test]
    fn buckets_include_100_in_the_last() {
        let rows: Vec<ResultRow> = [0.0, 9.99, 10.0, 59.0, 90.0, 100.0].iter().map(|p| row(AttemptStatus::Submitted, Some(*p), Some(true), None)).collect();
        let d = distribution(&rows);
        assert_eq!(d.len(), 10);
        assert_eq!(d.iter().map(|b| b.count).collect::<Vec<_>>(), vec![2, 1, 0, 0, 0, 1, 0, 0, 0, 2]);
        assert_eq!(d[9].label, "90–100");
        assert_eq!(d.iter().map(|b| b.count).sum::<i64>(), 6);
    }

    #[test]
    fn unfinished_attempts_are_not_in_the_distribution() {
        let d = distribution(&[row(AttemptStatus::InProgress, Some(50.0), None, None)]);
        assert_eq!(d.iter().map(|b| b.count).sum::<i64>(), 0);
    }
}
