//! Docstring span detection helpers.
//!
//! This module contains functions for locating docstring delimiters
//! (`"""` or `` ``` ``) and computing their spans within feature source text.

use gherkin::Span;

use super::FeatureSource;

/// Lightweight wrapper for trimmed line content during docstring scanning.
#[derive(Clone, Copy, Debug)]
pub(super) struct LineContent<'a>(&'a str);

impl<'a> LineContent<'a> {
    pub(super) fn new(line: &'a str) -> Self {
        Self(line)
    }

    pub(super) fn as_str(&self) -> &'a str {
        self.0
    }

    pub(super) fn trim_start(&self) -> &'a str {
        self.0.trim_start()
    }
}

impl AsRef<str> for LineContent<'_> {
    fn as_ref(&self) -> &str {
        self.0
    }
}

/// Tracks the state whilst scanning for docstring delimiters.
#[derive(Debug)]
struct DocstringState {
    pending_delimiter: Option<&'static str>,
    docstring_start: usize,
}

impl DocstringState {
    fn new() -> Self {
        Self {
            pending_delimiter: None,
            docstring_start: 0,
        }
    }
}

/// Cursor position and line end boundary for docstring scanning.
#[derive(Debug, Clone, Copy)]
struct LineBounds {
    cursor: usize,
    end: usize,
}

/// Find the span of a docstring starting after `start_from`.
///
/// Scans line-by-line for opening and closing docstring delimiters
/// (`"""` or `` ``` ``), returning the span from the opening delimiter
/// line to the closing delimiter line.
pub(super) fn find_docstring_span(source: FeatureSource<'_>, start_from: usize) -> Option<Span> {
    let cursor = advance_to_next_line_boundary(source, start_from)?;

    let mut state = DocstringState::new();
    let mut current_cursor = cursor;

    while current_cursor <= source.len() {
        let (line_trimmed, line_end) = extract_line_info(source, current_cursor)?;

        if let Some(span) = process_line_for_docstring(
            line_trimmed,
            LineBounds {
                cursor: current_cursor,
                end: line_end,
            },
            &mut state,
        ) {
            return Some(span);
        }

        if line_end == source.len() {
            break;
        }
        current_cursor = line_end.saturating_add(1);
    }
    None
}

fn advance_to_next_line_boundary(source: FeatureSource<'_>, start_from: usize) -> Option<usize> {
    if start_from > source.len() {
        return None;
    }
    let mut cursor = start_from.min(source.len());
    if let Some(next_newline) = source
        .get(cursor..source.len())
        .and_then(|tail| tail.find('\n'))
    {
        cursor = cursor.saturating_add(next_newline).saturating_add(1);
    }
    Some(cursor)
}

fn extract_line_info(source: FeatureSource<'_>, cursor: usize) -> Option<(LineContent<'_>, usize)> {
    let tail = source.get(cursor..source.len())?;
    let line_end = tail
        .find('\n')
        .map_or(source.len(), |idx| cursor.saturating_add(idx));
    let line = source.get(cursor..line_end)?;
    let line_trimmed = LineContent::new(line.trim_start());
    Some((line_trimmed, line_end))
}

fn process_line_for_docstring(
    line_trimmed: LineContent<'_>,
    bounds: LineBounds,
    state: &mut DocstringState,
) -> Option<Span> {
    match state.pending_delimiter {
        None => {
            if let Some(delim) = parse_docstring_delimiter(line_trimmed) {
                state.pending_delimiter = Some(delim);
                state.docstring_start = bounds.cursor;
            }
            None
        }
        Some(delim) => {
            if matches_docstring_closing(line_trimmed, delim) {
                Some(Span {
                    start: state.docstring_start,
                    end: bounds.end,
                })
            } else {
                None
            }
        }
    }
}

fn parse_docstring_delimiter(line_trimmed: LineContent<'_>) -> Option<&'static str> {
    if line_trimmed.as_str().starts_with("\"\"\"") {
        return Some("\"\"\"");
    }
    if line_trimmed.as_str().starts_with("```") {
        return Some("```");
    }
    None
}

fn matches_docstring_closing(line_trimmed: LineContent<'_>, delim: &'static str) -> bool {
    if !line_trimmed.as_str().starts_with(delim) {
        return false;
    }
    line_trimmed
        .as_str()
        .strip_prefix(delim)
        .is_some_and(|rest| rest.trim().is_empty())
}
