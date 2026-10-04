//! Server-side grading. The only place that ever compares an answer with the key.
//!
//! Answer JSON shapes (what clients send and what is stored):
//! - multiple_choice / true_false: a choice id string, e.g. `"c1"`
//! - multiple_select: an array of choice ids, e.g. `["c1","c3"]` (stored sorted, de-duplicated)
//! - identification: free text string
//! An empty string / empty array means "cleared" and is stored but never earns points.
//!
//! Rules: single-answer types are all-or-nothing; multiple_select is all-or-nothing too
//! (every correct choice and no wrong one) — no partial credit. Identification ignores case,
//! leading/trailing spaces and repeated inner spaces, and matches any accepted answer.

use std::collections::BTreeSet;

use serde_json::Value;

use crate::config::MAX_IDENTIFICATION_ANSWER_LEN;
use crate::models::{Choice, QuestionFull, QuestionType};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Answer {
    Single(String),
    Multi(Vec<String>),
    Text(String),
}

impl Answer {
    pub fn is_empty(&self) -> bool {
        match self {
            Answer::Single(s) | Answer::Text(s) => s.is_empty(),
            Answer::Multi(v) => v.is_empty(),
        }
    }

    /// Canonical JSON stored in the database.
    pub fn to_json(&self) -> String {
        match self {
            Answer::Single(s) | Answer::Text(s) => Value::String(s.clone()).to_string(),
            Answer::Multi(v) => serde_json::to_string(v).unwrap_or_else(|_| "[]".into()),
        }
    }
}

/// Validates the shape and the choice ids of a submitted answer. Errors are student-safe.
pub fn parse_answer(q: &QuestionFull, raw: &Value) -> Result<Answer, &'static str> {
    let valid_ids: BTreeSet<&str> = q.choices.iter().map(|c| c.id.as_str()).collect();
    match q.question.question_type {
        QuestionType::MultipleChoice | QuestionType::TrueFalse => {
            let s = raw.as_str().ok_or("Choose one option.")?;
            if s.is_empty() || valid_ids.contains(s) { Ok(Answer::Single(s.to_string())) } else { Err("That option does not belong to this question.") }
        }
        QuestionType::MultipleSelect => {
            let arr = raw.as_array().ok_or("Choose one or more options.")?;
            if arr.len() > q.choices.len() {
                return Err("Too many options selected.");
            }
            let mut set = BTreeSet::new();
            for v in arr {
                let id = v.as_str().ok_or("Choose one or more options.")?;
                if !valid_ids.contains(id) {
                    return Err("That option does not belong to this question.");
                }
                set.insert(id.to_string());
            }
            Ok(Answer::Multi(set.into_iter().collect()))
        }
        QuestionType::Identification => {
            let s = raw.as_str().ok_or("Type your answer.")?;
            if s.chars().count() > MAX_IDENTIFICATION_ANSWER_LEN {
                return Err("That answer is too long.");
            }
            if s.chars().any(|c| c.is_control() && c != '\n' && c != '\t') {
                return Err("That answer contains characters that are not allowed.");
            }
            Ok(Answer::Text(s.trim().to_string()))
        }
    }
}

fn normalise_text(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ").to_lowercase()
}

fn correct_ids(choices: &[Choice]) -> BTreeSet<&str> {
    choices.iter().filter(|c| c.is_correct).map(|c| c.id.as_str()).collect()
}

