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
//! - a marker must be immediately followed by a fence (the loader errors when a marker is not
//!   followed by one);
//! - the fence must declare a language;
//! - identifiers must be non-empty and unique across the loaded documents;
//! - an **unmarked fence inside an enforced region** is a hard error, so the documentation cannot
//!   quietly acquire an untested example.
//!
//! Enforcement is deliberately **regional**, not document-wide:
//! `docs/users-guide.md` has dozens of fenced blocks and marking them all is a
//! separate sweep (queued as a follow-up roadmap item). The enforced set is
//! `(document, section-heading)` pairs; everything below each heading — until
//! the next heading of the same or higher level, or the end of the document —
//! is a region where every fence must be marked.

use std::{collections::HashSet, path::Path};

use eyre::{Context, ContextCompat, Result, bail};

/// One marked fenced example loaded from a user-facing document.
pub struct DocumentedExample {
    /// Stable identifier declared by the `tested-example` marker.
    pub id: ExampleId,
    /// Markdown fence language.
    pub language: FenceLanguage,
    /// Exact text inside the fence, including a trailing newline.
    pub body: String,
}

/// A bounded region of a document in which every fence must be marked.
pub struct EnforcedRegion {
    /// Repository-relative document path.
    pub document: DocumentPath,
    /// Heading text that opens the enforced region.
    pub section: SectionHeading,
}

/// A repository-relative Markdown document path.
#[derive(Clone, Copy)]
pub(super) struct DocumentPath(&'static str);

impl DocumentPath {
    fn as_str(self) -> &'static str { self.0 }
}

/// A Markdown heading that bounds an enforced document region.
#[derive(Clone, Copy)]
pub(super) struct SectionHeading(&'static str);

impl SectionHeading {
    fn as_str(self) -> &'static str { self.0 }
}

/// A `tested-example` marker identifier accepted by the document parser.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct ExampleId(String);

impl ExampleId {
    fn as_str(&self) -> &str { &self.0 }
}

/// The language label attached to a Markdown fenced code block.
#[derive(Debug, PartialEq, Eq)]
pub(super) struct FenceLanguage(String);

impl FenceLanguage {
    pub(super) fn as_str(&self) -> &str { &self.0 }
}

/// Immutable boundaries and source for one enforced document region.
struct ScanRegion<'a> {
    lines: &'a [&'a str],
    start: usize,
    end: usize,
    document: DocumentPath,
}

/// State shared by every enforced-region scan in one document load.
#[derive(Default)]
struct ScanState {
    collected: Vec<DocumentedExample>,
    seen: HashSet<String>,
}

/// The regions currently under enforcement. Keep this list in sync with the
/// documents; the loaders below enforce exactly it.
fn enforced_regions() -> Vec<EnforcedRegion> {
    vec![EnforcedRegion {
        document: DocumentPath("docs/users-guide.md"),
        section: SectionHeading("Feature file rebuild invalidation"),
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
    let mut state = ScanState::default();
    for region in enforced_regions() {
        let document = document_path(region.document);
        let text = std::fs::read_to_string(&document)
            .wrap_err_with(|| format!("cannot read enforced document {}", document.display()))?;
        let lines: Vec<&str> = text.lines().collect();
        let (region_start, region_end) = find_region_bounds(&lines, region.section)
            .wrap_err_with(|| format!("in enforced document {}", region.document.as_str()))?;
        let region = ScanRegion {
            lines: &lines,
            start: region_start,
            end: region_end,
            document: region.document,
        };
        scan_region(&region, &mut state)
            .wrap_err_with(|| format!("in enforced document {}", region.document.as_str()))?;
    }
    Ok(state.collected)
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
        .find(|example| example.id.as_str() == id)
        .wrap_err_with(|| {
            "no tested-example marker named `{id}` is loaded; the enforced documents may not yet \
             carry it"
        })
}

/// Resolve a repository-relative document path from the test crate root.
fn document_path(relative: DocumentPath) -> std::path::PathBuf {
    let crate_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let Some(workspace_root) = crate_root.parent().and_then(Path::parent) else {
        panic!("crate root is two levels under the workspace root");
    };
    workspace_root.join(relative.as_str())
}

/// Index range of the region opened by `section`: from its heading line
/// (inclusive) to the next heading of the same or higher level (exclusive),
/// or the end of the document.
fn find_region_bounds(lines: &[&str], section: SectionHeading) -> Result<(usize, usize)> {
    // Level of a line, when it is a heading at 1..=6 hash depth.
    fn heading_level(line: &str) -> Option<usize> {
        let trimmed = line.trim();
        let level = trimmed.chars().take_while(|c| *c == '#').count();
        (level != 0 && level <= 6).then_some(level)
    }
    fn heading_text(line: &str) -> &str { line.trim().trim_start_matches('#').trim() }

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
                .is_some_and(|line| heading_text(line) == section.as_str())
        })
        .wrap_err_with(|| format!("document has no `{}` heading to enforce", section.as_str()))?;
    // The boundary is the next heading of the same or higher level, whatever
    // its text.
    let end = headings
        .iter()
        .skip_while(|(idx, _)| *idx <= start)
        .find(|(_, level)| *level <= start_level)
        .map_or(lines.len(), |(idx, _)| *idx);
    Ok((start, end))
}

