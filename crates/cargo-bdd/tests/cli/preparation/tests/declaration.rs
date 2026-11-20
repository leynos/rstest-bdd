//! Regression coverage for the prepared CLI fixture.
//!
//! The family's input hash includes the workspace's Cargo configuration, so a
//! profile or linker change forces preparation instead of being absorbed by a
//! warm stamp. And the witness the preparation test asserts on — the fixture's
//! own test binaries in the profile — is shown to reject a tree that holds
//! unrelated artefacts instead, because a check that only ever runs against a
//! warm tree cannot distinguish a real preparation from a no-op.
//!
//! Declared beside `withdrawal` and `ownership` rather than inline in
//! `preparation.rs`, which keeps that file inside the repository's line budget.

use super::{
    FIXTURE_PACKAGE,
    cli_preparation_description,
    closure::fixture_dependency_closure,
    family_roots,
    nested_cargo,
};
use crate::workspace_root;

#[test]
fn a_spent_gate_budget_refuses_before_the_closure_query_starts() {
    // The gate creates one deadline and charges the closure query and both
    // identity probes against it. The failure this pins is the one a per-command
    // bound alone could not: a query that *succeeds* just inside its own limit
    // used to hand the probes a budget already spent, and the runner killed the
    // test before either reported. A budget that is spent before the query has
    // even started must be refused for the same reason, and the refusal must
    // name the query so a reader knows which command never ran.
    let error = cli_preparation_description(
        nested_cargo::Deadline::starting_now(std::time::Duration::ZERO),
        super::closure::TIMED_GRAPH_QUERY_TIMEOUT,
    )
    .expect_err("a spent budget must not start the closure query");
    let text = format!("{error:#}");
    assert!(
        text.contains("fixture dependency closure"),
        "the refusal must name the command it declined to start: {text}"
    );
    // And it is a deadline refusal rather than a resolution failure: the query
    // never ran, so the fixture is not reported as unresolvable.
    assert!(
        !text.contains("cargo tree"),
        "a spent budget must not report the query as having run and failed: {text}"
    );
}

