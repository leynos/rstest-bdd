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
