//! Shared preparation policy for cold nested-Cargo fixture builds.
//!
//! Several integration suites drive real `cargo` invocations against fixture
//! crates. When a fixture has never been built, or its target directory has
//! been evicted, that build compiles a dependency graph from scratch inside
//! the test body, so the test's own deadline pays for work the test does not
//! assert on. `cargo-bdd::cli::list_steps_runs`,
//! `rstest-bdd::trybuild_macros::runner::step_macros_compile` and the
//! `feature_rebuild_invalidation` scenarios each exhausted their configured
//! limits that way.
//!
//! This module moves the cold work into explicitly named preparation tests
//! without widening any assertion deadline:
//!
//! * each preparation family writes a stamp inside the artefact directory it protects, and only
//!   after its last preparation command succeeded, so evicting the target directory also evicts the
//!   stamp;
//! * the stamp records a fingerprint of the build-relevant environment, the toolchain identity, a
//!   caller-supplied variant label and a caller-supplied hash of the fixture inputs, so a changed
//!   compiler, feature set or fixture source forces preparation again;
//! * under `cargo nextest` a missing or stale stamp fails fast with the command that prepares it,
//!   because nextest runs every test in its own process and a timed test cannot prepare without
//!   paying the cost it is meant to avoid;
//! * under plain `cargo test`, preparation runs lazily under a mutex shared by the threads of one
//!   test process, so the same suites stay usable without the extra Nextest pass. That mutex does
//!   not reach another process: under nextest each test has its own, and ordering across them comes
//!   from the `cargo-spawning` test group, so a family whose test binary shares a directory must
//!   stay in that group.
//!
//! Preparation commands are bounded by [`PREPARATION_COMMAND_TIMEOUT`], which
//! is deliberately separate from every assertion deadline. The pieces live in
//! private submodules: `stamp` holds the fingerprint and the stamp file,
//! `artefacts` decides whether a stamped tree is still there, `layout` finds
//! the directories Cargo put that tree in, `toolchain` resolves the compiler
//! identity, `input_hash` hashes fixture sources, `lifecycle` owns the gate
//! that decides whether to build, and `invalidate` withdraws a claim the gate
//! would otherwise honour. What a caller needs is re-exported here.

mod artefacts;
mod deadline;
mod input_hash;
mod invalidate;
mod layout;
mod lifecycle;
mod profiles;
mod sources;
mod stamp;
mod stamp_file;
mod toolchain;
pub use deadline::{Deadline, instruments_within};
pub use input_hash::{cargo_config_inputs, input_hash};
// The seams this crate's configuration cases vary instead of the process's
// environment; neither has a production caller.
#[cfg(test)]
pub(crate) use input_hash::{cargo_config_inputs_with, resolve_cargo_home};
pub use invalidate::{invalidate, invalidate_reported};
pub use lifecycle::{
    Admission,
    Instruments,
    PreparationOutcome,
    UnreadableStamp,
    admit,
    admit_with_probe,
    ensure_prepared_building,
    ensure_prepared_building_reported,
    prepare_now,
    prepare_now_reported,
};
pub use sources::{Directories, PreparationSummary, Sources};

#[cfg(test)]
mod fingerprint_props;
#[cfg(test)]
mod prop_tests;
#[cfg(test)]
mod tests;

use std::{
    env,
    io,
    path::{Path, PathBuf},
    sync::{Mutex, MutexGuard, PoisonError},
    time::Duration,
};

use super::{NestedCargoEnv, ProfileDestination, env_from_vars};

/// Wall-clock bound for one nested preparation command.
///
/// A preparation command compiles what the timed tests would otherwise
/// compile inside their own deadlines, so its bound is sized for a queued cold
/// build on a two-vCPU CI runner rather than for a warm assertion. It is not,
/// and must not become, the deadline of any test.
///
/// It must stay **below** the 30-minute `slow-timeout` that `.config/nextest.toml`
/// gives `nested_fixture_preparation`, even when a preparation test runs several
/// commands in sequence: a combined bound at or above the test's own budget
/// would let nextest kill the test first, discarding the captured child output
/// that [`run_bounded_with_timeout`] puts in the error. The worst case is
/// [`MAX_SEQUENTIAL_COMMANDS`] commands — the rebuild-invalidation preparation
/// compiles one fixture, then runs a metadata preflight and a second compile —
/// so nine minutes keeps three of them at 27 minutes, under the 30-minute
/// period.
///
/// Nine minutes is still far above any command measured on CI. The whole
/// three-command preparation test took 31.154 s on the cold run
/// `41981885921`, and a command cannot outlast the test that runs it, so every
/// command inside that test is covered many times over.
///
/// [`run_bounded_with_timeout`]: super::run_bounded_with_timeout
pub const PREPARATION_COMMAND_TIMEOUT: Duration = Duration::from_secs(9 * 60);