#[test]
fn the_workspace_cargo_configuration_is_an_input() {
    // The fixture build runs beneath the workspace, so Cargo applies the
    // committed `.cargo/config.toml`: its development profile options and
    // Linux linker flags rebuild dependency artefacts without changing a
    // fixture source. A stamp that ignored it would survive such an edit and
    // the timed CLI test would compile the graph inside its own 180 s limit.
    //
    // Both sides are the family's *own* root list, so a root the family
    // stopped naming disappears from both and the case fails rather than
    // passing on a restated copy. A restated control would keep its own copy
    // of every root, and the hashes would then differ for a reason unrelated
    // to the configuration.
    let workspace = workspace_root();
    let roots = family_roots().expect("the family's roots should resolve");
    // Both sides are cut down by the same predicate, and the only root they
    // differ in is the committed `.cargo` one. Dropping every non-workspace
    // root instead — the obvious control — also removes `$CARGO_HOME/config.toml`
    // on a host that has one, and a family that stopped hashing the workspace
    // file would still report two different hashes: the case would pass over
    // the regression it exists to catch. Removing the Cargo home entry from
    // both sides keeps that one root as the single difference, without copying
    // a file outside the checkout.
    let committed = workspace.join(".cargo");
    let cargo_home = std::env::var_os("CARGO_HOME")
        .map(std::path::PathBuf::from)
        .or_else(|| {
            std::env::var_os("HOME").map(|home| std::path::PathBuf::from(home).join(".cargo"))
        });
    let shared = |root: &std::path::PathBuf| {
        cargo_home
            .as_ref()
            .is_none_or(|home| !root.starts_with(home))
    };
    // `workspace_root` resolves, so both sides of this comparison are already
    // in one path form and a plain prefix test asks the question the caller
    // means — is this the same file — rather than the one the spelling
    // suggests. Comparing spellings is what failed on the Windows lane: the
    // probe reports the committed configuration through a `\\?\` verbatim
    // prefix, which no plainly spelled prefix matches, so the file was read as
    // absent while it sat in the list.
    let within_workspace = |root: &std::path::PathBuf| root.starts_with(&committed);
    let workspace_configuration: Vec<std::path::PathBuf> = roots
        .iter()
        .filter(|root| within_workspace(root))
        .cloned()
        .collect();
    assert!(
        workspace_configuration
            .iter()
            .any(|path| path.ends_with("config.toml")),
        "this checkout commits `.cargo/config.toml`, so the family must name it; the roots were \
         {roots:?}"
    );
    let control: Vec<std::path::PathBuf> =
        roots.iter().filter(|root| shared(root)).cloned().collect();
    // Filtered by the same predicate the removal is described with, not by
    // whole-path inequality: `control` holds the configuration's resolved form,
    // which differs from `committed` in every component the resolution added.
    // Comparing whole paths would leave the entry in place and this case would
    // then assert that a set equals itself.
    let without: Vec<std::path::PathBuf> = control
        .iter()
        .filter(|root| !within_workspace(root))
        .cloned()
        .collect();
    // The configuration is named as a root itself, so its full path reaches
    // the hash. That is deliberate — `input_hash` records a named root's
    // provenance because `build.rustc` and the other path-valued keys resolve
    // against the configuration's own directory — and it is why the two forms
    // are *not* interchangeable to the hash. Spelling cannot be what this case
    // pins: `family_roots` supplies the resolved form, so a stamp written
    // against it stays stable across a re-clone, which is the property the
    // fixture tests rely on.
    assert!(
        control
            .iter()
            .filter(|root| within_workspace(root))
            .all(|root| root.is_absolute()),
        "the configuration must be named absolutely, or the stamp would depend on where the \
         fixture happened to run: {control:?}"
    );
    let with_hash =
        nested_cargo::input_hash(&control).expect("hash with the workspace configuration");
    let without_hash =
        nested_cargo::input_hash(&without).expect("hash without the workspace configuration");
    assert_ne!(
        with_hash, without_hash,
        "the workspace Cargo configuration must change the CLI input hash; removing \
         {workspace_configuration:?} left it unchanged"
    );
}

#[test]
fn the_timed_graph_query_keeps_its_own_smaller_budget() {
    // The graph query runs before the gate has read the stamp, so a timed
    // assertion pays for it even when the fixture is prepared. The preparation
    // budget is nine minutes, and this binary's period is 180 s: under the
    // preparation bound a contended query could not return before nextest
    // killed the test, discarding the captured child output that names the
    // wait. The two bounds must therefore stay distinct, and this is the
    // comparison that keeps a later edit from re-merging them.
    //
    // The `.config/nextest.toml` period is not parsed here — this crate has no
    // TOML reader among its dev-dependencies — so the 180 s figure below is
    // the CLI override's period, restated. Keeping the two bounds apart is
    // what the assertion pins; a change to either figure belongs beside the
    // configuration entry that sets it, and must be mirrored here.
    //
    // Only the query's own bound is checked against the period here, because
    // the query and the probes are no longer charged separately: the gate
    // creates one `Deadline` and charges the closure query and both identity
    // probes against it, so their sum cannot exceed the budget however each
    // command fares. A query that succeeds just inside its limit therefore
    // leaves the probes the remainder rather than a fresh allowance.
    //
    // This was not true before the deadline existed. Holding only the query's
    // bound below the period left a slow *success* free to hand both probes a
    // budget that was already spent; a query that *failed* was safe, which is
    // what the old comment generalized from, and the case in between is the
    // one that reached the runner's kill.
    let query = super::closure::TIMED_GRAPH_QUERY_TIMEOUT.as_secs();
    let preparation = nested_cargo::PREPARATION_COMMAND_TIMEOUT.as_secs();
    let cli_period = 180;
    assert!(
        query < preparation,
        "the timed graph query must keep a budget below the preparation test's {preparation}s, or \
         nextest kills the test before the query can report its own timeout"
    );
    assert!(
        query < cli_period,
        "a {query}s query under the CLI override's {cli_period}s period would be killed by \
         nextest before it could report its own timeout"
    );
}

