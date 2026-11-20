//! The admission query: what it answers, and what it must not do to answer it.
//!
//! [`admit`] exists because a name that reads like a question must not compile
//! a dependency graph. That is a claim about *inaction*, and inaction is not
//! visible in a return value: a query that answered correctly could still have
//! taken the process-wide preparation lock, written a stamp over the one it
//! read, or run a build whose answer it then ignored. Every case here therefore
//! pins one of the three refusals the module documents — **no build**, **no
//! stamp write**, **no lock** — beside the answer itself.
//!
//! The build half is partly structural: `admit` takes no preparation closure,
//! so there is nothing for it to run. What a test can still establish is that
//! it charges nothing to the tree — no stamp appears where none existed, and a
//! stamp that was there comes back byte-identical.
//!
//! The lock half is asserted by holding the lock on another thread while the
//! query runs. A query that reached for it would wait until the holder gives
//! up, and the holder reports whether it gave up.

use std::{
    fs,
    io,
    sync::{
        Arc,
        Barrier,
        Mutex,
        mpsc::{self, RecvTimeoutError},
    },
    time::Duration,
};

use super::{super::stamp::stamp_path, prepared_profile, scratch_preparation};
use crate::nested_cargo::{
    Admission,
    Instruments,
    NestedCargoEnv,
    Preparation,
    PreparationSummary,
    admit,
    admit_with_probe,
    prepare_now,
    prepare_now_reported,
};

/// How long the lock holder waits for the query before deciding it blocked.
///
/// Only the failing direction depends on this: a query that took no lock lets
/// the holder be released at once, so a healthy run never waits at all. The
/// bound exists so a query that *did* take the lock fails the case instead of
/// hanging the suite.
const LOCK_HOLD_LIMIT: Duration = Duration::from_secs(10);

/// A preparation that has already stamped a usable profile.
///
/// The same starting point the eviction cases use: each case below compares the
/// query's answer against a tree whose state it established, so the answer
/// cannot be right for the wrong reason.
fn stamped_preparation() -> io::Result<(tempfile::TempDir, Preparation, NestedCargoEnv)> {
    let (directory, preparation, environment) = scratch_preparation()?;
    prepare_now(&preparation, || Ok(PreparationSummary::default()))?;
    Ok((directory, preparation, environment))
}

/// A preparation stamped against `identity` rather than the real toolchain.
///
/// The injectable cases need a stamp whose fingerprint was composed from a
/// compiler they can name. Preparing against the real one and then querying
/// against a chosen one would answer `NotPrepared` for every identity — the
/// right answer for the wrong reason — so the stamp is written the same way it
/// is read.
fn stamped_with(identity: &str) -> io::Result<(tempfile::TempDir, Preparation, NestedCargoEnv)> {
    let (directory, preparation, environment) = scratch_preparation()?;
    let identity = identity.to_owned();
    prepare_now_reported(
        &preparation,
        Instruments::new(move || Ok(identity), |_: &str| {}),
        || Ok(PreparationSummary::default()),
    )?;
    Ok((directory, preparation, environment))
}

#[test]
fn a_cold_tree_is_not_prepared_and_no_stamp_appears() -> io::Result<()> {
    // The stamp is the only thing `prepare_now` and the gate leave behind, so
    // its absence afterwards is the observation "the query built nothing". A
    // query that prepared would write one here, because this tree is exactly
    // what the lazy path prepares.
    let (directory, preparation, _environment) = scratch_preparation()?;
    let stamp = stamp_path(&preparation);
    assert!(!stamp.exists(), "the fixture must start cold");

    assert_eq!(admit(&preparation, false)?, Admission::NotPrepared);

    assert!(
        !stamp.exists(),
        "a query must not write the stamp a build would have written: {}",
        stamp.display()
    );
    assert!(
        directory.path().is_dir(),
        "the query must leave the tree alone, not merely decline to extend it"
    );
    Ok(())
}

#[test]
fn a_warm_tree_is_reusable_and_its_stamp_comes_back_byte_identical() -> io::Result<()> {
    // The other half of "writes no stamp": on a warm tree a query that stamped
    // would rewrite the file it had just read. Comparing the bytes catches a
    // rewrite even when the content is unchanged, and comparing them is what
    // makes this case independent of the clock — whole-second mtimes on some
    // filesystems would make a timestamp comparison pass for the wrong reason.
    let (_directory, preparation, _environment) = stamped_preparation()?;
    let stamp = stamp_path(&preparation);
    let before = fs::read(&stamp)?;

    assert_eq!(admit(&preparation, true)?, Admission::Reusable);
    assert_eq!(admit(&preparation, false)?, Admission::Reusable);

    assert_eq!(
        fs::read(&stamp)?,
        before,
        "a query must not rewrite the stamp it judged the tree by"
    );
    Ok(())
}

