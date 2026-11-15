//! Helpers for extracting spans from Gherkin data tables.

use gherkin::Span;

use super::{FeatureSource, docstring::LineContent};

/// Extract byte spans for each header cell in a Gherkin Examples table.
///
/// The upstream `gherkin` AST stores the table contents but does not provide
/// per-cell spans. This helper scans the source text within the table span,
/// locates the first pipe-delimited row, trims ASCII whitespace inside each
/// cell, and returns a byte span for each header cell's content.
///
/// Returns `None` when the source slice cannot be accessed or when no header
/// row can be located.
pub(super) fn extract_header_cell_spans(
    source: FeatureSource<'_>,
    table_span: Span,
) -> Option<Vec<Span>> {
    let table_text = source.get(table_span.start..table_span.end)?;
    let (header_line, header_line_start) = find_first_table_row_line(table_text)
        .map(|(line, offset)| (line, table_span.start + offset))?;
    Some(split_table_header_cells(
        LineContent::new(header_line),
        header_line_start,
    ))
}

/// Find the first pipe-delimited row and its offset within a table slice.
fn find_first_table_row_line(table_text: &str) -> Option<(&str, usize)> {
    let mut offset = 0usize;
    for line in table_text.split_inclusive('\n') {
        let line_no_nl = line.strip_suffix('\n').unwrap_or(line);
        let line_no_cr = line_no_nl.strip_suffix('\r').unwrap_or(line_no_nl);
        if LineContent::new(line_no_cr).trim_start().starts_with('|') {
            return Some((line_no_cr, offset));
        }
        offset = offset.saturating_add(line.len());
    }
    None
}

/// Split a table header row into spans for each cell's trimmed content.
fn split_table_header_cells(line: LineContent<'_>, global_line_start: usize) -> Vec<Span> {
    let bytes = line.as_str().as_bytes();
    let pipe_positions = find_pipe_positions(bytes);
    if pipe_positions.len() < 2 {
        return Vec::new();
    }

    pipe_positions
        .windows(2)
        .filter_map(|window| match *window {
            [left, right] => Some(header_cell_span(bytes, left, right, global_line_start)),
            _ => None,
        })
        .collect()
}

/// Byte offsets of every `|` in a header row.
fn find_pipe_positions(bytes: &[u8]) -> Vec<usize> {
    bytes
        .iter()
        .enumerate()
        .filter_map(|(idx, b)| (*b == b'|').then_some(idx))
        .collect()
}

/// Span of the trimmed cell between two adjacent pipes; an empty cell collapses
/// onto the right-hand pipe.
fn header_cell_span(bytes: &[u8], left: usize, right: usize, global_line_start: usize) -> Span {
    if right <= left + 1 {
        return Span {
            start: global_line_start + right,
            end: global_line_start + right,
        };
    }
    let (trimmed_start, trimmed_end) = trim_ascii_whitespace(bytes, left + 1, right);
    Span {
        start: global_line_start + trimmed_start,
        end: global_line_start + trimmed_end,
    }
}

/// Trim ASCII spaces and tabs from a byte range.
fn trim_ascii_whitespace(bytes: &[u8], mut start: usize, mut end: usize) -> (usize, usize) {
    while start < end && bytes.get(start).is_some_and(|b| is_ascii_space(*b)) {
        start = start.saturating_add(1);
    }
    while end > start
        && bytes
            .get(end.saturating_sub(1))
            .is_some_and(|b| is_ascii_space(*b))
    {
        end = end.saturating_sub(1);
    }
    (start, end)
}

/// Return whether a byte is an ASCII table-cell whitespace character.
fn is_ascii_space(b: u8) -> bool { matches!(b, b' ' | b'\t') }
