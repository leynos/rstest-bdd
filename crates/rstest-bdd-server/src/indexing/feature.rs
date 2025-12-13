//! Gherkin `.feature` file indexing support.

use std::path::{Path, PathBuf};

use gherkin::{GherkinEnv, Span};

use super::{
    FeatureFileIndex, FeatureIndexError, IndexedDocstring, IndexedExampleColumn, IndexedStep,
    IndexedTable,
};

/// Parse and index a `.feature` file from disk.
///
/// The returned index uses byte offsets within the (normalised) feature text,
/// matching the behaviour of `gherkin` which appends a trailing newline when
/// missing.
///
/// # Errors
///
/// Returns an error when the feature file cannot be read or when it cannot be
/// parsed as valid Gherkin.
pub fn index_feature_file(path: &Path) -> Result<FeatureFileIndex, FeatureIndexError> {
    let mut text = std::fs::read_to_string(path)?;
    normalise_trailing_newline(&mut text);
    index_feature_text(path.to_path_buf(), &text)
}

fn index_feature_text(path: PathBuf, source: &str) -> Result<FeatureFileIndex, FeatureIndexError> {
    let feature = gherkin::Feature::parse(source, GherkinEnv::default())?;

    let mut steps = Vec::new();
    if let Some(background) = feature.background.as_ref() {
        steps.extend(index_steps_for_container(source, &background.steps)?);
    }

    for scenario in &feature.scenarios {
        steps.extend(index_steps_for_container(source, &scenario.steps)?);
    }

    for rule in &feature.rules {
        if let Some(background) = rule.background.as_ref() {
            steps.extend(index_steps_for_container(source, &background.steps)?);
        }
        for scenario in &rule.scenarios {
            steps.extend(index_steps_for_container(source, &scenario.steps)?);
        }
    }

    let example_columns = extract_example_columns(source, &feature);

    Ok(FeatureFileIndex {
        path,
        steps,
        example_columns,
    })
}

fn normalise_trailing_newline(text: &mut String) {
    if !text.ends_with('\n') {
        text.push('\n');
    }
}

fn index_steps_for_container(
    source: &str,
    steps: &[gherkin::Step],
) -> Result<Vec<IndexedStep>, FeatureIndexError> {
    let mut indexed = Vec::with_capacity(steps.len());
    for step in steps {
        let table = step.table.as_ref().map(|t| IndexedTable {
            rows: t.rows.clone(),
            span: t.span,
        });

        let docstring = match step.docstring.as_ref() {
            Some(value) => {
                let start_from = table.as_ref().map_or(step.span.end, |table| table.span.end);
                let span = find_docstring_span(source, start_from)
                    .ok_or(FeatureIndexError::DocstringSpanNotFound(step.span))?;
                Some(IndexedDocstring {
                    value: value.clone(),
                    span,
                })
            }
            None => None,
        };

        indexed.push(IndexedStep {
            keyword: step.keyword.clone(),
            step_type: step.ty,
            text: step.value.clone(),
            span: step.span,
            docstring,
            table,
        });
    }
    Ok(indexed)
}

fn extract_example_columns(source: &str, feature: &gherkin::Feature) -> Vec<IndexedExampleColumn> {
    let mut columns = Vec::new();
    for scenario in &feature.scenarios {
        collect_example_columns_for_scenario(source, &scenario.examples, &mut columns);
    }
    for rule in &feature.rules {
        for scenario in &rule.scenarios {
            collect_example_columns_for_scenario(source, &scenario.examples, &mut columns);
        }
    }
    columns
}

fn collect_example_columns_for_scenario(
    source: &str,
    examples: &[gherkin::Examples],
    columns: &mut Vec<IndexedExampleColumn>,
) {
    for ex in examples {
        let Some(table) = ex.table.as_ref() else {
            continue;
        };
        let Some(header_spans) = extract_header_cell_spans(source, table.span) else {
            continue;
        };
        let Some(header_row) = table.rows.first() else {
            continue;
        };
        for (name, span) in header_row.iter().cloned().zip(header_spans) {
            columns.push(IndexedExampleColumn { name, span });
        }
    }
}

fn extract_header_cell_spans(source: &str, table_span: Span) -> Option<Vec<Span>> {
    let table_text = source.get(table_span.start..table_span.end)?;
    let (header_line, header_line_start) = find_first_table_row_line(table_text)
        .map(|(line, offset)| (line, table_span.start + offset))?;
    Some(split_table_header_cells(header_line, header_line_start))
}

fn find_first_table_row_line(table_text: &str) -> Option<(&str, usize)> {
    let mut offset = 0usize;
    for line in table_text.split_inclusive('\n') {
        let line_no_nl = line.strip_suffix('\n').unwrap_or(line);
        let line_no_cr = line_no_nl.strip_suffix('\r').unwrap_or(line_no_nl);
        if line_no_cr.trim_start().starts_with('|') {
            return Some((line_no_cr, offset));
        }
        offset = offset.saturating_add(line.len());
    }
    None
}