/// The most bounded commands one preparation test runs in sequence.
///
/// `.config/nextest.toml` gives the preparation tests a single `slow-timeout`
/// period, and the per-command bound above must keep this many commands inside
/// it. The rebuild-invalidation preparation is the worst case: it compiles one
/// fixture, then runs the addition fixture's metadata preflight and its
/// compile. Every other family runs a single bounded command. A new family
/// that runs more must raise this and shrink [`PREPARATION_COMMAND_TIMEOUT`] to
/// match, which `preparation_command_bound_fires_before_the_runner_kills_the_test`
/// enforces rather than trusting this comment.
pub const MAX_SEQUENTIAL_COMMANDS: u64 = 3;

/// Mutex serializing lazy preparation between threads of one test process.
///
/// It is *process-wide*, not cross-process: `cargo test` runs every test of a
/// binary on threads of one process, so a `Mutex` is what serializes them, and
/// a second process would not see it at all. Under `cargo nextest` each test is
/// its own process, so ordering across processes comes from the
/// `cargo-spawning` test group in `.config/nextest.toml` instead. Do not
/// describe this lock as serializing independent processes; it does not, and a
/// family that relied on it to would run two nested builds at once.
static PREPARE_LOCK: Mutex<()> = Mutex::new(());

/// Cargo's development profile directory, relative to a target directory.
///
/// Every nested build here runs without `--release` or `--profile`, so its
/// compiled units land below `<target>/debug`. This is the directory a timed
/// assertion reuses, and it is not always inside the directory the stamp is
/// written to: for the scratch families it is a sibling of the stamp root, so
/// [`Preparation::new`] names it directly, while for a family whose stamped
/// directory *is* its child's `CARGO_TARGET_DIR` it is a descendant, and
/// [`Preparation::for_fixture`] vouches for the whole containing directory
/// instead.
pub const DEFAULT_PROFILE: &str = "debug";

