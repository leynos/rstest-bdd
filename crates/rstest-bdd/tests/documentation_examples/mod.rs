//! Extractor for *tested living documentation* (`ExecPlan` 10.3.3 Milestone 7,
//! modelled on [`netsuke`](https://github.com/leynos/netsuke)).
//!
//! A user-facing document may carry fenced examples that the test suite can
//! execute, so the prose cannot silently rot. Each such example is introduced
//! by an HTML-comment marker that must immediately precede the fence
//! (ignoring blank lines):
//!
//! ```text
//! <!-- tested-example: scenarios-build-script -->
//! ```
//!
//! The invariants that keep the documentation honest:
//!
//! - a marker must be immediately followed by a fence (the loader errors when
//!   a marker is not followed by one);
//! - the fence must declare a language;
//! - identifiers must be non-empty and unique across the loaded documents;
//! - an **unmarked fence inside an enforced region** is a hard error, so the
//!   documentation cannot quietly acquire an untested example.
//!
//! Enforcement is deliberately **regional**, not document-wide:
//! `docs/users-guide.md` has dozens of fenced blocks and marking them all is a
//! separate sweep (queued as a follow-up roadmap item). The enforced set is
//! `(document, section-heading)` pairs; everything below each heading — until
//! the next heading of the same or higher level, or the end of the document —
//! is a region where every fence must be marked.

use eyre::{Context, ContextCompat, Result, bail};
use std::collections::HashSet;
use std::path::Path;

/// One marked fenced example loaded from a user-facing document.
pub struct DocumentedExample {
    /// Stable identifier declared by the `tested-example` marker.
    pub id: String,
    /// Markdown fence language.
    pub language: String,
    /// Exact text inside the fence, including a trailing newline.
    pub body: String,
}

/// A bounded region of a document in which every fence must be marked.
pub struct EnforcedRegion {
    /// Repository-relative document path.
    pub document: &'static str,
    /// Heading text that opens the enforced region.
    pub section: &'static str,
}

/// The regions currently under enforcement. Keep this list in sync with the
/// documents; the loaders below enforce exactly it.
fn enforced_regions() -> Vec<EnforcedRegion> {
    vec![EnforcedRegion {
        document: "docs/users-guide.md",
        section: "Feature file rebuild invalidation",
    }]
}

/// Every marked example in the enforced documents, in document order.
///
/// # Errors
///
/// Returns an error when a document cannot be read, a marker is malformed, a
/// fence inside an enforced region is unmarked or unterminated, or an
/// identifier is duplicated or empty.
pub fn load_documented_examples() -> Result<Vec<DocumentedExample>> {
    let mut collected = Vec::new();
    let mut seen = HashSet::new();
    for region in enforced_regions() {
        let document = document_path(region.document);
        let text = std::fs::read_to_string(&document)
            .wrap_err_with(|| format!("cannot read enforced document {}", document.display()))?;
        let lines: Vec<&str> = text.lines().collect();
        let (region_start, region_end) = find_region_bounds(&lines, region.section)
            .wrap_err_with(|| format!("in enforced document {}", region.document))?;
        scan_region(
            &lines,
            region_start,
            region_end,
            region.document,
            &mut collected,
            &mut seen,
        )
        .wrap_err_with(|| format!("in enforced document {}", region.document))?;
    }
    Ok(collected)
}

/// Load the documented example identified by `id`.
///
/// # Errors
///
/// Returns an error when the documents are invalid or `id` is absent.
pub fn documented_example(id: &str) -> Result<DocumentedExample> {
    let examples = load_documented_examples()?;
    examples
        .into_iter()
        .find(|example| example.id == id)
        .wrap_err_with(|| {
            "no tested-example marker named `{id}` is loaded; the enforced \
         documents may not yet carry it"
        })
}

/// Resolve a repository-relative document path from the test crate root.
fn document_path(relative: &str) -> std::path::PathBuf {
    let crate_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let Some(workspace_root) = crate_root.parent().and_then(Path::parent) else {
        panic!("crate root is two levels under the workspace root");
    };
    workspace_root.join(relative)
}