fn split_table_header_cells(line: &str, global_line_start: usize) -> Vec<Span> {
    let bytes = line.as_bytes();
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

fn find_docstring_span(source: &str, start_from: usize) -> Option<Span> {
    let mut cursor = start_from.min(source.len());
    // Ensure we start scanning from the next line boundary.
    if let Some(next_newline) = source.get(cursor..).and_then(|tail| tail.find('\n')) {
        cursor = cursor.saturating_add(next_newline).saturating_add(1);
    }

    let mut pending_delimiter: Option<&'static str> = None;
    let mut docstring_start = 0usize;

    while cursor <= source.len() {
        let tail = source.get(cursor..)?;
        let line_end = tail
            .find('\n')
            .map_or(source.len(), |idx| cursor.saturating_add(idx));
        let line = source.get(cursor..line_end)?;
        let line_trimmed = line.trim_start();

        if pending_delimiter.is_none() {
            if let Some(delim) = parse_docstring_delimiter(line_trimmed) {
                pending_delimiter = Some(delim);
                docstring_start = cursor;
            }
        } else if matches_docstring_closing(line_trimmed, pending_delimiter) {
            return Some(Span {
                start: docstring_start,
                end: line_end,
            });
        }

        if line_end == source.len() {
            break;
        }
        cursor = line_end.saturating_add(1);
    }
    None
}

fn parse_docstring_delimiter(line_trimmed: &str) -> Option<&'static str> {
    if line_trimmed.starts_with("\"\"\"") {
        return Some("\"\"\"");
    }
    if line_trimmed.starts_with("```") {
        return Some("```");
    }
    None
}

fn matches_docstring_closing(line_trimmed: &str, delim: Option<&'static str>) -> bool {
    let Some(delim) = delim else {
        return false;
    };
    if !line_trimmed.starts_with(delim) {
        return false;
    }
    line_trimmed
        .strip_prefix(delim)
        .is_some_and(|rest| rest.trim().is_empty())
}

#[cfg(test)]
#[expect(
    clippy::expect_used,
    reason = "tests use explicit failures for clarity"
)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn indexes_steps_tables_docstrings_and_example_columns() {
        let dir = TempDir::new().expect("temp dir");
        let path = dir.path().join("demo.feature");

        let feature = concat!(
            "Feature: demo\n",
            "  Scenario Outline: outline\n",
            "    Given a message\n",
            "      \"\"\"\n",
            "      hello\n",
            "      \"\"\"\n",
            "    When numbers\n",
            "      | a | b |\n",
            "      | 1 | 2 |\n",
            "    Then result is <Result>\n",
            "    Examples:\n",
            "      | Result | Extra |\n",
            "      | ok     | x     |\n",
        );

        std::fs::write(&path, feature).expect("write feature file");

        let index = index_feature_file(&path).expect("index feature file");
        assert_eq!(index.steps.len(), 3);
        assert_eq!(index.example_columns.len(), 2);
        let first_column = index
            .example_columns
            .first()
            .expect("expected example columns");
        assert_eq!(first_column.name, "Result");
        let second_column = index
            .example_columns
            .get(1)
            .expect("expected second example column");
        assert_eq!(second_column.name, "Extra");

        let given = index.steps.first().expect("expected indexed steps");
        assert_eq!(given.keyword.trim(), "Given");
        assert!(given.docstring.is_some());
        let doc = given.docstring.as_ref().expect("doc string present");
        assert!(doc.span.start < doc.span.end);

        let when = index.steps.get(1).expect("expected second step");
        assert_eq!(when.keyword.trim(), "When");
        assert!(when.table.is_some());
        let table = when.table.as_ref().expect("table present");
        let first_row = table.rows.first().expect("table should have rows");
        assert_eq!(first_row, &vec!["a".to_string(), "b".to_string()]);
        assert!(table.span.start < table.span.end);
    }

    #[test]
    fn docstring_span_includes_backtick_delimiters() {
        let dir = TempDir::new().expect("temp dir");
        let path = dir.path().join("ticks.feature");
        let feature = concat!(
            "Feature: demo\n",
            "  Scenario: s\n",
            "    Given a message\n",
            "      ```\n",
            "      hello\n",
            "      ```\n",
        );
        std::fs::write(&path, feature).expect("write feature file");

        let index = index_feature_file(&path).expect("index feature file");
        let step = index.steps.first().expect("expected indexed step");
        let doc = step.docstring.as_ref().expect("doc string present");
        let doc_text = feature
            .get(doc.span.start..doc.span.end)
            .expect("doc span should be valid for source");
        assert!(doc_text.contains("```"));
    }
}
