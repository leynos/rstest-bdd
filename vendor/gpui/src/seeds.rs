//! Seed resolution for the vendored GPUI test shim.
//!
//! The environment is read through an injected lookup so a test can pin the
//! seed set without mutating process state, and the production entry point
//! passes the real process environment through the same seam.

use std::{env, ops::RangeInclusive};

/// Resolve the seed sequence against an injected environment lookup.
///
/// The production entry point passes the process environment; the tests pass a
/// lookup that answers nothing, so no ambient `SEED` or `ITERATIONS` value can
/// change an expectation. Environment overrides take precedence over explicit
/// seeds to mirror GPUI's reproducible test-run contract.
pub(super) fn calculate_seeds_with(
    iterations: u64,
    explicit_seeds: &[u64],
    lookup: impl Fn(&str) -> Option<u64>,
) -> (impl Iterator<Item = u64> + '_, bool) {
    let iterations = lookup("ITERATIONS").unwrap_or(iterations).max(1);
    let env_seed = lookup("SEED");
    let start = env_seed.unwrap_or(0);
    let generated = build_seed_range(start, iterations);
    let explicit_seeds_to_append: &[u64] = match env_seed {
        Some(_) => &[],
        None => explicit_seeds,
    };

    let is_multiple_runs =
        generated.start() != generated.end() || !explicit_seeds_to_append.is_empty();
    (
        generated.chain(explicit_seeds_to_append.iter().copied()),
        is_multiple_runs,
    )
}

/// Parse an optional unsigned environment override without failing the test run.
///
/// Invalid values are diagnosed to stderr because this shim has no logging
/// subscriber and must make a discarded override observable to the developer.
pub(super) fn parse_env_u64(key: &str) -> Option<u64> { parse_env_u64_value(key, env::var(key)) }

/// Parse a fallible environment lookup as an optional unsigned override.
///
/// A missing variable is deliberately optional, whereas an invalid Unicode or
/// numeric value is discarded after its diagnostic makes the configuration
/// mistake visible to the test author.
#[expect(
    clippy::print_stderr,
    reason = "invalid environment overrides must be visible without global logging"
)]
pub(super) fn parse_env_u64_value(key: &str, value: Result<String, env::VarError>) -> Option<u64> {
    match value {
        Ok(value) => match value.parse() {
            Ok(parsed) => Some(parsed),
            Err(error) => {
                eprintln!("ignoring invalid {key} value {value:?}: {error}");
                None
            }
        },
        Err(env::VarError::NotPresent) => None,
        Err(env::VarError::NotUnicode(value)) => {
            eprintln!(
                "ignoring invalid {key} value {}: value is not valid Unicode",
                value.display()
            );
            None
        }
    }
}

/// Build a non-empty consecutive seed range, truncating safely on overflow.
///
/// The truncation warning is emitted directly because silently wrapping would
/// make a failing run unreproducible.
#[expect(
    clippy::print_stderr,
    reason = "seed-range truncation must be visible without global logging"
)]
fn build_seed_range(start: u64, iterations: u64) -> RangeInclusive<u64> {
    let last_offset = iterations.max(1) - 1;
    let end = start.checked_add(last_offset).unwrap_or_else(|| {
        eprintln!(
            "seed range overflowed for start={start} and iterations={iterations}; truncating \
             generated seeds"
        );
        u64::MAX
    });

    start..=end
}

#[cfg(test)]
mod tests {
    //! Coverage for seed resolution through the injected lookup.

    use std::{env::VarError, ffi::OsString};

    use proptest::prelude::*;

    use super::{calculate_seeds_with, parse_env_u64_value};

