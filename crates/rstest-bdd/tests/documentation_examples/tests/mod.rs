//! Unit tests for the tested-living-documentation extractor.
//!
//! The synthetic cases pin the invariants (unmarked fence error, marker/fence
//! adjacency, duplicate and empty identifiers, unterminated fences). The two
//! integration-ish tests run against the real enforced documents, so a
//! malformed marker in `docs/users-guide.md` fails here with a named line.

use eyre::Result;

use super::{
    DocumentPath,
    EnforcedRegion,
    ExampleId,
    ScanRegion,
    ScanState,
    SectionHeading,
    documented_example,
    extract_marker_id,
    find_region_bounds,
    load_documented_examples,
    parse_fenced_example,
    scan_region,
};

fn lines(text: &str) -> Vec<&str> { text.lines().collect() }

#[test]
fn region_runs_from_heading_to_next_same_or_higher_level() {
    let text = lines(
        "## Top\n\n### Feature file rebuild invalidation\n\nsome text\n\n```rust\nx\n```\n\n### \
         Next\n\ntext\n",
    );
    let (start, end) =
        find_region_bounds(&text, SectionHeading("Feature file rebuild invalidation"))
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
    assert!(find_region_bounds(&text, SectionHeading("No such section")).is_err());
}

#[test]
fn marker_is_parsed() {
    assert_eq!(
        extract_marker_id("<!-- tested-example: scenarios-build-script -->"),
        Some(ExampleId("scenarios-build-script".to_owned()))
    );
    assert_eq!(extract_marker_id("not a marker"), None);
    assert_eq!(extract_marker_id("<!-- tested-example: -->"), None);
}

#[test]
fn unmarked_fence_in_region_is_a_hard_error() {
    let text = lines("### Region\n\n```rust\nlet x = 1;\n```\n");
    let region = ScanRegion {
        lines: &text,
        start: 0,
        end: text.len(),
        document: DocumentPath("docs.md"),
    };
    let err = scan_region(&region, &mut ScanState::default());
    assert!(err.is_err());
    let message = format!("{err:?}");
    assert!(message.contains("unmarked fenced block"), "{message}");
}

#[test]
fn marked_fence_extracts_example() {
    let text =
        lines("### Region\n\n<!-- tested-example: sample -->\n\n```rust\nfn main() {}\n```\n");
    let region = ScanRegion {
        lines: &text,
        start: 0,
        end: text.len(),
        document: DocumentPath("docs.md"),
    };
    let mut state = ScanState::default();
    scan_region(&region, &mut state).expect("the marked fence must extract");
    assert_eq!(state.collected.len(), 1);
    let sample = state.collected.first().expect("one example was collected");
    assert_eq!(sample.id.as_str(), "sample");
    assert_eq!(sample.language.as_str(), "rust");
    assert_eq!(sample.body, "fn main() {}\n");
}

#[test]
fn marker_without_fence_is_rejected() {
    let text = lines("### Region\n\n<!-- tested-example: sample -->\n\nno fence follows\n");
    let region = ScanRegion {
        lines: &text,
        start: 0,
        end: text.len(),
        document: DocumentPath("docs.md"),
    };
    let error = scan_region(&region, &mut ScanState::default())
        .expect_err("a marker without a fenced example must fail the scan");
    assert!(format!("{error:?}").contains("is not followed by a fenced block"));
}

#[test]
fn duplicate_ids_across_scans_are_errors() {
    let first = lines("### Region\n\n<!-- tested-example: dup -->\n```rust\na\n```\n");
    let second = lines("### Region\n\n<!-- tested-example: dup -->\n```rust\nb\n```\n");
    let first_region = ScanRegion {
        lines: &first,
        start: 0,
        end: first.len(),
        document: DocumentPath("first.md"),
    };
    let second_region = ScanRegion {
        lines: &second,
        start: 0,
        end: second.len(),
        document: DocumentPath("second.md"),
    };
    let mut state = ScanState::default();
    scan_region(&first_region, &mut state).expect("the first identifier is unique");
    let err = scan_region(&second_region, &mut state);
    assert!(err.is_err());
    assert!(format!("{err:?}").contains("duplicate tested-example identifier `dup`"));
}

#[test]
fn empty_id_is_an_error() {
    let text = lines("### Region\n\n<!-- tested-example:  -->\n```rust\na\n```\n");
    let region = ScanRegion {
        lines: &text,
        start: 0,
        end: text.len(),
        document: DocumentPath("docs.md"),
    };
    let err = scan_region(&region, &mut ScanState::default());
    assert!(err.is_err());
}

#[test]
fn fence_without_language_is_an_error() {
    let err = parse_fenced_example(&lines("```\na\n```"), 0, ExampleId("no-lang".to_owned()));
    assert!(err.is_err());
}

#[test]
fn unterminated_fence_is_an_error() {
    let err = parse_fenced_example(&lines("```rust\na\n"), 0, ExampleId("broken".to_owned()));
    assert!(err.is_err());
}

#[test]
fn mismatched_fence_delimiter_is_unterminated() {
    let result = parse_fenced_example(
        &lines("```rust\na\n~~~\n"),
        0,
        ExampleId("mismatched".to_owned()),
    );
    let error = result
        .err()
        .expect("mismatched fences must be unterminated");
    assert!(format!("{error:?}").contains("unterminated"));
}

#[test]
fn shorter_closing_fence_is_unterminated() {
    let result = parse_fenced_example(
        &lines("````rust\na\n```\n"),
        0,
        ExampleId("short".to_owned()),
    );
    let error = result.err().expect("shorter fences must be unterminated");
    assert!(format!("{error:?}").contains("unterminated"));
}

#[test]
fn users_guide_enforced_region_is_coherent() -> Result<()> {
    // Integration guard: the enforced-regions list must match the guide's
    // actual section, and the recipe marker must be loadable with the exact
    // directory line. Cargo re-runs a build script whose `rerun-if-changed`
    // path does not exist on every invocation (treating it as perpetually
    // dirty), so a corruption to a *missing* path would still pass the
    // behavioural test — this exact-line pin is what catches drift with
    // certainty. A corruption to a wrong *existing* path fails the
    // behavioural run outright.
    let examples = load_documented_examples()?;
    let recipe = documented_example("scenarios-build-script")?;
    assert!(
        examples
            .iter()
            .any(|example| example.id.as_str() == "scenarios-build-script")
    );
    assert_eq!(recipe.language.as_str(), "rust");
    assert!(
        recipe
            .body
            .contains("cargo::rerun-if-changed=tests/features"),
        "the recipe must watch the fixture's bound directory:\n{}",
        recipe.body
    );
    Ok(())
}

#[test]
fn enforced_regions_list_is_what_we_think() {
    let regions: Vec<(&str, &str)> = super::enforced_regions()
        .into_iter()
        .map(|EnforcedRegion { document, section }| (document.as_str(), section.as_str()))
        .collect();
    assert_eq!(
        regions,
        vec![("docs/users-guide.md", "Feature file rebuild invalidation")]
    );
}
