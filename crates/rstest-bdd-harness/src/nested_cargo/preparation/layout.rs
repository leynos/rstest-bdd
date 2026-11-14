//! Where Cargo actually put the units, and how to find them again.
//!
//! Cargo's build-directory layout is explicitly not a stable interface, and the
//! release that changed it said so: 1.100.0 moved every compiled unit from
//! `<profile>/deps` to `<profile>/build/<pkg>/<hash>/out`, keeping the same
//! files under a different shape, with no stable opt-out. Naming a directory is
//! therefore not something this module can do. It discovers them instead.
//!
//! What it relies on is rustc's contract rather than Cargo's placement: rustc
//! writes a unit's dep-info record into the `--out-dir` that holds that unit's
//! outputs. A directory containing `.d` records is therefore a directory holding
//! units, whatever Cargo called it and wherever Cargo put it, and the outputs
//! that unit produced are the same-stem entries beside its record. That holds in
//! both layouts this workspace builds under, which is what makes the check
//! survive the change that broke its predecessor.
//!
//! Two structural facts are still required of a prepared profile, because each
//! is what makes Cargo skip a rebuild: it must hold at least one such unit
//! directory, and it must hold at least one non-empty fingerprint store — the
//! directory Cargo reads to decide those units are still current.

use std::{
    collections::BTreeSet,
    ffi::OsStr,
    fs,
    io,
    path::{Path, PathBuf},
};

/// Cargo's incremental-compilation cache.
///
/// It holds scratch state Cargo regenerates rather than reusable units, and on
/// a real workspace it is the largest directory under a profile by an order of
/// magnitude. Pruned so the walk costs one listing per real unit directory
/// instead of tens of thousands of entries.
const INCREMENTAL: &str = "incremental";

/// The names a fingerprint store directory is given.
///
/// A profile-level store is `.fingerprint`; the per-unit store inside
/// `build/<pkg>/<hash>` is `fingerprint`. Both hold the records Cargo reads to
/// decide a unit is current, so either satisfies the structural check.
const FINGERPRINT_STORE_DIRS: [&str; 2] = ["fingerprint", ".fingerprint"];

/// How far below the profile the discovery walk descends.
///
/// Measured against both layouts: `<profile>/deps` is one level down,
/// `<profile>/build/<pkg>-<hash>` two, and `<profile>/build/<pkg>/<hash>/out`
/// four. The bound is deliberately generous rather than exact, so a layout that
/// gains a level is still discovered; it exists only to keep the walk away from
/// a symlinked cycle, which would otherwise not terminate.
const MAX_DEPTH: usize = 6;

/// One directory's entry names and its immediate subdirectories.
struct Listing {
    /// Every entry name, sorted.
    names: Vec<String>,
    /// The entries that are directories, in listing order.
    subdirectories: Vec<PathBuf>,
}

/// The directories a profile actually holds its work in.
#[derive(Debug, Default)]
pub(super) struct Layout {
    /// Directories holding at least one rustc dep-info record.
    pub(super) units: Vec<PathBuf>,
    /// Non-empty directories holding Cargo's fingerprint records.
    pub(super) fingerprints: Vec<PathBuf>,
}

impl Layout {
    /// Whether this layout still holds what a preparation needs.
    ///
    /// Both halves are required. A profile with units but no fingerprint store
    /// is one Cargo rebuilds, because it holds no record saying those units are
    /// current; a profile with a store but no units is one with nothing for a
    /// record to speak about. Either state costs the timed assertion the cold
    /// build this check exists to keep out of it.
    pub(super) fn is_usable(&self) -> bool {
        !self.units.is_empty() && !self.fingerprints.is_empty()
    }
}