/// Index range of the region opened by `section`: from its heading line
/// (inclusive) to the next heading of the same or higher level (exclusive),
/// or the end of the document.
fn find_region_bounds(lines: &[&str], section: &str) -> Result<(usize, usize)> {
    // Level of a line, when it is a heading at 1..=6 hash depth.
    fn heading_level(line: &str) -> Option<usize> {
        let trimmed = line.trim();
        let level = trimmed.chars().take_while(|c| *c == '#').count();
        (level != 0 && level <= 6).then_some(level)
    }
    fn heading_text(line: &str) -> &str {
        line.trim().trim_start_matches('#').trim()
    }

    let headings: Vec<(usize, usize)> = lines
        .iter()
        .enumerate()
        .filter_map(|(idx, line)| heading_level(line).map(|level| (idx, level)))
        .collect();
    let (start, start_level) = headings
        .iter()
        .copied()
        .find(|(idx, _)| {
            lines
                .get(*idx)
                .is_some_and(|line| heading_text(line) == section)
        })
        .wrap_err_with(|| format!("document has no `{section}` heading to enforce"))?;
    // The boundary is the next heading of the same or higher level, whatever
    // its text.
    let end = headings
        .iter()
        .skip_while(|(idx, _)| *idx <= start)
        .find(|(_, level)| *level <= start_level)
        .map_or(lines.len(), |(idx, _)| *idx);
    Ok((start, end))
}

/// Scan `lines[region_start..region_end]`, extracting marked fenced examples
/// and hard-erroring on unmarked ones.
fn scan_region(
    lines: &[&str],
    start: usize,
    end: usize,
    document: &str,
    collected: &mut Vec<DocumentedExample>,
    seen: &mut HashSet<String>,
) -> Result<()> {
    let mut idx = start;
    while idx < end {
        let Some(line) = lines.get(idx) else {
            break;
        };
        if !is_fence(line) {
            idx += 1;
            continue;
        }
        let Some(id) = previous_marker(lines, start, idx) else {
            bail!(
                "unmarked fenced block at line {} in the enforced `{document}` \
                 region: every fence there needs a \
                 `<!-- tested-example: id -->` marker immediately before it",
                idx + 1
            );
        };
        let (example, consumed) = parse_fenced_example(lines, idx, id)?;
        if example.id.is_empty() {
            bail!(
                "empty tested-example identifier in `{document}` at line {}",
                idx + 1
            );
        }
        if !seen.insert(example.id.clone()) {
            bail!(
                "duplicate tested-example identifier `{}` in `{document}`",
                example.id
            );
        }
        collected.push(example);
        idx += consumed;
    }
    Ok(())
}

/// Whether a line opens a Markdown fence (an optional run of spaces then
/// three or more backticks or tildes).
fn is_fence(line: &str) -> bool {
    let trimmed = line.trim_start();
    trimmed.starts_with("```") || trimmed.starts_with("~~~")
}

/// The marker immediately above `fence_idx` (skipping blank lines) within the
/// region that starts at `region_start`, or `None` when no marker precedes
/// the fence.
fn previous_marker(lines: &[&str], region_start: usize, fence_idx: usize) -> Option<String> {
    let mut cursor = fence_idx;
    while cursor > region_start {
        cursor -= 1;
        let line = lines.get(cursor)?.trim();
        if line.is_empty() {
            continue;
        }
        return extract_marker_id(line);
    }
    None
}

/// Parse the identifier out of a `<!-- tested-example: ID -->` marker line.
fn extract_marker_id(line: &str) -> Option<String> {
    let line = line.trim();
    if !line.starts_with("<!-- tested-example:") {
        return None;
    }
    let (_, rest) = line.split_once("<!-- tested-example:")?;
    let rest = rest.strip_suffix("-->")?.trim();
    (!rest.is_empty()).then(|| rest.to_owned())
}

/// Parse one marked fenced block starting at `fence_idx`, returning the
/// example and how many lines it consumed (fence, body, closing fence).
fn parse_fenced_example(
    lines: &[&str],
    fence_idx: usize,
    id: String,
) -> Result<(DocumentedExample, usize)> {
    let opening = lines
        .get(fence_idx)
        .ok_or_else(|| eyre::eyre!("tested-example `{id}` fence line is missing"))?
        .trim_start();
    let language = opening
        .trim_start_matches('`')
        .trim_start_matches('~')
        .trim()
        .to_owned();
    if language.is_empty() {
        bail!("tested-example `{id}` fence declares no language");
    }
    let mut body_lines = Vec::new();
    let mut cursor = fence_idx + 1;
    loop {
        let Some(line) = lines.get(cursor) else {
            bail!("tested-example `{id}` fence is unterminated");
        };
        if is_fence(line) {
            break;
        }
        body_lines.push(*line);
        cursor += 1;
    }
    let mut body = body_lines.join("\n");
    body.push('\n');
    Ok((
        DocumentedExample { id, language, body },
        cursor - fence_idx + 1,
    ))
}

#[cfg(test)]
mod tests;