/// One family of prepared fixture artefacts.
#[derive(Debug, Clone)]
pub struct Preparation {
    /// Stable kebab-case family name, used in the stamp file name and logs.
    pub family: String,
    /// Directory the prepared artefacts live in, and where the stamp is
    /// written.
    pub directory: PathBuf,
    /// The filtered environment the preparation commands run with.
    pub environment: NestedCargoEnv,
    /// Caller-supplied label for the feature set or tool variant, such as
    /// `all-features` or the resolved Clippy version.
    pub variant: String,
    /// Caller-supplied hash of the fixture sources and lockfiles.
    pub inputs: String,
    /// Directories whose contents the stamp vouches for.
    ///
    /// A stamp lives at [`Preparation::directory`], which for the scratch
    /// families is their own root — `<target>/tests/<family>`. The compiled
    /// units a timed assertion actually reuses are written to Cargo's profile
    /// directory, `<target>/debug`, and for those families that is a *sibling*
    /// of the stamp's directory rather than a descendant of it. Evicting only
    /// `<target>/debug` therefore leaves the stamp intact while removing the
    /// dependency graph, so a stamp alone cannot answer "are the artefacts
    /// still there".
    ///
    /// The gate requires every directory listed here to exist. A missing one
    /// refuses the preparation exactly as a missing stamp does, which is what
    /// keeps a timed assertion from rebuilding the graph inside its deadline.
    ///
    /// A build selected onto a target triple writes two profiles, and both are
    /// listed. See `profiles::profiles` in this module's private submodules for
    /// which ones and why.
    pub artefacts: Vec<PathBuf>,
    /// Cargo package names whose compiled units this family's stamp vouches
    /// for by name.
    ///
    /// A profile can be shared. The rebuild-invalidation and feature-addition
    /// families both build into the workspace `target/debug`, and the CLI
    /// family stamps that same profile, so a family that recorded every name
    /// it found there vouched for its siblings' units too. The rebuild
    /// experiment then runs `cargo clean -p rstest-bdd-rebuild-invalidation-fixture`
    /// before each baseline *by design*, which removes exactly those units —
    /// and no name-level rule can tell that apart from an eviction, because
    /// the replacement generations that follow are themselves recorded. The
    /// sibling's stamp was refused for ever, and the timed assertion it gates
    /// could never run.
    ///
    /// Naming the packages is what keeps the claim to what the family built.
    /// Each family records its own fixture's units exactly, and leaves a
    /// sibling's units to the sibling's own stamp. The dependency graph every
    /// family shares stays covered by the structural layer, which refuses a
    /// profile that has lost its units or its fingerprint store — the states
    /// `cargo clean`, `rm -rf <target>/debug` and a cache eviction leave.
    ///
    /// The list must name **every package the preparation built**, not only
    /// the fixture: one `cargo build` of a path-dependency fixture also builds
    /// the workspace crates that fixture depends on, and Cargo rebuilds a
    /// dependency whose fingerprint record is gone. Declaring the fixture
    /// alone filters those records out of both what is recorded and what is
    /// found, so an evicted dependency compares equal to itself and passes the
    /// gate — the rebuild then happens inside the timed assertion the gate
    /// exists to protect. Only the caller knows the dependency list, which is
    /// why it is declared here rather than derived.
    ///
    /// A name here is a Cargo *target* name as well as a package name, because
    /// the two need not agree: a package called `adapter` may declare
    /// `[lib] name = "engine"`, and its output is then
    /// `deps/libengine-<hash>.rlib` while its fingerprint records stay under
    /// `.fingerprint/adapter-<hash>/`. The records would be owned and the
    /// output would not, so removing only the output is invisible. Such a
    /// caller must list `engine` as well; a package name covers the targets
    /// that share it, which is every target in this workspace.
    ///
    /// Empty means the family owns the whole profile, which is right for
    /// [`Preparation::for_fixture`]: such a family sets `CARGO_TARGET_DIR` to
    /// the directory it stamps, so no other family writes there.
    pub owned_packages: Vec<String>,
    /// Directory the nested build commands run in.
    ///
    /// Cargo resolves a relative `CARGO_HOME` and a relative `RUSTC` against
    /// the working directory of the command that reads them. The identity
    /// probe and the configuration walk run in *this* process's directory
    /// unless they are told otherwise, so without this the fingerprint could
    /// describe one Cargo home or compiler while the nested build used
    /// another — the difference being visible only as a timed rebuild.
    ///
    /// Supplied explicitly to every constructor. It is deliberately not
    /// derived from [`Preparation::directory`]: that is where the stamp is
    /// written, and most families build somewhere else. A `trybuild` family
    /// builds the generated crate in a subdirectory of the stamped root, and
    /// the UI-lint family runs from `CARGO_MANIFEST_DIR`, so a derived value
    /// recorded the wrong compiler whenever a relative override was in play.
    pub working_directory: PathBuf,
}

impl Preparation {
    /// Build a preparation description.
    ///
    /// The artefact directory is derived from `environment`, not from
    /// `directory`: [`Preparation::directory`] is where the stamp is written,
    /// while the compiled units land in the profile directory of the target
    /// directory the *child* resolves. Deriving both from one value is what
    /// keeps them in step when the target directory moves.
    ///
    /// # Parameters
    ///
    /// - `sources`: the family name, variant label and input hash.
    /// - `directories`: the stamp's root and the directory the nested build runs in; see
    ///   [`Directories`].
    /// - `environment`: the filtered environment the nested commands run with, which also names the
    ///   target directory the artefacts are derived from.
    ///
    /// # Returns
    ///
    /// A description whose artefact roots are the target directory's selected
    /// profiles, or empty when the environment names no target root.
    ///
    /// # Errors
    ///
    /// As [`NestedCargoEnv::target_directory`]: a relative target directory
    /// that cannot be resolved is reported rather than recorded as a relative
    /// artefact path, which the gate would then check against the wrong root.
    pub fn new(
        sources: Sources,
        directories: Directories,
        environment: NestedCargoEnv,
    ) -> io::Result<Self> {
        // The working directory is passed on because Cargo resolves a
        // path-valued `--target` against it before naming the profile: the
        // nested builds run in `directories.working`, not in this process's
        // directory, so a relative specification file names a profile there.
        let artefacts = environment
            .target_directory()?
            .map(|target| profiles::profiles(&target, &environment, &directories.working))
            .unwrap_or_default();
        Ok(Self {
            family: sources.family,
            working_directory: directories.working,
            directory: directories.root,
            environment,
            variant: sources.variant,
            inputs: sources.inputs,
            artefacts,
            owned_packages: Vec::new(),
        })
    }

