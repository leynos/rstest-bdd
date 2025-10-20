/// Extend the destination tag set with new values, preserving order and
/// removing duplicates.
pub(crate) fn extend_tag_set(target: &mut Vec<String>, additions: &[String]) {
    for tag in additions {
        let formatted = if tag.starts_with('@') {
            tag.clone()
        } else {
            format!("@{tag}")
        };
        if !target.iter().any(|existing| existing == &formatted) {
            target.push(formatted);
        }
    }
}

/// Merge two tag sets, preserving insertion order and de-duplicating values.
pub(crate) fn merge_tag_sets(base: &[String], additions: &[String]) -> Vec<String> {
    let mut merged = base.to_vec();
    extend_tag_set(&mut merged, additions);
    merged
}
