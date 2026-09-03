//! State-machine helpers for marked fenced examples in one enforced region.

use eyre::{Result, bail};

use super::{
    DocumentedExample,
    ExampleId,
    ScanRegion,
    ScanState,
    extract_marker_id,
    is_fence,
    parse_fenced_example,
};

/// Scan `region.lines[region.start..region.end]`, extracting marked fenced examples
/// and hard-erroring on unmarked ones.
pub(super) fn scan_region(region: &ScanRegion<'_>, state: &mut ScanState) -> Result<()> {
    let mut idx = region.start;
    let mut pending_marker: Option<(ExampleId, usize)> = None;
    while idx < region.end {
        let Some(line) = region.lines.get(idx) else {
            break;
        };
        match classify_region_line(line) {
            RegionLine::Blank => idx += 1,
            RegionLine::Marker(id) => {
                replace_pending_marker(&mut pending_marker, id, idx, region)?;
                idx += 1;
            }
            RegionLine::Content => {
                reject_pending_marker(&mut pending_marker, region)?;
                idx += 1;
            }
            RegionLine::Fence => {
                idx += collect_fenced_example(&mut pending_marker, region, state, idx)?;
            }
        }
    }
    reject_pending_marker(&mut pending_marker, region)
}

enum RegionLine {
    Blank,
    Marker(ExampleId),
    Fence,
    Content,
}

fn classify_region_line(line: &str) -> RegionLine {
    if line.trim().is_empty() {
        RegionLine::Blank
    } else if let Some(id) = extract_marker_id(line) {
        RegionLine::Marker(id)
    } else if is_fence(line) {
        RegionLine::Fence
    } else {
        RegionLine::Content
    }
}

fn replace_pending_marker(
    pending_marker: &mut Option<(ExampleId, usize)>,
    id: ExampleId,
    idx: usize,
    region: &ScanRegion<'_>,
) -> Result<()> {
    if let Some((pending_id, marker_line)) = pending_marker.replace((id, idx)) {
        return missing_fence_after_marker(&pending_id, marker_line, region);
    }
    Ok(())
}

fn reject_pending_marker(
    pending_marker: &mut Option<(ExampleId, usize)>,
    region: &ScanRegion<'_>,
) -> Result<()> {
    if let Some((id, marker_line)) = pending_marker.take() {
        return missing_fence_after_marker(&id, marker_line, region);
    }
    Ok(())
}

fn collect_fenced_example(
    pending_marker: &mut Option<(ExampleId, usize)>,
    region: &ScanRegion<'_>,
    state: &mut ScanState,
    idx: usize,
) -> Result<usize> {
    let (id, _) = pending_marker.take().ok_or_else(|| {
        eyre::eyre!(
            "unmarked fenced block at line {} in the enforced `{}` region: every fence there \
             needs a `<!-- tested-example: id -->` marker immediately before it",
            idx + 1,
            region.document.as_str(),
        )
    })?;
    let (example, consumed) = parse_fenced_example(region.lines, idx, region.end, id)?;
    validate_example_identifier(&example, region, idx)?;
    register_example(state, example, region)?;
    Ok(consumed)
}

fn validate_example_identifier(
    example: &DocumentedExample,
    region: &ScanRegion<'_>,
    idx: usize,
) -> Result<()> {
    if example.id.as_str().is_empty() {
        bail!(
            "empty tested-example identifier in `{}` at line {}",
            region.document.as_str(),
            idx + 1,
        );
    }
    Ok(())
}

fn register_example(
    state: &mut ScanState,
    example: DocumentedExample,
    region: &ScanRegion<'_>,
) -> Result<()> {
    if !state.seen.insert(example.id.as_str().to_owned()) {
        bail!(
            "duplicate tested-example identifier `{}` in `{document}`",
            example.id.as_str(),
            document = region.document.as_str(),
        );
    }
    state.collected.push(example);
    Ok(())
}

fn missing_fence_after_marker(
    id: &ExampleId,
    marker_line: usize,
    region: &ScanRegion<'_>,
) -> Result<()> {
    bail!(
        "tested-example marker `{}` at line {} in `{}` is not followed by a fenced block",
        id.as_str(),
        marker_line + 1,
        region.document.as_str(),
    )
}
