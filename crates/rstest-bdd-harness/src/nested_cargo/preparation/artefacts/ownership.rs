//! Which recorded names a family's stamp is entitled to speak about.
//!
//! A Cargo profile can be shared. This workspace's `target/debug` is written by
//! the integration-test binaries themselves, by the `cargo-bdd` CLI fixture,
//! and by two scratch fixtures that the rebuild-invalidation suite builds and
//! then deliberately evicts with `cargo clean -p`. A family that recorded every
//! name it found in that profile therefore vouched for work it never did, and
//! the deliberate eviction looked exactly like the accident the check exists to
//! catch.
//!
//! The distinguishing fact cannot be found in the tree at check time. At that
//! point a unit is either present or not, and a unit removed by a sibling's
//! designed clean is indistinguishable from one removed by a stray `rm`. What
//! separates them is *provenance*: the sibling's units were produced by the
//! sibling's preparation command, and this family's stamp has no claim on them.
//!
//! Only the caller knows which packages its own preparation builds, so it
//! declares them (see [`Preparation::owned_packages`]). This module answers the
//! matching question: whether a recorded name belongs to one of those packages.
//!
//! The rest of the profile is not abandoned. [`super::verify_structural`] still
//! refuses a profile that has lost its units or its fingerprint store, which is
//! the state `cargo clean`, `rm -rf <target>/debug` and a cache eviction leave.
//!
//! [`Preparation::owned_packages`]: super::super::Preparation::owned_packages

/// Whether `name` names a unit this family built.
///
/// `owned` is empty for a family that owns its whole target directory, and
/// every name is then its own — see [`super::super::Preparation::for_fixture`].
///
/// Each path component is examined, because Cargo puts the package name in
/// more than one place: a unit's output is `deps/<stem>-<hash>.<ext>`, while its
/// fingerprint records are filed under `.fingerprint/<package>-<hash>/`. A
/// nested `build/<package>-<hash>/out` adds a third. Matching any component is
/// what lets one rule cover all three without this module having to know which
/// layout Cargo is using today.
pub(super) fn name_is_owned(name: &str, owned: &[String]) -> bool {
    if owned.is_empty() {
        return true;
    }
    name.split('/').any(|part| {
        let stem = strip_unit_suffix(part);
        owned.iter().any(|package| package_matches(stem, package))
    })
}

/// Remove Cargo's `-<hash>` metadata suffix and any output extension.
///
/// Cargo names a unit `<stem>-<16 hex digits>`, so the last `-` separates the
/// stem from the hash. An extension follows on an output (`libx-<hash>.rlib`)
/// but not on a directory, so both are removed here and the stem is what
/// remains.
///
/// A component that does not carry that shape is returned unchanged: it is not
/// a unit name, and a caller comparing it against a package name will simply
/// not match.
fn strip_unit_suffix(part: &str) -> &str {
    let without_extension = match part.rsplit_once('.') {
        // `.d`, `.rlib`, `.rmeta`, `.json`: an output form rather than part of
        // the stem. A leading dot (`invoked.timestamp` is not, but a hidden
        // name could be) has no stem before it, so the whole component stands.
        Some((stem, _)) if !stem.is_empty() => stem,
        _ => part,
    };
    match without_extension.rsplit_once('-') {
        Some((stem, hash)) if is_metadata_hash(hash) => stem,
        _ => without_extension,
    }
}

/// Whether `text` is exactly the 16 hexadecimal characters Cargo hashes with.
fn is_metadata_hash(text: &str) -> bool {
    text.len() == 16 && text.bytes().all(|byte| byte.is_ascii_hexdigit())
}

/// Whether a unit stem names `package`.
///
/// Cargo hyphenates a package in directory names and underscores it in the
/// crate name it derives, so both spellings have to match: a package
/// `rstest-bdd-feature-addition-fixture` appears as
/// `.fingerprint/rstest-bdd-feature-addition-fixture-<hash>/` and as
/// `lib.rstest_bdd_feature_addition_fixture-<hash>.rlib`. A library output adds
/// the `lib` prefix on top of that.
fn package_matches(stem: &str, package: &str) -> bool {
    let underscored = package.replace('-', "_");
    stem == package
        || stem == underscored
        || stem.strip_prefix("lib") == Some(underscored.as_str())
        || stem.strip_prefix("lib") == Some(package)
}

#[cfg(test)]
mod tests;
