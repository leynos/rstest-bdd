//! Unit tests for the tested-living-documentation extractor.
//!
//! The synthetic cases pin the invariants (unmarked fence error, marker/fence
//! adjacency, duplicate and empty identifiers, unterminated fences). The two
//! integration-ish tests run against the real enforced documents, so a
//! malformed marker in `docs/users-guide.md` fails here with a named line.

use super::{
    EnforcedRegion, documented_example, extract_marker_id, find_region_bounds,
    load_documented_examples, parse_fenced_example, previous_marker, scan_region,
};
use eyre::Result;
use std::collections::HashSet;

fn lines(text: &str) -> Vec<&str> {
    text.lines().collect()
}

#[test]
fn region_runs_from_heading_to_next_same_or_higher_level() {
    let text = lines(
        "## Top\n\n### Feature file rebuild invalidation\n\nsome text\n\n```rust\nx\n```\n\n### Next\n\ntext\n",
    );
    let (start, end) = find_region_bounds(&text, "Feature file rebuild invalidation")
        .expect("the section must exist");
    assert_eq!(start, 2);
    // The region ends at the next same-or-higher-level heading (exclusive).
    assert_eq!(end, 10);
    assert_eq!(
        text.get(end..).map(|rest| rest.join("\n")),
        Some("### Next\n\ntext".to_owned())
    );
}

#[test]
fn missing_section_is_an_error() {
    let text = lines("## Top\n");
    assert!(find_region_bounds(&text, "No such section").is_err());
}

#[test]
fn marker_is_parsed() {
    assert_eq!(
        extract_marker_id("<!-- tested-example: scenarios-build-script -->"),
        Some("scenarios-build-script".to_owned())
    );
    assert_eq!(extract_marker_id("not a marker"), None);
    assert_eq!(extract_marker_id("<!-- tested-example: -->"), None);
}

#[test]
fn unmarked_fence_in_region_is_a_hard_error() {
    let text = lines("### Region\n\n```rust\nlet x = 1;\n```\n");
    let mut collected = Vec::new();
    let err = scan_region(
        &text,
        0,
        text.len(),
        "docs.md",
        &mut collected,
        &mut HashSet::new(),
    );
    assert!(err.is_err());
    let message = format!("{err:?}");
    assert!(message.contains("unmarked fenced block"), "{message}");
}

#[test]
fn marked_fence_extracts_example() {
    let text =
        lines("### Region\n\n<!-- tested-example: sample -->\n\n```rust\nfn main() {}\n```\n");
    let mut collected = Vec::new();
    scan_region(
        &text,
        0,
        text.len(),
        "docs.md",
        &mut collected,
        &mut HashSet::new(),
    )
    .expect("the marked fence must extract");
    assert_eq!(collected.len(), 1);
    let sample = collected.first().expect("one example was collected");
    assert_eq!(sample.id, "sample");
    assert_eq!(sample.language, "rust");
    assert_eq!(sample.body, "fn main() {}\n");
}

#[test]
fn marker_without_fence_is_ignored() {
    let text = lines("### Region\n\n<!-- tested-example: sample -->\n\nno fence follows\n");
    let mut collected = Vec::new();
    scan_region(
        &text,
        0,
        text.len(),
        "docs.md",
        &mut collected,
        &mut HashSet::new(),
    )
    .expect("an ignored marker must not fail the scan");
    assert!(collected.is_empty());
}

#[test]
fn duplicate_ids_across_scans_are_errors() {
    let text = lines(
        "### Region\n\n<!-- tested-example: dup -->\n```rust\na\n```\n\n<!-- tested-example: dup -->\n```rust\nb\n```\n",
    );
    let mut collected = Vec::new();
    let err = scan_region(
        &text,
        0,
        text.len(),
        "docs.md",
        &mut collected,
        &mut HashSet::new(),
    );
    assert!(err.is_err());
    assert!(format!("{err:?}").contains("duplicate tested-example identifier `dup`"));
}

#[test]
fn empty_id_is_an_error() {
    let text = lines("### Region\n\n<!-- tested-example:  -->\n```rust\na\n```\n");
    let mut collected = Vec::new();
    let err = scan_region(
        &text,
        0,
        text.len(),
        "docs.md",
        &mut collected,
        &mut HashSet::new(),
    );
    assert!(err.is_err());
}

#[test]
fn fence_without_language_is_an_error() {
    let err = parse_fenced_example(&lines("```\na\n```"), 0, "no-lang".to_owned());
    assert!(err.is_err());
}

#[test]
fn unterminated_fence_is_an_error() {
    let err = parse_fenced_example(&lines("```rust\na\n"), 0, "broken".to_owned());
    assert!(err.is_err());
}

#[test]
fn previous_marker_skips_blank_lines() {
    let text = lines("<!-- tested-example: above -->\n\n```rust\nx\n```\n");
    assert_eq!(previous_marker(&text, 0, 2), Some("above".to_owned()));
    assert_eq!(previous_marker(&text, 0, 0), None);
}

#[test]
fn users_guide_enforced_region_is_coherent() -> Result<()> {
    // Integration guard: the enforced-regions list must match the guide's
    // actual section, and the recipe marker must be loadable.
    let examples = load_documented_examples()?;
    let recipe = documented_example("scenarios-build-script")?;
    assert!(examples.iter().any(|e| e.id == "scenarios-build-script"));
    assert_eq!(recipe.language, "rust");
    assert!(recipe.body.contains("rerun-if-changed"), "{}", recipe.body);
    Ok(())
}

#[test]
fn enforced_regions_list_is_what_we_think() {
    let regions: Vec<(&str, &str)> = super::enforced_regions()
        .into_iter()
        .map(|EnforcedRegion { document, section }| (document, section))
        .collect();
    assert_eq!(
        regions,
        vec![("docs/users-guide.md", "Feature file rebuild invalidation")]
    );
}