/// `(is_correct, points_awarded)` for one stored answer. Unparseable or empty answers score 0.
pub fn grade(q: &QuestionFull, stored_json: &str) -> (bool, f64) {
    let correct = match serde_json::from_str::<Value>(stored_json).ok().and_then(|v| parse_answer(q, &v).ok()) {
        None => false,
        Some(a) if a.is_empty() => false,
        Some(Answer::Single(id)) => correct_ids(&q.choices).len() == 1 && correct_ids(&q.choices).contains(id.as_str()),
        Some(Answer::Multi(ids)) => {
            let want = correct_ids(&q.choices);
            !want.is_empty() && ids.iter().map(String::as_str).collect::<BTreeSet<_>>() == want
        }
        Some(Answer::Text(t)) => {
            let given = normalise_text(&t);
            q.choices.iter().any(|c| c.is_correct && normalise_text(&c.choice_text) == given)
        }
    };
    (correct, if correct { q.question.points } else { 0.0 })
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Totals {
    pub score: f64,
    pub total_points: f64,
    pub percentage: f64,
    pub passed: bool,
}

pub fn totals(score: f64, total_points: f64, passing_score: f64) -> Totals {
    let percentage = if total_points > 0.0 { (score / total_points * 100.0 * 100.0).round() / 100.0 } else { 0.0 };
    Totals { score, total_points, percentage, passed: percentage >= passing_score }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::Question;
    use serde_json::json;

    fn q(ty: QuestionType, points: f64, choices: &[(&str, &str, bool)]) -> QuestionFull {
        QuestionFull {
            question: Question {
                id: "q".into(), exam_id: "e".into(), question_text: "?".into(), question_type: ty, points, sort_order: 0,
                required: true, explanation: None, created_at: String::new(), updated_at: String::new(),
            },
            choices: choices.iter().enumerate().map(|(i, (id, t, ok))| Choice {
                id: id.to_string(), question_id: "q".into(), choice_text: t.to_string(), is_correct: *ok, sort_order: i as i64,
            }).collect(),
        }
    }

    fn mc() -> QuestionFull { q(QuestionType::MultipleChoice, 2.0, &[("a", "A", true), ("b", "B", false), ("c", "C", false)]) }
    fn ms() -> QuestionFull { q(QuestionType::MultipleSelect, 3.0, &[("a", "A", true), ("b", "B", true), ("c", "C", false)]) }
    fn id() -> QuestionFull { q(QuestionType::Identification, 1.5, &[("x", "Paris", true), ("y", "paris france", true)]) }

    #[test]
    fn single_choice_is_all_or_nothing() {
        assert_eq!(grade(&mc(), "\"a\""), (true, 2.0));
        assert_eq!(grade(&mc(), "\"b\""), (false, 0.0));
        assert_eq!(grade(&mc(), "\"\""), (false, 0.0));
        let tf = q(QuestionType::TrueFalse, 1.0, &[("t", "True", true), ("f", "False", false)]);
        assert_eq!(grade(&tf, "\"t\""), (true, 1.0));
        assert_eq!(grade(&tf, "\"f\""), (false, 0.0));
    }

    #[test]
    fn multiple_select_needs_exactly_the_correct_set() {
        assert_eq!(grade(&ms(), r#"["a","b"]"#), (true, 3.0));
        assert_eq!(grade(&ms(), r#"["b","a"]"#), (true, 3.0), "order does not matter");
        assert_eq!(grade(&ms(), r#"["a"]"#), (false, 0.0), "missing one");
        assert_eq!(grade(&ms(), r#"["a","b","c"]"#), (false, 0.0), "extra wrong one");
        assert_eq!(grade(&ms(), "[]"), (false, 0.0));
    }

    #[test]
    fn identification_ignores_case_and_spacing_but_not_spelling() {
        assert_eq!(grade(&id(), "\"PARIS\""), (true, 1.5));
        assert_eq!(grade(&id(), "\"  paris   france \""), (true, 1.5));
        assert_eq!(grade(&id(), "\"Pariss\""), (false, 0.0));
        assert_eq!(grade(&id(), "\"\""), (false, 0.0));
    }

    #[test]
    fn malformed_stored_answers_score_zero_never_panic() {
        for bad in ["not json", "null", "42", "{}", "[1,2]", "[\"zzz\"]", "\"zzz\""] {
            assert_eq!(grade(&mc(), bad), (false, 0.0), "{bad}");
            assert_eq!(grade(&ms(), bad), (false, 0.0), "{bad}");
        }
    }

    #[test]
    fn parse_validates_shape_choice_ids_and_text() {
        assert_eq!(parse_answer(&mc(), &json!("a")).unwrap(), Answer::Single("a".into()));
        assert!(parse_answer(&mc(), &json!("nope")).is_err());
        assert!(parse_answer(&mc(), &json!(["a"])).is_err());
        assert!(parse_answer(&mc(), &json!(1)).is_err());
        assert_eq!(parse_answer(&ms(), &json!(["b", "a", "a"])).unwrap(), Answer::Multi(vec!["a".into(), "b".into()]));
        assert!(parse_answer(&ms(), &json!(["a", "zzz"])).is_err());
        assert!(parse_answer(&ms(), &json!("a")).is_err());
        assert!(parse_answer(&ms(), &json!([1])).is_err());
        assert_eq!(parse_answer(&id(), &json!("  hi  ")).unwrap(), Answer::Text("hi".into()));
        assert!(parse_answer(&id(), &json!(["x"])).is_err());
        assert!(parse_answer(&id(), &json!("a".repeat(MAX_IDENTIFICATION_ANSWER_LEN + 1))).is_err());
        assert!(parse_answer(&id(), &json!("bad\u{0007}")).is_err());
    }

    #[test]
    fn canonical_json_round_trips() {
        for a in [Answer::Single("a".into()), Answer::Multi(vec!["a".into(), "b".into()]), Answer::Text("x \"y\"".into())] {
            let q = match &a { Answer::Single(_) => mc(), Answer::Multi(_) => ms(), Answer::Text(_) => id() };
            let v: Value = serde_json::from_str(&a.to_json()).unwrap();
            assert_eq!(parse_answer(&q, &v).unwrap(), a);
        }
    }

    #[test]
    fn totals_percentage_and_pass_mark() {
        let t = totals(7.0, 10.0, 70.0);
        assert_eq!((t.percentage, t.passed), (70.0, true));
        assert!(!totals(6.99, 10.0, 70.0).passed);
        assert_eq!(totals(1.0, 3.0, 50.0).percentage, 33.33);
        let z = totals(0.0, 0.0, 70.0);
        assert_eq!((z.percentage, z.passed), (0.0, false));
    }
}