#[test]
fn an_evicted_profile_is_not_prepared() -> io::Result<()> {
    // A matching stamp whose tree has gone is not a reusable tree, and it is
    // not a *missing* one either. The gate refuses it under Nextest; the query
    // reports the same state without turning it into an error, so a caller can
    // report it without preparing.
    let (_directory, preparation, _environment) = stamped_preparation()?;
    let profile = prepared_profile(&preparation)?;
    fs::remove_dir_all(&profile)?;

    assert_eq!(admit(&preparation, true)?, Admission::NotPrepared);
    assert_eq!(admit(&preparation, false)?, Admission::NotPrepared);
    Ok(())
}

#[test]
fn an_unreadable_stamp_is_reported_by_runner() -> io::Result<()> {
    // The two runners reach opposite conclusions from the same fact, and the
    // query keeps the original error so either can explain itself. This is the
    // one branch where the answer differs by runner, so it is the one that
    // would silently regress if the policy moved into the stamp module.
    let (_directory, preparation, _environment) = stamped_preparation()?;
    let stamp = stamp_path(&preparation);
    // A stamp file whose bytes are not valid UTF-8 cannot be read back as a
    // stamp on any platform, which is what makes this fixture portable.
    fs::write(&stamp, b"\xff\xfe not a stamp")?;

    match admit(&preparation, true)? {
        Admission::Unreadable(reported) => assert!(
            !reported.error.is_empty(),
            "the query must carry the read failure it decided on"
        ),
        other => panic!("nextest must not admit an unreadable stamp: {other:?}"),
    }
    assert_eq!(
        admit(&preparation, false)?,
        Admission::NotPrepared,
        "plain cargo test prepares over a corrupt stamp, so the query answers the same as for a \
         stale one"
    );
    Ok(())
}

#[test]
fn the_query_answers_while_another_thread_holds_the_preparation_lock() -> io::Result<()> {
    // The third refusal. The lock is held on a worker thread for as long as the
    // query takes; a query that reached for it could not return until the
    // holder's limit expired, and the holder reports that expiry rather than
    // hiding it. The worker is released on the normal path immediately after
    // the query returns, so a healthy run does not wait.
    let (_directory, preparation, _environment) = stamped_preparation()?;
    let preparation = Arc::new(preparation);
    let holding = Arc::new(Barrier::new(2));
    let (released, await_release) = mpsc::channel::<()>();
    let holder_timed_out = Arc::new(Mutex::new(false));

    std::thread::scope(|scope| -> io::Result<()> {
        let holder = {
            let holding = Arc::clone(&holding);
            let holder_timed_out = Arc::clone(&holder_timed_out);
            scope.spawn(move || {
                // The guard is released when this closure returns, which is
                // what the query on the main thread is racing.
                let _guard = super::super::PREPARE_LOCK
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                holding.wait();
                match await_release.recv_timeout(LOCK_HOLD_LIMIT) {
                    // A timeout means the query is still waiting for this lock:
                    // record the expiry so the assertion below can fail with a
                    // reason rather than letting the case pass by luck. The
                    // sender dropping without a value is the normal path too,
                    // since a released holder has nothing left to wait for.
                    Err(RecvTimeoutError::Timeout) => {
                        *holder_timed_out
                            .lock()
                            .unwrap_or_else(std::sync::PoisonError::into_inner) = true;
                    }
                    Ok(()) | Err(RecvTimeoutError::Disconnected) => {}
                }
            })
        };

        holding.wait();
        let answered = admit(&preparation, true);
        // Release before asserting so a failure cannot leave the holder
        // waiting out its full limit.
        let _ = released.send(());
        holder.join().expect("the lock holder must not panic");
        assert!(
            !*holder_timed_out
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner),
            "the admission query must not wait for the preparation lock"
        );
        assert_eq!(answered?, Admission::Reusable);
        Ok(())
    })
}

#[test]
fn the_probe_can_be_supplied_without_a_second_toolchain_resolution() -> io::Result<()> {
    // `admit` resolves the real toolchain once. The injectable form proves the
    // query's answer is a pure function of the tree and the identity supplied:
    // the same tree judged against the same identity twice answers the same,
    // and the probe is called exactly once each time rather than being cached
    // across calls or skipped.
    let fixed = "rustc 1.96.0-nightly (fixed)";
    let (_directory, preparation, _environment) = stamped_with(fixed)?;
    let mut probes = 0_u32;

    let first = admit_with_probe(&preparation, true, || {
        probes += 1;
        Ok(String::from(fixed))
    })?;
    let second = admit_with_probe(&preparation, true, || {
        probes += 1;
        Ok(String::from(fixed))
    })?;

    assert_eq!(probes, 2, "each query identifies the compiler once");
    assert_eq!(first, second);
    assert_eq!(first, Admission::Reusable);
    Ok(())
}

#[test]
fn a_different_compiler_makes_the_stamp_stale() -> io::Result<()> {
    // The fingerprint covers the toolchain identity, so a query asked against
    // a compiler the stamp does not describe must not admit the tree. Without
    // this the injectable form above could pass while ignoring the identity it
    // was handed.
    let (_directory, preparation, _environment) = stamped_with("rustc 1.96.0-nightly (fixed)")?;

    assert_eq!(
        admit_with_probe(&preparation, true, || Ok(String::from(
            "rustc 0.0.0 (a compiler the stamp never saw)"
        )))?,
        Admission::NotPrepared
    );
    Ok(())
}
