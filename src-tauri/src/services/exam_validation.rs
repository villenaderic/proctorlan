//! Single source of truth for exam rules. The builder UI calls this (via a command) for live
//! feedback; the service calls it again before saving/activating, so rules are never duplicated.
//!
//! Two levels: *save* (hard limits; drafts may be incomplete) and *ready* (everything required
//! before an exam may be activated and run).

use serde::Serialize;

use crate::config::*;
use crate::models::{NewChoice, NewExam, NewQuestion, QuestionType};

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ValidationIssue {
    /// e.g. `title`, `durationMinutes`, `questions`, `questions.2.text`, `questions.2.choices`
    pub path: String,
    pub message: String,
}

fn issue(issues: &mut Vec<ValidationIssue>, path: impl Into<String>, message: impl Into<String>) {
    issues.push(ValidationIssue { path: path.into(), message: message.into() });
}

/// Trims text and enforces type-specific invariants the UI should not have to care about.
pub fn normalize(exam: &mut NewExam) {
    exam.title = exam.title.trim().to_string();
    exam.description = exam.description.trim().to_string();
    exam.instructions = exam.instructions.trim().to_string();
    for q in &mut exam.questions {
        q.question_text = q.question_text.trim().to_string();
        q.explanation = q.explanation.take().map(|e| e.trim().to_string()).filter(|e| !e.is_empty());
        for c in &mut q.choices {
            c.choice_text = c.choice_text.trim().to_string();
        }
        match q.question_type {
            // Every row of an identification question is an accepted answer.
            QuestionType::Identification => {
                q.choices.retain(|c| !c.choice_text.is_empty());
                q.choices.iter_mut().for_each(|c| c.is_correct = true);
            }
            QuestionType::TrueFalse => {
                for c in &mut q.choices {
                    match c.choice_text.to_ascii_lowercase().as_str() {
                        "true" => c.choice_text = "True".into(),
                        "false" => c.choice_text = "False".into(),
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }
}

pub fn validate_exam(exam: &NewExam, ready: bool) -> Vec<ValidationIssue> {
    let mut v = Vec::new();

    if exam.title.trim().is_empty() {
        issue(&mut v, "title", "Title is required.");
    } else if exam.title.chars().count() > MAX_TITLE_LEN {
        issue(&mut v, "title", format!("Title must be at most {MAX_TITLE_LEN} characters."));
    }
    if exam.description.chars().count() > MAX_TEXT_LEN {
        issue(&mut v, "description", format!("Description must be at most {MAX_TEXT_LEN} characters."));
    }
    if exam.instructions.chars().count() > MAX_TEXT_LEN {
        issue(&mut v, "instructions", format!("Instructions must be at most {MAX_TEXT_LEN} characters."));
    }
    if !(1..=MAX_DURATION_MINUTES).contains(&exam.duration_minutes) {
        issue(&mut v, "durationMinutes", format!("Duration must be between 1 and {MAX_DURATION_MINUTES} minutes."));
    }
    if !exam.passing_score.is_finite() || !(0.0..=100.0).contains(&exam.passing_score) {
        issue(&mut v, "passingScore", "Passing score must be between 0 and 100.");
    }
    if exam.questions.len() > MAX_QUESTIONS_PER_EXAM {
        issue(&mut v, "questions", format!("An exam can have at most {MAX_QUESTIONS_PER_EXAM} questions."));
    }
    if ready && exam.questions.is_empty() {
        issue(&mut v, "questions", "Add at least one question.");
    }
    for (i, q) in exam.questions.iter().enumerate() {
        validate_question(&mut v, i, q, ready);
    }
    v
}

fn validate_question(v: &mut Vec<ValidationIssue>, i: usize, q: &NewQuestion, ready: bool) {
    let p = |field: &str| format!("questions.{i}.{field}");
    let n = i + 1;

    if q.question_text.trim().is_empty() {
        if ready {
            issue(v, p("text"), format!("Question {n}: enter the question text."));
        }
    } else if q.question_text.chars().count() > MAX_TEXT_LEN {
        issue(v, p("text"), format!("Question {n}: text must be at most {MAX_TEXT_LEN} characters."));
    }
    if !q.points.is_finite() || q.points <= 0.0 || q.points > MAX_POINTS {
        issue(v, p("points"), format!("Question {n}: points must be greater than 0 and at most {MAX_POINTS}."));
    }
    if q.choices.len() > MAX_CHOICES_PER_QUESTION {
        issue(v, p("choices"), format!("Question {n}: at most {MAX_CHOICES_PER_QUESTION} choices."));
    }
    if q.choices.iter().any(|c| c.choice_text.chars().count() > MAX_CHOICE_LEN) {
        issue(v, p("choices"), format!("Question {n}: a choice is longer than {MAX_CHOICE_LEN} characters."));
    }
    if !ready {
        return;
    }

    let correct = q.choices.iter().filter(|c| c.is_correct).count();
    let blank = q.choices.iter().any(|c| c.choice_text.trim().is_empty());
    match q.question_type {
        QuestionType::MultipleChoice | QuestionType::MultipleSelect => {
            let multi = q.question_type == QuestionType::MultipleSelect;
            if q.choices.len() < 2 {
                issue(v, p("choices"), format!("Question {n}: add at least two choices."));
            } else if blank {
                issue(v, p("choices"), format!("Question {n}: fill in or remove empty choices."));
            } else if has_duplicates(&q.choices) {
                issue(v, p("choices"), format!("Question {n}: choices must be different from each other."));
            }
            if correct == 0 {
                issue(v, p("correct"), format!("Question {n}: mark the correct answer."));
            } else if !multi && correct > 1 {
                issue(v, p("correct"), format!("Question {n}: multiple choice has exactly one correct answer."));
            }
        }
        QuestionType::TrueFalse => {
            let texts: Vec<&str> = q.choices.iter().map(|c| c.choice_text.as_str()).collect();
            if texts.len() != 2 || !texts.contains(&"True") || !texts.contains(&"False") {
                issue(v, p("choices"), format!("Question {n}: true/false needs the choices True and False."));
            }
            if correct != 1 {
                issue(v, p("correct"), format!("Question {n}: choose whether True or False is correct."));
            }
        }
        QuestionType::Identification => {
            if q.choices.is_empty() {
                issue(v, p("choices"), format!("Question {n}: add at least one accepted answer."));
            }
        }
    }
}

fn has_duplicates(choices: &[NewChoice]) -> bool {
    let mut seen = std::collections::HashSet::new();
    choices.iter().any(|c| !seen.insert(c.choice_text.trim().to_lowercase()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ch(t: &str, ok: bool) -> NewChoice { NewChoice { choice_text: t.into(), is_correct: ok } }
    fn q(ty: QuestionType, choices: Vec<NewChoice>) -> NewQuestion {
        NewQuestion { question_text: "Q?".into(), question_type: ty, points: 1.0, required: true, explanation: None, choices }
    }
    fn exam(questions: Vec<NewQuestion>) -> NewExam {
        NewExam { title: "T".into(), description: String::new(), instructions: String::new(), duration_minutes: 30, passing_score: 60.0,
            randomize_questions: false, randomize_choices: false, allow_review: true, auto_submit: true, show_results: false, questions }
    }
    fn good_mc() -> NewQuestion { q(QuestionType::MultipleChoice, vec![ch("A", true), ch("B", false)]) }
    fn paths(v: &[ValidationIssue]) -> Vec<&str> { v.iter().map(|i| i.path.as_str()).collect() }

    #[test]
    fn valid_exam_has_no_issues() {
        assert!(validate_exam(&exam(vec![good_mc()]), true).is_empty());
    }

    #[test]
    fn title_duration_and_passing_score_are_checked_even_for_drafts() {
        let mut e = exam(vec![]);
        e.title = "  ".into();
        e.duration_minutes = 0;
        e.passing_score = 101.0;
        assert_eq!(paths(&validate_exam(&e, false)), ["title", "durationMinutes", "passingScore"]);
        e.passing_score = f64::NAN;
        assert!(paths(&validate_exam(&e, false)).contains(&"passingScore"));
        e.duration_minutes = MAX_DURATION_MINUTES + 1;
        assert!(paths(&validate_exam(&e, false)).contains(&"durationMinutes"));
    }

    #[test]
    fn empty_exam_is_a_valid_draft_but_not_ready() {
        let e = exam(vec![]);
        assert!(validate_exam(&e, false).is_empty());
        assert_eq!(paths(&validate_exam(&e, true)), ["questions"]);
    }

    #[test]
    fn multiple_choice_rules() {
        let none = q(QuestionType::MultipleChoice, vec![ch("A", false), ch("B", false)]);
        assert!(paths(&validate_exam(&exam(vec![none]), true)).contains(&"questions.0.correct"));
        let two = q(QuestionType::MultipleChoice, vec![ch("A", true), ch("B", true)]);
        assert!(paths(&validate_exam(&exam(vec![two]), true)).contains(&"questions.0.correct"));
        let one = q(QuestionType::MultipleChoice, vec![ch("A", true)]);
        assert!(paths(&validate_exam(&exam(vec![one]), true)).contains(&"questions.0.choices"));
        let blank = q(QuestionType::MultipleChoice, vec![ch("A", true), ch("  ", false)]);
        assert!(paths(&validate_exam(&exam(vec![blank]), true)).contains(&"questions.0.choices"));
        let dup = q(QuestionType::MultipleChoice, vec![ch("Same", true), ch("same", false)]);
        assert!(paths(&validate_exam(&exam(vec![dup]), true)).contains(&"questions.0.choices"));
    }

    #[test]
    fn multiple_select_allows_several_correct_but_needs_one() {
        let ok = q(QuestionType::MultipleSelect, vec![ch("A", true), ch("B", true), ch("C", false)]);
        assert!(validate_exam(&exam(vec![ok]), true).is_empty());
        let none = q(QuestionType::MultipleSelect, vec![ch("A", false), ch("B", false)]);
        assert!(!validate_exam(&exam(vec![none]), true).is_empty());
    }

    #[test]
    fn true_false_rules() {
        let ok = q(QuestionType::TrueFalse, vec![ch("True", true), ch("False", false)]);
        assert!(validate_exam(&exam(vec![ok]), true).is_empty());
        let none = q(QuestionType::TrueFalse, vec![ch("True", false), ch("False", false)]);
        assert!(!validate_exam(&exam(vec![none]), true).is_empty());
        let wrong = q(QuestionType::TrueFalse, vec![ch("Yes", true), ch("No", false)]);
        assert!(!validate_exam(&exam(vec![wrong]), true).is_empty());
    }

    #[test]
    fn identification_needs_an_accepted_answer() {
        let none = q(QuestionType::Identification, vec![]);
        assert!(paths(&validate_exam(&exam(vec![none]), true)).contains(&"questions.0.choices"));
        let ok = q(QuestionType::Identification, vec![ch("Paris", true)]);
        assert!(validate_exam(&exam(vec![ok]), true).is_empty());
    }

    #[test]
    fn question_text_and_points() {
        let mut bad = good_mc();
        bad.question_text = " ".into();
        bad.points = 0.0;
        let issues = validate_exam(&exam(vec![bad]), true);
        assert!(paths(&issues).contains(&"questions.0.text") && paths(&issues).contains(&"questions.0.points"));
        // points are a hard rule even for drafts; blank text is not
        let mut draft = good_mc();
        draft.question_text.clear();
        assert!(validate_exam(&exam(vec![draft]), false).is_empty());
    }

    #[test]
    fn limits_are_enforced() {
        let many: Vec<_> = (0..=MAX_QUESTIONS_PER_EXAM).map(|_| good_mc()).collect();
        assert!(paths(&validate_exam(&exam(many), false)).contains(&"questions"));
        let wide = q(QuestionType::MultipleChoice, (0..=MAX_CHOICES_PER_QUESTION).map(|i| ch(&i.to_string(), i == 0)).collect());
        assert!(paths(&validate_exam(&exam(vec![wide]), false)).contains(&"questions.0.choices"));
    }

    #[test]
    fn normalize_cleans_input() {
        let mut e = exam(vec![
            q(QuestionType::Identification, vec![ch("  Paris ", false), ch("  ", false)]),
            q(QuestionType::TrueFalse, vec![ch("true", true), ch("FALSE", false)]),
        ]);
        e.title = "  Geo  ".into();
        normalize(&mut e);
        assert_eq!(e.title, "Geo");
        assert_eq!(e.questions[0].choices.len(), 1);
        assert!(e.questions[0].choices[0].is_correct);
        assert_eq!(e.questions[0].choices[0].choice_text, "Paris");
        assert_eq!(e.questions[1].choices[0].choice_text, "True");
        assert_eq!(e.questions[1].choices[1].choice_text, "False");
        assert!(validate_exam(&e, true).is_empty());
    }
}