    /// Describe a family whose nested builds all run against `directory`.
    ///
    /// A family's code path sets `CARGO_TARGET_DIR` to exactly the directory
    /// it stamps, so recording that directory as the child's target is what
    /// keeps the stamp and the artefacts it vouches for in step. The rest of
    /// the inherited environment is filtered by the shared rules, with the
    /// caller's profile destination preserved.
    ///
    /// Such a family owns its target directory, so it vouches for the whole of
    /// it rather than for one profile inside it. The profiles Cargo writes are
    /// not a fixed set: a build without `--target` writes `<root>/debug`, while
    /// `trybuild` passes `--target <triple>` and writes the host units to
    /// `<root>/debug` and everything else to `<root>/<triple>/debug`. Naming
    /// one would leave the other unvouched for, so evicting only that one would
    /// leave the stamp matching and the timed assertion would rebuild the graph
    /// inside its deadline — the failure this module exists to prevent. The
    /// same applies to the check-mode units `cargo check` writes beside the
    /// build-mode ones. `layout::discover` walks whatever is there, so the
    /// recorded names cover every profile the preparation left without this
    /// function predicting them.
    ///
    /// `inputs` is the caller-supplied hash of the fixture sources; callers
    /// derive it before preparation runs, because preparation writes inside
    /// the very roots it hashes. Coverage stays with the caller: the fixture's
    /// own run is what the coverage gate measures, so an inherited
    /// `LLVM_PROFILE_FILE` is kept.
    ///
    /// # Parameters
    ///
    /// - `sources`: the family name, variant label and input hash, as [`Sources::new`].
    /// - `directories`: the stamped root, which is also the target directory the child receives,
    ///   and the directory the nested build actually runs in. A `trybuild` family builds the
    ///   generated crate rather than the stamped root, so callers pass
    ///   `TrybuildTree::generated_crate_root` as the working directory and a UI-lint family passes
    ///   its manifest directory. See [`Preparation::working_directory`] for why this must not be
    ///   derived.
    ///
    /// # Returns
    ///
    /// A description that vouches for the whole of the stamped root, since a
    /// family constructed this way owns its target directory.
    ///
    /// # Errors
    ///
    /// As [`Preparation::new`].
    pub fn for_fixture(sources: Sources, directories: Directories) -> io::Result<Self> {
        let environment = env_for_directory(&directories.root, ProfileDestination::Caller)?;
        let mut preparation = Self::new(sources, directories, environment)?;
        preparation.artefacts = vec![preparation.directory.clone()];
        Ok(preparation)
    }
}

/// Filter the inherited environment for a child that builds in `directory`.
///
/// `CARGO_TARGET_DIR` is dropped from the inherited values because the
/// directory being stamped is the authority: a stale inherited value would
/// point the build somewhere the stamp does not describe. Callers that only
/// need an environment for a nested build should reach for
/// [`super::build_child_env`] or [`env_from_vars`] instead: this helper serves
/// [`Preparation::for_fixture`], because dropping the inherited value is a
/// decision about what a *stamp* describes rather than a general filtering
/// rule.
///
/// # Parameters
/// - `directory`: The directory being stamped, which supplies the `CARGO_TARGET_DIR` that replaces
///   the dropped value.
/// - `profile_destination`: Where the child's profile output goes, passed to [`env_from_vars`]
///   unchanged.
///
/// # Returns
/// The inherited environment less `CARGO_TARGET_DIR`, plus the resolved target
/// directory: what the nested build receives, not what this process holds.
///
/// # Errors
///
/// As [`env_from_vars`]. The dropped variable is re-supplied from `directory`,
/// so this reports only when `directory` itself is relative and the working
/// directory cannot be read.
pub fn env_for_directory(
    directory: &Path,
    profile_destination: ProfileDestination,
) -> io::Result<NestedCargoEnv> {
    env_from_vars(
        env::vars_os()
            .filter(|(key, _)| key != "CARGO_TARGET_DIR")
            .collect(),
        Some(directory),
        profile_destination,
    )
}

/// Take the preparation lock.
///
/// Shared with `lifecycle`, which is the only caller: the lock's purpose is to
/// serialize the lazy path between threads of this process, and that path lives
/// there. It stays in this module because the lock itself does, and a second
/// `Mutex` would serialize nothing. It is deliberately not a cross-process
/// lock; see [`PREPARE_LOCK`] for what serializes the suites under nextest.
pub(super) fn lock() -> MutexGuard<'static, ()> {
    PREPARE_LOCK.lock().unwrap_or_else(PoisonError::into_inner)
}