#[test]
fn the_owned_packages_cover_the_whole_dependency_closure_and_reach_the_gate() {
    // Both halves of the fixture's build have to be declared, and the case
    // is written against the live graph rather than against a literal so it
    // cannot drift from the fixture. The workspace crates are the transitive
    // path dependencies the manifest never names; the registry crates are
    // `inventory`, `serial_test` and their trees, whose units are compiled
    // into the same profile and are evictable in exactly the same way even
    // though their sources cannot change.
    let closure = fixture_dependency_closure(super::closure::TIMED_GRAPH_QUERY_TIMEOUT)
        .expect("the fixture closure must resolve");
    let declared: std::collections::BTreeSet<&str> = std::iter::once(FIXTURE_PACKAGE)
        .chain(closure.iter().map(String::as_str))
        .collect();
    // The list the gate actually receives, not only a set rebuilt here from
    // the closure. A preparation that stopped wiring the closure would leave
    // every assertion below green — they test the local set — while the gate
    // filtered the dependency's records out of both sides and an eviction
    // passed. The fixture's test targets are part of that list as well: they
    // are units named for the target rather than for any package, so the
    // closure alone leaves their outputs unowned.
    let query_timeout = super::closure::TIMED_GRAPH_QUERY_TIMEOUT;
    let (preparation, _environment, _deadline) = cli_preparation_description(
        nested_cargo::Deadline::starting_now(query_timeout),
        query_timeout,
    )
    .expect("preparation description should resolve");
    let owned: std::collections::BTreeSet<&str> = preparation
        .owned_packages
        .iter()
        .map(String::as_str)
        .collect();
    let expected: std::collections::BTreeSet<&str> = closure
        .iter()
        .map(String::as_str)
        .chain(super::witness::FIXTURE_TEST_BINARIES)
        .collect();
    assert_eq!(
        owned, expected,
        "the preparation must hand the gate exactly the fixture's dependency closure and its test \
         targets"
    );

    // The registry half specifically: a list built from the manifests alone
    // holds the path crates and neither of these.
    for package in ["inventory", "serial_test"] {
        assert!(
            declared.contains(package),
            "the declaration must cover the fixture's registry dependencies; {package} is \
             compiled into the shared profile and its eviction would pass the gate"
        );
    }
    // And the transitive workspace half, which the manifests also omit.
    for package in [
        "rstest-bdd-harness",
        "rstest-bdd-patterns",
        "rstest-bdd-policy",
    ] {
        assert!(
            declared.contains(package),
            "the declaration must cover the fixture's transitive workspace dependencies; \
             {package} is built here and its eviction would pass the gate"
        );
    }
}

#[test]
fn the_closure_query_is_charged_against_the_budget_the_gate_receives() {
    // The hand-off the gate depends on. `cli_preparation_description` pays for
    // the closure query and returns the deadline the probes are then charged
    // against, so what it returns must be the budget it was given less the
    // query — not a fresh allowance. The gate cases in the harness start from a
    // budget that is already spent or ample, so they stay green if a new
    // deadline is substituted here; only measuring across the real query shows
    // it. The query takes the package-cache lock and spawns Cargo, so its cost
    // is never zero.
    let budget = super::closure::TIMED_GRAPH_QUERY_TIMEOUT;
    let deadline = nested_cargo::Deadline::starting_now(budget);
    let started = std::time::Instant::now();
    let (_preparation, _environment, handed_on) =
        cli_preparation_description(deadline, budget).expect("the description should resolve");
    let spent = started.elapsed();
    let left = handed_on
        .remaining()
        .expect("a two-minute budget outlives one graph query");
    assert!(
        left + spent <= budget,
        "the probes must be left the budget less the query ({spent:?}), not a fresh allowance: \
         {left:?} remains of {budget:?}"
    );
}

