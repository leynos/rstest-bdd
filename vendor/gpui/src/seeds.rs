//! Seed resolution for the vendored GPUI test shim.
//!
//! The environment is read through an injected lookup so a test can pin the
//! seed set without mutating process state, and the production entry point
//! passes the real process environment through the same seam.

use std::env;

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
    let seeds = select_seeds(iterations, explicit_seeds, lookup("SEED"));

    let is_multiple_runs = seeds.len() > 1;
    (seeds.into_iter(), is_multiple_runs)
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

/// Select deterministic seeds, giving an environment override precedence.
fn select_seeds(iterations: u64, explicit_seeds: &[u64], env_seed: Option<u64>) -> Vec<u64> {
    if let Some(seed) = env_seed {
        return build_seed_range(seed, iterations);
    }

    let mut seeds = build_seed_range(0, iterations);
    if iterations == 1 && explicit_seeds.is_empty() {
        return seeds;
    }

    seeds.extend(explicit_seeds.iter().copied());
    seeds
}

/// Build a non-empty consecutive seed range, truncating safely on overflow.
///
/// The truncation warning is emitted directly because silently wrapping would
/// make a failing run unreproducible.
#[expect(
    clippy::print_stderr,
    reason = "seed-range truncation must be visible without global logging"
)]
fn build_seed_range(start: u64, iterations: u64) -> Vec<u64> {
    let mut seeds = Vec::new();
    for offset in 0..iterations.max(1) {
        if let Some(seed) = start.checked_add(offset) {
            seeds.push(seed);
        } else {
            eprintln!(
                "seed range overflowed for start={start} and iterations={iterations}; truncating \
                 generated seeds"
            );
            break;
        }
    }

    if seeds.is_empty() {
        seeds.push(start);
    }

    seeds
}

#[cfg(test)]
mod tests {
    //! Coverage for seed resolution through the injected lookup.

    use std::{env::VarError, ffi::OsString};

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

    /// A range that would overflow truncates instead of wrapping, and an
    /// exhausted range still runs once.
    #[test]
    fn truncated_ranges_stay_ordered_and_non_empty() {
        assert_eq!(
            resolve(3, &[], &[("SEED", u64::MAX)]),
            (vec![u64::MAX], false),
            "a range starting at the maximum seed must truncate to the seeds that exist"
        );

        for iterations in 0..=8 {
            for start in [0, 17, u64::MAX - 3] {
                let (range, _) = resolve(iterations, &[], &[("SEED", start)]);
                assert!(
                    !range.is_empty(),
                    "iterations={iterations} start={start} must still run at least one seed"
                );
                assert!(
                    range.iter().all(|seed| *seed >= start),
                    "iterations={iterations} start={start} must never wrap below its start"
                );
                assert!(
                    range
                        .iter()
                        .zip(range.iter().skip(1))
                        .all(|(current, next)| current.checked_add(1) == Some(*next)),
                    "iterations={iterations} start={start} must stay consecutive"
                );
            }
        }
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