/// Discover the unit and fingerprint directories below `profile`.
///
/// A profile that does not exist is reported as an absent layout — no units, no
/// store — rather than as an error: that is the ordinary eviction this check
/// exists to name, and a bare `No such file or directory` would replace a
/// precise diagnosis with a filesystem message.
///
/// # Errors
///
/// Returns the read error for any failure other than absence. An unreadable
/// directory must refuse the preparation rather than be read as an empty one,
/// which is the unsafe direction: it would let a tree that cannot be inspected
/// pass as a prepared tree.
pub(super) fn discover(profile: &Path) -> io::Result<Layout> {
    let mut layout = Layout::default();
    let mut pending = vec![(profile.to_path_buf(), 0_usize)];
    while let Some((directory, depth)) = pending.pop() {
        // `depth == 0` is the profile itself, so its children are the level
        // Cargo's incremental cache lives at.
        let Some(listing) = list_or_absent(&directory, depth == 0)? else {
            // Only the profile itself can be legitimately absent.
            return Ok(layout);
        };
        classify(&directory, &listing, &mut layout);
        if depth < MAX_DEPTH {
            pending.extend(
                listing
                    .subdirectories
                    .into_iter()
                    .map(|subdirectory| (subdirectory, depth + 1)),
            );
        }
    }
    // Sorted so the record built from these paths — and therefore the stamp
    // written from it — is stable across runs. `read_dir` order is the
    // filesystem's, and an unsorted list would rewrite the stamp every time.
    layout.units.sort();
    layout.fingerprints.sort();
    Ok(layout)
}

/// List `directory`, reporting an absent profile as `None`.
///
/// Absence is only tolerated at the profile itself; a missing subdirectory
/// mid-walk is a read fault, because a tree that changed under the walk cannot
/// be vouched for.
///
/// # Errors
///
/// As [`listing_of`], for every failure other than an absent profile.
fn list_or_absent(directory: &Path, is_profile: bool) -> io::Result<Option<Listing>> {
    match listing_of(directory, is_profile) {
        Ok(listing) => Ok(Some(listing)),
        Err(error) if error.kind() == io::ErrorKind::NotFound && is_profile => Ok(None),
        Err(error) => Err(error),
    }
}

/// Record what `directory` contributes to `layout`.
///
/// Two independent questions, neither of which can shorten the other: whether
/// the directory holds units, and whether it is a populated fingerprint store.
fn classify(directory: &Path, listing: &Listing, layout: &mut Layout) {
    if listing
        .names
        .iter()
        .any(|name| dep_info_stem(name).is_some())
    {
        layout.units.push(directory.to_path_buf());
    }
    if is_fingerprint_store(directory) && !listing.names.is_empty() {
        layout.fingerprints.push(directory.to_path_buf());
    }
}

/// Whether `entry` is a directory the walk should descend into.
///
/// Split out of [`listing_of`] as its own predicate so the loop body stays one
/// branch: the Whitaker suite's `conditional_max_n_branches` refuses an
/// `if` that nests three tests, and naming the rule makes it checkable.
///
/// `prune_incremental` is only true for the profile's own children, so the
/// cache is excluded at the one level Cargo keeps it and nowhere else.
///
/// # Errors
///
/// Returns the read error when the entry's file type cannot be read.
fn is_walkable(entry: &fs::DirEntry, prune_incremental: bool) -> io::Result<bool> {
    if !entry.file_type()?.is_dir() {
        return Ok(false);
    }
    Ok(!(prune_incremental && entry.file_name() == INCREMENTAL))
}

/// Read one directory's entry names and its immediate subdirectories.
///
/// Every iteration step is propagated with `?`. `read_dir` yields its entries
/// lazily, so a fault partway through the listing arrives from the loop rather
/// than from the call, and it must refuse the preparation rather than be
/// counted as an entry — the same fail-open direction the caller's error arm
/// guards.
///
/// Subdirectories are identified by the entry's own file type, without a second
/// `stat`: a profile can hold thousands of entries and this runs once per
/// preparation.
///
/// # Errors
///
/// Returns the read error when the directory cannot be listed, or an entry's
/// type or name cannot be read.
fn listing_of(directory: &Path, prune_incremental: bool) -> io::Result<Listing> {
    let mut names = Vec::new();
    let mut subdirectories = Vec::new();
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        if is_walkable(&entry, prune_incremental)? {
            subdirectories.push(entry.path());
        }
        names.push(entry.file_name().to_string_lossy().into_owned());
    }
    names.sort();
    Ok(Listing {
        names,
        subdirectories,
    })
}

