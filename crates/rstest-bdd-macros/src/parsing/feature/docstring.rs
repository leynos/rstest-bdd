//! Media-type annotations on docstrings.
//!
//! Gherkin lets a docstring name its media type after the opening delimiter,
//! as in `"""json`. `gherkin` 0.16 does not recognize the annotation: it keeps
//! the token as the first content line and then dedents, and because the
//! token sits at column zero nothing is dedented, so the step receives
//! `json` followed by the body with its source indentation.
//!
//! [`strip_media_type`] removes such a first line and re-dedents the rest.
//! Without an annotation `gherkin` yields a docstring that starts with the
//! newline ending the delimiter line; the stripped form starts with the same
//! newline, so the leading and trailing newlines that existing steps rely on
//! are unchanged.

/// Return `docstring` without a leading media-type line, re-dedented.
///
/// A docstring is only rewritten when its first line, which can only be
/// non-empty when text follows the opening delimiter, is a single media-type
/// token such as `json`, `text/plain`, or `application/vnd.api+json`.
///
/// # Examples
///
/// ```rust,ignore
/// let parsed = "json\n      {\"a\": 1}\n";
/// assert_eq!(strip_media_type(parsed), "\n{\"a\": 1}\n");
/// assert_eq!(strip_media_type("\nplain\n"), "\nplain\n");
/// ```
pub(crate) fn strip_media_type(docstring: &str) -> String {
    match docstring.split_once('\n') {
        Some((first, rest)) if is_media_type(first) => format!("\n{}", dedent(rest)),
        None if is_media_type(docstring) => String::new(),
        _ => docstring.to_owned(),
    }
}

/// Return whether a line consists of a single media-type token.
fn is_media_type(line: &str) -> bool {
    let token = line.trim();
    token
        .chars()
        .next()
        .is_some_and(|first| first.is_ascii_alphanumeric())
        && token
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '.' | '+' | '-' | '_' | '/'))
}

/// Remove the whitespace prefix shared by every non-blank line, and empty
/// whitespace-only lines, as `gherkin` does when it dedents a docstring.
fn dedent(text: &str) -> String {
    let prefix = text
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(leading_whitespace)
        .reduce(common_prefix)
        .unwrap_or("");
    text.split('\n')
        .map(|line| {
            if line.trim().is_empty() {
                ""
            } else {
                line.strip_prefix(prefix).unwrap_or(line)
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// The leading run of whitespace in `line`.
fn leading_whitespace(line: &str) -> &str {
    let content_start = line.len() - line.trim_start().len();
    line.get(..content_start).unwrap_or("")
}

/// The longest shared prefix of two whitespace runs.
fn common_prefix<'a>(lhs: &'a str, rhs: &str) -> &'a str {
    let shared = lhs
        .char_indices()
        .zip(rhs.chars())
        .find(|((_, left), right)| left != right)
        .map_or_else(|| lhs.len().min(rhs.len()), |((idx, _), _)| idx);
    lhs.get(..shared).unwrap_or("")
}

#[cfg(test)]
mod tests {
    //! Unit tests for docstring media-type stripping.

    use gherkin::{Feature, GherkinEnv};
    use rstest::rstest;

    use super::*;

    /// Parse a one-step feature and return the step's raw `gherkin` docstring.
    fn parsed_docstring(delimiter_line: &str) -> String {
        let source = format!(
                "Feature: f\n  Scenario: s\n    Given a step\n      {delimiter_line}\n        \
                 {{\n          \"a\": 1\n        }}\n\n      \"\"\"\n"
            );
        let feature = Feature::parse(&source, GherkinEnv::default()).expect("feature should parse");
        feature
            .scenarios
            .first()
            .and_then(|scenario| scenario.steps.first())
            .and_then(|step| step.docstring.clone())
            .expect("step should carry a docstring")
    }

    #[rstest]
    #[case::json("\"\"\"json")]
    #[case::mime("\"\"\"application/vnd.api+json")]
    fn annotated_docstrings_match_bare_ones(#[case] delimiter_line: &str) {
        let bare = parsed_docstring("\"\"\"");
        assert!(bare.starts_with('\n') && bare.ends_with('\n'), "{bare:?}");
        assert_eq!(strip_media_type(&parsed_docstring(delimiter_line)), bare);
    }

    #[test]
    fn bare_docstrings_are_unchanged() {
        let bare = parsed_docstring("\"\"\"");
        assert_eq!(strip_media_type(&bare), bare);
    }

    #[rstest]
    #[case::prose("not a token\n  body\n")]
    #[case::punctuated("{\"inline\": true}\n")]
    fn other_first_lines_are_kept(#[case] docstring: &str) {
        assert_eq!(strip_media_type(docstring), docstring);
    }

    #[test]
    fn annotation_without_body_becomes_empty() {
        assert_eq!(strip_media_type("json"), "");
    }

    #[test]
    fn dedent_keeps_relative_indentation() {
        assert_eq!(dedent("    a\n      b\n  \n    c"), "a\n  b\n\nc");
    }
}
