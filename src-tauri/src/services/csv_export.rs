//! CSV writing. Opens correctly in Excel (UTF-8 BOM, CRLF) and is safe against spreadsheet formula injection.

/// Quotes a field when needed and neutralises cells that a spreadsheet would run as a formula.
/// Student-controlled text (names, IDs, typed answers) must always pass through here.
pub fn text_cell(raw: &str) -> String {
    let mut s = raw.to_string();
    if matches!(s.chars().next(), Some('=' | '+' | '-' | '@' | '\t' | '\r')) {
        s.insert(0, '\'');
    }
    quote(&s)
}

/// Numbers we computed ourselves are written as-is (a leading minus is not an injection there).
pub fn raw_cell(s: &str) -> String {
    quote(s)
}

fn quote(s: &str) -> String {
    if s.contains([',', '"', '\n', '\r']) { format!("\"{}\"", s.replace('"', "\"\"")) } else { s.to_string() }
}

pub fn num(v: f64) -> String {
    let r = (v * 100.0).round() / 100.0;
    if r.fract() == 0.0 { format!("{}", r as i64) } else { format!("{r}") }
}

/// Joins already-encoded cells into the document: BOM first, CRLF between rows.
pub fn document(rows: Vec<Vec<String>>) -> String {
    let mut out = String::from('\u{FEFF}');
    for r in rows {
        out.push_str(&r.join(","));
        out.push_str("\r\n");
    }
    out
}

/// A file-system-safe name fragment.
pub fn slug(s: &str) -> String {
    let mut out = String::new();
    let mut dash = false;
    for c in s.chars() {
        if c.is_alphanumeric() { out.push(c.to_ascii_lowercase()); dash = false; }
        else if !dash && !out.is_empty() { out.push('-'); dash = true; }
    }
    let out = out.trim_end_matches('-').to_string();
    let out: String = out.chars().take(40).collect();
    if out.is_empty() { "exam".into() } else { out }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quotes_commas_quotes_and_newlines() {
        assert_eq!(text_cell("Reyes, Ana"), "\"Reyes, Ana\"");
        assert_eq!(text_cell("say \"hi\""), "\"say \"\"hi\"\"\"");
        assert_eq!(text_cell("a\nb"), "\"a\nb\"");
        assert_eq!(text_cell("plain"), "plain");
    }

    #[test]
    fn neutralises_formulas() {
        for evil in ["=1+1", "+cmd", "-2+3", "@SUM(A1)", "\tx"] {
            assert!(text_cell(evil).starts_with('\''), "{evil}");
        }
        assert_eq!(text_cell("=HYPERLINK(\"x\",\"y\")"), "\"'=HYPERLINK(\"\"x\"\",\"\"y\"\")\"");
        assert_eq!(text_cell("Ana-Marie"), "Ana-Marie");
    }

    #[test]
    fn numbers_and_document() {
        assert_eq!(num(80.0), "80");
        assert_eq!(num(66.666), "66.67");
        assert_eq!(num(1.5), "1.5");
        let d = document(vec![vec!["a".into(), "b".into()], vec!["1".into(), "2".into()]]);
        assert_eq!(d, "\u{FEFF}a,b\r\n1,2\r\n");
    }

    #[test]
    fn slugs_are_safe() {
        assert_eq!(slug("Midterm: CS 101 / Sec A!"), "midterm-cs-101-sec-a");
        assert_eq!(slug("../../etc/passwd"), "etc-passwd");
        assert_eq!(slug("???"), "exam");
        assert!(slug(&"x".repeat(200)).len() <= 40);
    }
}