/// Whether `directory` is named as one of Cargo's fingerprint stores.
fn is_fingerprint_store(directory: &Path) -> bool {
    directory
        .file_name()
        .and_then(OsStr::to_str)
        .is_some_and(|name| FINGERPRINT_STORE_DIRS.contains(&name))
}

/// The entry names in `directory`, sorted.
///
/// Sorting keeps the recorded list — which becomes part of the stamp — stable
/// across runs, for the reason [`discover`] gives.
///
/// # Errors
///
/// Returns the read error when the directory cannot be listed or an entry
/// cannot be named.
pub(super) fn entries_of(directory: &Path) -> io::Result<Vec<String>> {
    let mut names: Vec<String> = fs::read_dir(directory)?
        .map(|entry| entry.map(|entry| entry.file_name().to_string_lossy().into_owned()))
        .collect::<io::Result<_>>()?;
    names.sort();
    Ok(names)
}

/// Split a dep-info file name into its unit stem.
///
/// Returns `None` for anything that is not a `.d` record, so an unrelated entry
/// in the directory — an `.rlib`, a `.rmeta`, a build script's output — is never
/// mistaken for one.
fn dep_info_stem(name: &str) -> Option<&str> { name.strip_suffix(".d") }

/// The outputs in `present` that belong to the unit with `stem`.
///
/// This is a *rule about rustc's naming*, not a list of extensions, and it is
/// used only to discover which entries belong together — never to decide that a
/// tree is usable. An earlier version used it for the decision itself and was
/// wrong: see the `artefacts` module documentation.
///
/// The rule uses what is stable: rustc names a library output
/// `lib<stem>.<something>` and a linked binary `<stem>` or `<stem>.<something>`
/// (`.exe` on Windows). Anchoring on the stem and requiring a separator keeps
/// the pairing from matching a different unit whose name merely starts with the
/// same characters.
fn is_output_of(name: &str, stem: &str) -> bool {
    // A record's own name must not count as its output: `unit.d` matches the
    // binary form because `.d` is a dot-extension, so a unit with no output
    // would otherwise appear to witness itself.
    if name == format!("{stem}.d") {
        return false;
    }
    // A library: `lib<stem>` followed by an extension.
    let library = name
        .strip_prefix("lib")
        .and_then(|rest| rest.strip_prefix(stem))
        .is_some_and(|extension| extension.starts_with('.'));
    // A linked binary: exactly `<stem>`, or `<stem>.<extension>`.
    let binary = name
        .strip_prefix(stem)
        .is_some_and(|rest| rest.is_empty() || rest.starts_with('.'));
    library || binary
}

/// The outputs a directory's dep-info records describe.
///
/// These are the units Cargo will recompile if their outputs go missing, so they
/// are exactly the names a preparation must be able to vouch for. The list is
/// one directory listing, reused for every record: a real unit directory holds
/// thousands of entries and this runs once per preparation.
///
/// # Errors
///
/// Returns the read error when the directory cannot be listed.
pub(super) fn outputs_in(directory: &Path) -> io::Result<Vec<String>> {
    let present = entries_of(directory)?;
    let mut outputs: BTreeSet<String> = BTreeSet::new();
    for stem in present.iter().filter_map(|name| dep_info_stem(name)) {
        outputs.extend(
            present
                .iter()
                .filter(|name| is_output_of(name, stem))
                .cloned(),
        );
    }
    Ok(outputs.into_iter().collect())
}

#[cfg(test)]
mod tests;
