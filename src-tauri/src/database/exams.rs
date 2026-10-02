use sqlx::{Sqlite, Transaction};

use super::{ids::*, Database};
use crate::errors::{AppError, AppResult};
use crate::models::*;

type Tx<'a> = Transaction<'a, Sqlite>;

impl Database {
    pub async fn create_exam(&self, created_by: Option<&str>, input: &NewExam) -> AppResult<String> {
        let mut tx = self.pool.begin().await?;
        let (id, ts) = (new_id(), now());
        sqlx::query(
            "INSERT INTO exams (id, title, description, instructions, duration_minutes, passing_score,
               randomize_questions, randomize_choices, allow_review, auto_submit, show_results, status, created_by, created_at, updated_at)
             VALUES (?,?,?,?,?,?,?,?,?,?,?,'inactive',?,?,?)",
        )
        .bind(&id).bind(input.title.trim()).bind(&input.description).bind(&input.instructions)
        .bind(input.duration_minutes).bind(input.passing_score)
        .bind(input.randomize_questions).bind(input.randomize_choices).bind(input.allow_review)
        .bind(input.auto_submit).bind(input.show_results).bind(created_by).bind(&ts).bind(&ts)
        .execute(&mut *tx).await?;
        insert_questions(&mut tx, &id, &input.questions, &ts).await?;
        tx.commit().await?;
        Ok(id)
    }

    /// Replaces an exam's settings and questions atomically. Refused once sessions exist,
    /// because answers reference questions and results must stay reproducible.
    pub async fn replace_exam(&self, id: &str, input: &NewExam) -> AppResult<()> {
        let mut tx = self.pool.begin().await?;
        if session_count(&mut tx, id).await? > 0 {
            return Err(AppError::Conflict("This exam already has sessions and can no longer be edited. Duplicate it to make changes.".into()));
        }
        let ts = now();
        let res = sqlx::query(
            "UPDATE exams SET title=?, description=?, instructions=?, duration_minutes=?, passing_score=?,
               randomize_questions=?, randomize_choices=?, allow_review=?, auto_submit=?, show_results=?, updated_at=? WHERE id=?",
        )
        .bind(input.title.trim()).bind(&input.description).bind(&input.instructions)
        .bind(input.duration_minutes).bind(input.passing_score)
        .bind(input.randomize_questions).bind(input.randomize_choices).bind(input.allow_review)
        .bind(input.auto_submit).bind(input.show_results).bind(&ts).bind(id)
        .execute(&mut *tx).await?;
        if res.rows_affected() == 0 {
            return Err(AppError::NotFound("Exam not found.".into()));
        }
        sqlx::query("DELETE FROM questions WHERE exam_id = ?").bind(id).execute(&mut *tx).await?;
        insert_questions(&mut tx, id, &input.questions, &ts).await?;
        tx.commit().await?;
        Ok(())
    }

    pub async fn set_exam_status(&self, id: &str, status: ExamStatus) -> AppResult<()> {
        let res = sqlx::query("UPDATE exams SET status=?, updated_at=? WHERE id=?").bind(status).bind(now()).bind(id).execute(&self.pool).await?;
        if res.rows_affected() == 0 {
            return Err(AppError::NotFound("Exam not found.".into()));
        }
        Ok(())
    }

    pub async fn delete_exam(&self, id: &str) -> AppResult<()> {
        let mut tx = self.pool.begin().await?;
        if session_count(&mut tx, id).await? > 0 {
            return Err(AppError::Conflict("This exam has session history and cannot be deleted. Mark it inactive instead.".into()));
        }
        let res = sqlx::query("DELETE FROM exams WHERE id = ?").bind(id).execute(&mut *tx).await?;
        if res.rows_affected() == 0 {
            return Err(AppError::NotFound("Exam not found.".into()));
        }
        tx.commit().await?;
        Ok(())
    }

    pub async fn duplicate_exam(&self, id: &str, created_by: Option<&str>) -> AppResult<String> {
        let full = self.get_exam_full(id).await?;
        let mut copy = NewExam::from(&full);
        copy.title = format!("{} (copy)", full.exam.title);
        self.create_exam(created_by, &copy).await
    }

    pub async fn get_exam_full(&self, id: &str) -> AppResult<ExamFull> {
        let exam: Exam = sqlx::query_as("SELECT * FROM exams WHERE id = ?").bind(id).fetch_optional(&self.pool).await?
            .ok_or_else(|| AppError::NotFound("Exam not found.".into()))?;
        let questions: Vec<Question> = sqlx::query_as("SELECT * FROM questions WHERE exam_id = ? ORDER BY sort_order").bind(id).fetch_all(&self.pool).await?;
        let choices: Vec<Choice> = sqlx::query_as(
            "SELECT c.* FROM choices c JOIN questions q ON q.id = c.question_id WHERE q.exam_id = ? ORDER BY c.sort_order",
        ).bind(id).fetch_all(&self.pool).await?;
        let questions = questions.into_iter().map(|q| {
            let own = choices.iter().filter(|c| c.question_id == q.id).cloned().collect();
            QuestionFull { question: q, choices: own }
        }).collect();
        let session_count = self.pool_session_count(id).await?;
        Ok(ExamFull { exam, questions, session_count })
    }

    async fn pool_session_count(&self, id: &str) -> AppResult<i64> {
        Ok(sqlx::query_scalar("SELECT COUNT(*) FROM exam_sessions WHERE exam_id = ?").bind(id).fetch_one(&self.pool).await?)
    }

    pub async fn list_exams(&self) -> AppResult<Vec<ExamSummary>> {
        Ok(sqlx::query_as(
            "SELECT e.*,
                    (SELECT COUNT(*) FROM questions WHERE exam_id = e.id) AS question_count,
                    (SELECT COALESCE(SUM(points), 0.0) FROM questions WHERE exam_id = e.id) AS total_points,
                    (SELECT COUNT(*) FROM exam_sessions WHERE exam_id = e.id) AS session_count
             FROM exams e ORDER BY e.updated_at DESC",
        ).fetch_all(&self.pool).await?)
    }
}

async fn session_count(tx: &mut Tx<'_>, exam_id: &str) -> AppResult<i64> {
    Ok(sqlx::query_scalar("SELECT COUNT(*) FROM exam_sessions WHERE exam_id = ?").bind(exam_id).fetch_one(&mut **tx).await?)
}

async fn insert_questions(tx: &mut Tx<'_>, exam_id: &str, questions: &[NewQuestion], ts: &str) -> AppResult<()> {
    for (qi, q) in questions.iter().enumerate() {
        let qid = new_id();
        sqlx::query(
            "INSERT INTO questions (id, exam_id, question_text, question_type, points, sort_order, required, explanation, created_at, updated_at)
             VALUES (?,?,?,?,?,?,?,?,?,?)",
        )
        .bind(&qid).bind(exam_id).bind(q.question_text.trim()).bind(q.question_type).bind(q.points)
        .bind(qi as i64).bind(q.required).bind(&q.explanation).bind(ts).bind(ts)
        .execute(&mut **tx).await?;
        for (ci, c) in q.choices.iter().enumerate() {
            sqlx::query("INSERT INTO choices (id, question_id, choice_text, is_correct, sort_order) VALUES (?,?,?,?,?)")
                .bind(new_id()).bind(&qid).bind(&c.choice_text).bind(c.is_correct).bind(ci as i64)
                .execute(&mut **tx).await?;
        }
    }
    Ok(())
}
