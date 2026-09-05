//! Property-based invariants for [`saturate_to_i32`].
//!
//! The clamping behaviour must hold for the complete `i64` input domain:
//! values above `i32::MAX` saturate to `i32::MAX`, values below `i32::MIN`
//! saturate to `i32::MIN`, and in-range values convert unchanged. Every
//! result must also stay inside the `i32` range.

use proptest::prelude::*;

use super::saturate_to_i32;

proptest! {
    /// Any `i64` input must clamp exactly like an independent boundary
    /// reference, and the result must remain a valid `i32`.
    #[test]
    fn clamps_to_i32_range(value in any::<i64>()) {
        // Independent reference: explicit boundary checks instead of
        // restating the production `TryFrom`-based expression.
        let expected = if value > i64::from(i32::MAX) {
            i32::MAX
        } else if value < i64::from(i32::MIN) {
            i32::MIN
        } else {
            // In range, so the conversion cannot fail.
            i32::try_from(value).expect("value is within i32 range")
        };

        let result = saturate_to_i32(value);

        // Result matches the clamp reference.
        prop_assert_eq!(result, expected);
        // Result always lies within the `i32` bounds.
        prop_assert!((i32::MIN..=i32::MAX).contains(&result));
    }

    /// The upper saturation edge returns exactly `i32::MAX`.
    #[test]
    fn saturates_at_upper_boundary(value in (i64::from(i32::MAX) + 1)..=i64::MAX) {
        prop_assert_eq!(saturate_to_i32(value), i32::MAX);
    }

    /// The lower saturation edge returns exactly `i32::MIN`.
    #[test]
    fn saturates_at_lower_boundary(value in i64::MIN..=(i64::from(i32::MIN) - 1)) {
        prop_assert_eq!(saturate_to_i32(value), i32::MIN);
    }
}