#[test]
fn a_query_that_spends_the_budget_leaves_the_gate_none_for_its_probes() {
    // The same hand-off, one call further on: `gate_from` is what passes the
    // charged deadline from the query to the gate, and `ensure_cli_prepared`
    // reaches it with the production query. The query here is a controlled
    // one that succeeds only once the budget has gone. If the gate were handed
    // a fresh deadline, the Cargo probe would run and the refusal would be the
    // stamp's `NotFound`; charged against the same budget, the probe is
    // declined by name before it starts.
    //
    // The inputs are resolved before the budget starts, and the query sleeps
    // for the bound it is given rather than for a fixed time. So the budget is
    // always live when the query starts and always spent when it returns,
    // however long the hashing took on this host.
    let inputs = super::cli_preparation().expect("the family's inputs should resolve");
    let budget = super::budget::Budget::starting_now(std::time::Duration::from_millis(200));
    let error = super::budget::gate_from(inputs, budget, true, |bound| {
        std::thread::sleep(bound + std::time::Duration::from_millis(50));
        Ok(vec!["rstest-bdd".to_owned()])
    })
    .expect_err("a gate whose query spent the budget must not start a probe");
    let text = format!("{error:?}");
    assert!(
        text.contains("deadline expired before `cargo -vV`"),
        "the refusal must be the spent budget's, naming the probe it declined: {text}"
    );
}

#[test]
fn the_query_is_given_its_bound_with_no_description_to_wait_for() {
    // `charged` takes the budget and the query and nothing else, so no
    // description can be built between taking the bound and running the
    // query: there is nothing here to build one from. Moving that work back
    // in front of the query means giving this function the family's inputs,
    // and this call then stops compiling. What is left to assert is that the
    // query runs, once, under the bound the budget allows, and that its
    // units reach the caller with the test targets beside them.
    let limit = std::time::Duration::from_secs(30);
    let budget = super::budget::Budget {
        deadline: nested_cargo::Deadline::starting_now(std::time::Duration::from_secs(3600)),
        query_limit: limit,
    };
    let mut bounds = Vec::new();
    let owned = super::budget::charged(budget, |bound| {
        bounds.push(bound);
        Ok(vec!["rstest-bdd".to_owned()])
    })
    .expect("a live budget must run the query");
    assert_eq!(bounds, [limit], "the query runs once, under its own limit");
    assert_eq!(
        owned.first().map(String::as_str),
        Some("rstest-bdd"),
        "the query's units must reach the description"
    );
    for target in super::witness::FIXTURE_TEST_BINARIES {
        assert!(
            owned.iter().any(|name| name == target),
            "the test target {target} must be owned beside the closure"
        );
    }
}

#[test]
fn the_query_runs_before_the_description_is_built() {
    // The composition every production caller reaches: `described_from` is
    // `described_with` over the family's inputs. Both halves record when they
    // run, so building the description ahead of the query — where slow
    // canonicalization would make the query's bound stale — fails here.
    let order = std::cell::RefCell::new(Vec::new());
    let budget = super::budget::Budget::starting_now(std::time::Duration::from_secs(3600));
    let error = super::budget::described_with(
        budget,
        |_bound| {
            order.borrow_mut().push("query");
            Ok(vec!["rstest-bdd".to_owned()])
        },
        |owned| {
            order.borrow_mut().push("description");
            // The builder is handed the query's answer, which is the other
            // half of the ordering: it has nothing to build from until then.
            Err(eyre::eyre!("stopped after receiving {} units", owned.len()))
        },
    )
    .expect_err("the recording builder stops the description");
    assert_eq!(
        *order.borrow(),
        ["query", "description"],
        "the query must run before the description is built"
    );
    assert!(
        format!("{error:#}").contains("stopped after receiving 3 units"),
        "the builder must receive the query's unit and both test targets: {error:#}"
    );
}
