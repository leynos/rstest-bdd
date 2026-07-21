//! Property-based tests for guard-based `StepContext` borrowing (ADR-012).
//!
//! Drives arbitrary sequences of acquire/release operations over a small
//! pool of owned fixtures and checks the outcomes against a reference model
//! of `RefCell` borrow semantics: a mutable borrow succeeds iff the fixture
//! has no live guards, a shared borrow succeeds iff it has no live mutable
//! guard, and borrows of distinct fixtures never interfere.

use proptest::prelude::*;
use rstest_bdd::{FixtureBorrowError, FixtureRef, FixtureRefMut, StepContext};

const FIXTURE_NAMES: [&str; 3] = ["alpha", "beta", "gamma"];

/// A single operation in a generated borrow scenario.
#[derive(Debug, Clone, Copy)]
enum Op {
    /// Attempt to acquire a borrow of fixture `index`, holding any guard.
    Acquire { index: usize, mutable: bool },
    /// Release the guard in slot `slot % held.len()`, if any guards are held.
    Release { slot: usize },
}

fn op_strategy() -> impl Strategy<Value = Op> {
    prop_oneof![
        (0..FIXTURE_NAMES.len(), any::<bool>())
            .prop_map(|(index, mutable)| Op::Acquire { index, mutable }),
        (0usize..16).prop_map(|slot| Op::Release { slot }),
    ]
}

/// A held guard plus the fixture it borrows, for model bookkeeping.
///
/// The guard fields are never read: they exist to keep the underlying
/// `RefCell` borrow alive until the slot is released.
enum HeldGuard<'a> {
    Shared {
        index: usize,
        _guard: FixtureRef<'a, u32>,
    },
    Mutable {
        index: usize,
        _guard: FixtureRefMut<'a, u32>,
    },
}

impl HeldGuard<'_> {
    fn fixture_index(&self) -> usize {
        match self {
            Self::Shared { index, .. } | Self::Mutable { index, .. } => *index,
        }
    }

    fn is_mutable(&self) -> bool {
        matches!(self, Self::Mutable { .. })
    }
}

proptest! {
    /// Borrow outcomes match the reference model for arbitrary operation
    /// sequences.
    #[test]
    fn borrow_outcomes_match_reference_model(
        ops in proptest::collection::vec(op_strategy(), 1..40),
    ) {
        let cells: Vec<_> = (0..FIXTURE_NAMES.len())
            .map(|index| {
                #[expect(
                    clippy::cast_possible_truncation,
                    reason = "fixture pool has three entries"
                )]
                StepContext::owned_cell(index as u32)
            })
            .collect();
        let mut ctx = StepContext::default();
        for (name, cell) in FIXTURE_NAMES.iter().zip(&cells) {
            ctx.insert_owned::<u32>(name, cell);
        }

        let mut held: Vec<HeldGuard<'_>> = Vec::new();
        for op in ops {
            match op {
                Op::Acquire { index, mutable } => {
                    let Some(&name) = FIXTURE_NAMES.get(index) else {
                        return Err(TestCaseError::fail("fixture index in range"));
                    };
                    let has_mut = held
                        .iter()
                        .any(|g| g.fixture_index() == index && g.is_mutable());
                    let has_any = held.iter().any(|g| g.fixture_index() == index);
                    if mutable {
                        match ctx.try_borrow_mut::<u32>(name) {
                            Ok(guard) => {
                                prop_assert!(
                                    !has_any,
                                    "mutable borrow must fail while any guard is live"
                                );
                                held.push(HeldGuard::Mutable { index, _guard: guard });
                            }
                            Err(err) => {
                                prop_assert!(has_any, "mutable borrow failed without conflict");
                                prop_assert_eq!(
                                    err,
                                    FixtureBorrowError::AlreadyBorrowed { name: name.into() }
                                );
                            }
                        }
                    } else {
                        match ctx.try_borrow::<u32>(name) {
                            Ok(guard) => {
                                prop_assert!(
                                    !has_mut,
                                    "shared borrow must fail while a mutable guard is live"
                                );
                                held.push(HeldGuard::Shared { index, _guard: guard });
                            }
                            Err(err) => {
                                prop_assert!(has_mut, "shared borrow failed without conflict");
                                prop_assert_eq!(
                                    err,
                                    FixtureBorrowError::AlreadyBorrowed { name: name.into() }
                                );
                            }
                        }
                    }
                }
                Op::Release { slot } => {
                    if !held.is_empty() {
                        held.swap_remove(slot.rem_euclid(held.len()));
                    }
                }
            }
        }
    }
}
