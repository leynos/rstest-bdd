//! Shared argument-parsing helpers for macro entry points.

use syn::parse::ParseStream;

/// Assign `value` to `slot` if empty, or return a duplicate-argument error.
pub(crate) fn set_once_arg<T>(
    slot: &mut Option<T>,
    value: T,
    label: &str,
    input: ParseStream<'_>,
) -> syn::Result<()> {
    if slot.is_some() {
        return Err(input.error(format!("duplicate `{label}` argument")));
    }
    *slot = Some(value);
    Ok(())
}
