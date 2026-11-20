//! LaTeX → Typst math conversion for `$…$` and `$$…$$` formulas.
//!
//! Wraps the `tex2typst-rs` crate. That crate maps matrices and `cases` to
//! `mat(delim: #none, …)`, so those environments are converted here first.

use regex::Regex;
use std::sync::OnceLock;

static ENV_RE: OnceLock<Regex> = OnceLock::new();

/// Converts a LaTeX math formula (without the surrounding `$`) to Typst math.
/// Returns an error message if the formula cannot be converted.
pub fn latex_to_typst(src: &str) -> Result<String, String> {
    let re = ENV_RE.get_or_init(|| {
        Regex::new(r"(?s)\\begin\{(pmatrix|bmatrix|Bmatrix|vmatrix|Vmatrix|cases)\}(.*?)\\end\{(pmatrix|bmatrix|Bmatrix|vmatrix|Vmatrix|cases)\}").unwrap()
    });
    // Convert the supported environments to Typst directly and run the generic
    // converter on the text between them.
    let mut out = String::new();
    let mut last = 0;
    let push = |out: &mut String, piece: &str| {
        if !piece.is_empty() {
            if !out.is_empty() && !out.ends_with(' ') && !out.ends_with('(') {
                out.push(' ');
            }
            out.push_str(piece);
        }
    };
    for caps in re.captures_iter(src) {
        let whole = caps.get(0).unwrap();
        if caps[1] != caps[3] {
            return Err(format!("mismatched \\begin{{{}}} / \\end{{{}}}", &caps[1], &caps[3]));
        }
        let before = src[last..whole.start()].trim();
        if !before.is_empty() {
            push(&mut out, &generic(before)?);
        }
        push(&mut out, &environment(&caps[1], &caps[2])?);
        last = whole.end();
    }
    let tail = src[last..].trim();
    if last == 0 {
        return generic(src);
    }
    if !tail.is_empty() {
        push(&mut out, &generic(tail)?);
    }
    Ok(out)
}

fn generic(src: &str) -> Result<String, String> {
    tex2typst_rs::tex2typst(src).map_err(|e| e.to_string())
}

/// Rows are separated by `\\`, cells by `&`.
fn rows(body: &str) -> Result<Vec<Vec<String>>, String> {
    body.split("\\\\")
        .map(str::trim)
        .filter(|r| !r.is_empty())
        .map(|r| r.split('&').map(|c| latex_to_typst(c.trim())).collect())
        .collect()
}

fn environment(name: &str, body: &str) -> Result<String, String> {
    let rows = rows(body)?;
    if name == "cases" {
        let lines: Vec<String> = rows.iter().map(|r| r.join(" quad ")).collect();
        return Ok(format!("cases({})", lines.join(", ")));
    }
    let delim = match name {
        "pmatrix" => "\"(\"",
        "bmatrix" => "\"[\"",
        "Bmatrix" => "\"{\"",
        "vmatrix" => "\"|\"",
        _ => "\"||\"",
    };
    let lines: Vec<String> = rows.iter().map(|r| r.join(", ")).collect();
    Ok(format!("mat(delim: {delim}, {})", lines.join("; ")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_common_commands() {
        assert_eq!(latex_to_typst(r"\log").unwrap(), "log");
        assert_eq!(latex_to_typst(r"\frac{a}{b}").unwrap(), "a/b");
        assert_eq!(latex_to_typst(r"\sqrt{x}").unwrap(), "sqrt(x)");
        assert_eq!(latex_to_typst(r"\alpha + \beta").unwrap(), "alpha + beta");
        assert_eq!(latex_to_typst(r"\sum_{k=1}^n k").unwrap(), "sum_(k = 1)^n k");
    }

    #[test]
    fn converts_matrices_with_delimiters() {
        assert_eq!(
            latex_to_typst(r"\begin{pmatrix} 1 & 2 \\ 3 & 4 \end{pmatrix}").unwrap(),
            "mat(delim: \"(\", 1, 2; 3, 4)"
        );
        assert_eq!(
            latex_to_typst(r"\begin{bmatrix} a & b \end{bmatrix}").unwrap(),
            "mat(delim: \"[\", a, b)"
        );
    }

    #[test]
    fn converts_cases() {
        assert_eq!(
            latex_to_typst(r"f(x) = \begin{cases} 1 & x > 0 \\ 0 & \text{otherwise} \end{cases}").unwrap(),
            "f(x) = cases(1 quad x > 0, 0 quad \"otherwise\")"
        );
    }

    #[test]
    fn unbalanced_input_is_an_error() {
        assert!(latex_to_typst(r"\frac{a").is_err());
    }
}
