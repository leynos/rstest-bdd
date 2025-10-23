//! Normalises and combines tag sets so tag filtering across
//! macros remains deterministic regardless of feature ordering
//! or raw tag formatting.

use std::collections::HashSet;

/// Normalise a tag to start with '@'.
fn normalize_tag(tag: &str) -> String {
    let trimmed = tag.trim();
    if trimmed.starts_with('@') {
        trimmed.to_string()
    } else {
        format!("@{trimmed}")
    }
}

/// Normalise all tags in the target vector to start with '@'.
fn normalize_existing_tags(target: &mut [String]) {
    for tag in target.iter_mut() {
        if !tag.starts_with('@') {
            *tag = format!("@{tag}");
        }
    }
}

/// Add tags from additions to target, skipping duplicates.
fn add_unique_tags(target: &mut Vec<String>, additions: &[String]) {
    for tag in additions {
        let formatted = normalize_tag(tag);
        if !target.iter().any(|existing| existing == &formatted) {
            target.push(formatted);
        }
    }
}

/// Extend the destination tag set with new values, preserving order and
/// removing duplicates.
///
/// Both `target` and `additions` may contain tags without a leading `@` or
/// repeated entries. Normalisation occurs in-place so callers do not need to
/// pre-sanitise their inputs.
pub(crate) fn extend_tag_set(target: &mut Vec<String>, additions: &[String]) {
    normalize_existing_tags(target);

    let mut seen = HashSet::new();
    target.retain(|tag| seen.insert(tag.clone()));

    add_unique_tags(target, additions);
}

/// Merge two tag sets, preserving insertion order and de-duplicating values.
///
/// The returned collection always uses `@tag` formatting and omits duplicates
/// even if `base` or `additions` contain un-normalised values.
///
/// # Examples
///
/// ```
/// # use crate::parsing::tags::merge_tag_sets;
/// let base = vec!["@fast".to_string(), "slow".to_string()];
/// let additions = vec!["@fast".to_string(), "web".to_string()];
/// let merged = merge_tag_sets(&base, &additions);
/// assert_eq!(
///     merged,
///     vec![
///         "@fast".to_string(),
///         "@slow".to_string(),
///         "@web".to_string(),
///     ]
/// );
/// ```
pub(crate) fn merge_tag_sets(base: &[String], additions: &[String]) -> Vec<String> {
    let mut merged = base.to_vec();
    extend_tag_set(&mut merged, additions);
    merged
}
