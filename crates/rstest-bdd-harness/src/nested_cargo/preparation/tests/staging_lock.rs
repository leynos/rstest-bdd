//! Staging is serialized with other staging and with preparation.
//!
//! A staging step replaces the tree a preparation's nested build reads. Run
//! outside the preparation lock, two callers that both find the tree cold can
//! remove and copy it at once, and one can remove it beneath the other's
//! build. The case here drives two threads through the staged entry point and
//! fails if a staging step ever overlaps another staging step or a build.

use std::{
    io,
    sync::{
        Barrier,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    time::Duration,
};

use super::scratch_preparation;
use crate::nested_cargo::{PreparationSummary, invalidate, prepare_now_staged};

/// Occupy the shared tree for a moment, counting any second occupant.
///
/// `busy` stands for the scratch tree: whoever holds it is copying into it or
/// building from it. Finding it already held is the overlap under test.
fn occupy(busy: &AtomicBool, overlaps: &AtomicUsize) {
    if busy.swap(true, Ordering::SeqCst) {
        overlaps.fetch_add(1, Ordering::SeqCst);
    }
    std::thread::sleep(Duration::from_millis(50));
    busy.store(false, Ordering::SeqCst);
}

#[test]
fn staging_never_overlaps_another_staging_or_a_build() -> io::Result<()> {
    let (_directory, preparation, _environment) = scratch_preparation()?;
    let busy = AtomicBool::new(false);
    let overlaps = AtomicUsize::new(0);
    let start = Barrier::new(2);

    let run = || -> io::Result<()> {
        start.wait();
        for _ in 0..3 {
            prepare_now_staged(
                &preparation,
                || {
                    occupy(&busy, &overlaps);
                    Ok(())
                },
                || {
                    occupy(&busy, &overlaps);
                    Ok(PreparationSummary::default())
                },
            )?;
            // Withdraw the stamp so the next round builds again, which is
            // what puts a build beside the other thread's staging.
            invalidate(&preparation)?;
        }
        Ok(())
    };
    std::thread::scope(|scope| {
        let first = scope.spawn(run);
        let second = scope.spawn(run);
        first
            .join()
            .map_err(|_| io::Error::other("the first staging thread panicked"))??;
        second
            .join()
            .map_err(|_| io::Error::other("the second staging thread panicked"))?
    })?;

    assert_eq!(
        overlaps.load(Ordering::SeqCst),
        0,
        "a staging step must not run beside another staging step or a build"
    );
    Ok(())
}