    /// A lookup that answers the given pairs, as the process environment would.
    fn lookup<'a>(entries: &'a [(&'a str, u64)]) -> impl Fn(&str) -> Option<u64> + 'a {
        move |key| {
            entries
                .iter()
                .find(|(name, _)| *name == key)
                .map(|(_, value)| *value)
        }
    }

    /// Resolve the seeds and the multi-seed flag for one injected environment.
    fn resolve(iterations: u64, explicit: &[u64], entries: &[(&str, u64)]) -> (Vec<u64>, bool) {
        let (seeds, is_multiple_runs) = calculate_seeds_with(iterations, explicit, lookup(entries));
        (seeds.collect(), is_multiple_runs)
    }

    /// Generated, explicit, iteration-override and seed-override inputs each
    /// resolve to the sequence GPUI's reproducible runs depend on.
    #[test]
    fn resolves_generated_explicit_and_environment_seeds() {
        assert_eq!(
            resolve(1, &[], &[]),
            (vec![0], false),
            "one iteration with nothing else set must run the generated seed once"
        );
        assert_eq!(
            resolve(3, &[], &[]),
            (vec![0, 1, 2], true),
            "three iterations must generate three consecutive seeds and flag the run as multiple"
        );
        assert_eq!(
            resolve(1, &[7, 9], &[]),
            (vec![0, 7, 9], true),
            "explicit seeds must be appended to the generated range and flag the run as multiple"
        );
        assert_eq!(
            resolve(1, &[], &[("ITERATIONS", 2)]),
            (vec![0, 1], true),
            "an iterations override must replace the caller's default"
        );
        assert_eq!(
            resolve(3, &[], &[("ITERATIONS", 0)]),
            (vec![0], false),
            "an iterations override of zero must still run once"
        );
        assert_eq!(
            resolve(1, &[7], &[("SEED", 5)]),
            (vec![5], false),
            "a seed override must take precedence over explicit seeds"
        );
        assert_eq!(
            resolve(2, &[7], &[("SEED", 5)]),
            (vec![5, 6], true),
            "a seed override must still generate the requested number of seeds"
        );
    }

    // Exercise the entire input domain while consuming only a bounded prefix.
    // The range is lazy, so an arbitrary iteration count is safe to generate;
    // the prefix keeps the assertions cheap. Lengths use `u128` to keep the
    // inclusive endpoint calculation exact at the `u64` boundary.
    proptest! {
        #[test]
        fn generated_ranges_stay_ordered_non_wrapping_and_non_empty(
            start in any::<u64>(),
            iterations in any::<u64>(),
        ) {
            let effective_iterations = iterations.max(1);
            let expected_last = start.saturating_add(effective_iterations - 1);
            let expected_length = u128::from(expected_last) - u128::from(start) + 1;
            let (seeds, is_multiple_runs) =
                calculate_seeds_with(iterations, &[], lookup(&[("SEED", start)]));

            let expected_size_hint = usize::try_from(expected_length)
                .map_or((usize::MAX, None), |length| (length, Some(length)));
            prop_assert_eq!(
                seeds.size_hint(),
                expected_size_hint,
                "start={} iterations={}: iterator length must match the truncated range",
                start,
                iterations
            );

            let prefix: Vec<_> = seeds.take(1024).collect();
            let expected_prefix_length = usize::try_from(expected_length.min(1024)).unwrap_or(1024);
            prop_assert_eq!(
                prefix.len(),
                expected_prefix_length,
                "start={} iterations={}: the bounded prefix must not be empty",
                start,
                iterations
            );
            prop_assert_eq!(
                prefix.first().copied(),
                Some(start),
                "start={} iterations={}: the prefix must begin at the requested seed",
                start,
                iterations
            );
            prop_assert!(
                prefix.iter().all(|seed| *seed >= start),
                "start={} iterations={}: range must not wrap below its start",
                start,
                iterations
            );
            prop_assert!(
                prefix
                    .iter()
                    .zip(prefix.iter().skip(1))
                    .all(|(current, next)| current.checked_add(1) == Some(*next)),
                "start={} iterations={}: seeds must stay consecutive",
                start,
                iterations
            );
            let expected_prefix_last = start.saturating_add(
                u64::try_from(expected_prefix_length)
                    .unwrap_or(u64::MAX)
                    .saturating_sub(1),
            );
            prop_assert_eq!(
                prefix.last().copied(),
                Some(expected_prefix_last),
                "start={} iterations={}: the bounded prefix must end at its computed seed",
                start,
                iterations
            );
            prop_assert_eq!(
                is_multiple_runs,
                expected_length > 1,
                "start={} iterations={}: run multiplicity must follow the generated bounds",
                start,
                iterations
            );
        }
    }

    // Explicit seeds always follow the generated range, and the multi-seed
    // flag matches whether more than one seed will run.
    proptest! {
        #[test]
        fn explicit_seeds_append_after_the_generated_range(
            iterations in 0u64..16,
            explicit in prop::collection::vec(any::<u64>(), 0..8),
        ) {
            let (range, is_multiple_runs) = resolve(iterations, &explicit, &[]);
            let generated = usize::try_from(iterations.max(1))
                .unwrap_or(usize::MAX)
                .min(range.len());
            let appended = range.get(generated..).unwrap_or_default();
            prop_assert!(
                appended == explicit.as_slice(),
                "iterations={iterations}: explicit seeds must follow the generated range"
            );
            prop_assert_eq!(is_multiple_runs, range.len() > 1);
        }
    }

    /// The named overflow boundaries still behave: the maximum seed truncates
    /// to itself, and an exhausted range runs once.
    #[test]
    fn truncation_boundaries_remain_explicit() {
        assert_eq!(
            resolve(3, &[], &[("SEED", u64::MAX)]),
            (vec![u64::MAX], false),
            "a range starting at the maximum seed must truncate to the seeds that exist"
        );
        assert_eq!(
            resolve(3, &[], &[("SEED", u64::MAX - 1)]),
            (vec![u64::MAX - 1, u64::MAX], true),
            "a range one below the maximum must include both remaining seeds"
        );
    }

    /// Environment parsing keeps valid numbers and discards everything else.
    #[test]
    fn parses_only_valid_environment_values() {
        assert_eq!(
            parse_env_u64_value("SEED", Ok("42".to_owned())),
            Some(42),
            "a numeric override must be accepted"
        );
        assert_eq!(
            parse_env_u64_value("SEED", Ok("not-a-number".to_owned())),
            None,
            "a non-numeric override must be discarded"
        );
        assert_eq!(
            parse_env_u64_value("SEED", Err(VarError::NotPresent)),
            None,
            "a missing override must read as absent rather than as zero"
        );
        assert_eq!(
            parse_env_u64_value("SEED", Err(VarError::NotUnicode(OsString::from("invalid")))),
            None,
            "a non-Unicode override must be discarded"
        );
    }
}