/// Scan `region.lines[region.start..region.end]`, extracting marked fenced examples
/// and hard-erroring on unmarked ones.
fn scan_region(region: &ScanRegion<'_>, state: &mut ScanState) -> Result<()> {
    let mut idx = region.start;
    while idx < region.end {
        let Some(line) = region.lines.get(idx) else {
            break;
        };
        if !is_fence(line) {
            idx += 1;
            continue;
        }
        let Some(id) = previous_marker(region.lines, region.start, idx) else {
            bail!(
                "unmarked fenced block at line {} in the enforced `{}` region: every fence there \
                 needs a `<!-- tested-example: id -->` marker immediately before it",
                idx + 1,
                region.document.as_str(),
            );
        };
        let (example, consumed) = parse_fenced_example(region.lines, idx, id)?;
        if example.id.as_str().is_empty() {
            bail!(
                "empty tested-example identifier in `{}` at line {}",
                region.document.as_str(),
                idx + 1,
            );
        }
        if !state.seen.insert(example.id.as_str().to_owned()) {
            bail!(
                "duplicate tested-example identifier `{}` in `{document}`",
                example.id.as_str(),
                document = region.document.as_str(),
            );
        }
        state.collected.push(example);
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
fn previous_marker(lines: &[&str], region_start: usize, fence_idx: usize) -> Option<ExampleId> {
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
fn extract_marker_id(line: &str) -> Option<ExampleId> {
    let line = line.trim();
    if !line.starts_with("<!-- tested-example:") {
        return None;
    }
    let (_, rest) = line.split_once("<!-- tested-example:")?;
    let rest = rest.strip_suffix("-->")?.trim();
    (!rest.is_empty()).then(|| ExampleId(rest.to_owned()))
}

/// Parse one marked fenced block starting at `fence_idx`, returning the
/// example and how many lines it consumed (fence, body, closing fence).
fn parse_fenced_example(
    lines: &[&str],
    fence_idx: usize,
    id: ExampleId,
) -> Result<(DocumentedExample, usize)> {
    let opening = lines
        .get(fence_idx)
        .ok_or_else(|| eyre::eyre!("tested-example `{}` fence line is missing", id.as_str()))?
        .trim_start();
    let language = opening
        .trim_start_matches('`')
        .trim_start_matches('~')
        .trim()
        .to_owned();
    if language.is_empty() {
        bail!(
            "tested-example `{}` fence declares no language",
            id.as_str()
        );
    }
    let mut body_lines = Vec::new();
    let mut cursor = fence_idx + 1;
    loop {
        let Some(line) = lines.get(cursor) else {
            bail!("tested-example `{}` fence is unterminated", id.as_str());
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
        DocumentedExample {
            id,
            language: FenceLanguage(language),
            body,
        },
        cursor - fence_idx + 1,
    ))
}

#[cfg(test)]
mod tests;
