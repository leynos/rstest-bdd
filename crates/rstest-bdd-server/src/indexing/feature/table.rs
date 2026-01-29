//! Helpers for extracting spans from Gherkin data tables.

use gherkin::Span;

use super::FeatureSource;
use super::docstring::LineContent;

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

fn split_table_header_cells(line: LineContent<'_>, global_line_start: usize) -> Vec<Span> {
    let bytes = line.as_str().as_bytes();
    let mut pipe_positions = Vec::new();
    for (idx, b) in bytes.iter().enumerate() {
        if *b == b'|' {
            pipe_positions.push(idx);
        }
    }
    if pipe_positions.len() < 2 {
        return Vec::new();
    }

    let mut spans = Vec::with_capacity(pipe_positions.len().saturating_sub(1));
    for window in pipe_positions.windows(2) {
        let &[left, right] = window else {
            continue;
        };
        if right <= left + 1 {
            spans.push(Span {
                start: global_line_start + right,
                end: global_line_start + right,
            });
            continue;
        }
        let cell_start = left + 1;
        let cell_end = right;
        let (trimmed_start, trimmed_end) = trim_ascii_whitespace(bytes, cell_start, cell_end);
        spans.push(Span {
            start: global_line_start + trimmed_start,
            end: global_line_start + trimmed_end,
        });
    }
    spans
}

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

fn is_ascii_space(b: u8) -> bool {
    matches!(b, b' ' | b'\t')
}
