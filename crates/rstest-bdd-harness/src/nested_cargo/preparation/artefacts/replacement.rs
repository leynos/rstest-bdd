//! When a recorded artefact name has been superseded rather than evicted.
//!
//! Cargo names every compiled unit `<stem>-<hash>`, where the hash covers the
//! unit's metadata. Recompiling a crate that something else invalidated
//! therefore replaces `libx-<old>.rlib` with `libx-<new>.rlib` and removes the
//! old name, so *every* name a stamp recorded for that unit looks absent at
//! once even though the profile still holds the unit.
//!
//! That distinction matters here because the scratch families stamp the shared
//! `<target>/debug`: a family's stamp also records its sibling fixture's units,
//! and the rebuild experiment deliberately runs
//! `cargo clean -p rstest-bdd-rebuild-invalidation-fixture` before rebuilding.
//! Holding the recorded names unconditionally made the sibling's stamp fail
//! every later gate with four "missing" names that had merely been superseded.
//!
//! Excusing anything with a matching stem would reopen the hole the exact layer
//! exists to close, so a replacement must also carry the *same output form*:
//! an `.rlib` that went must be answered by another `.rlib` for that stem, never
//! by a surviving `.rmeta`. That keeps
//! `an_rlib_removed_beside_a_surviving_rmeta_is_an_eviction` refusing, because
//! no second `.rlib` exists there, while a recompiled unit passes.

/// Whether `recorded` is still answered for by what the profile now holds.
///
/// Exact presence is the ordinary answer: the unit the preparation left is
/// still there under the same name. The other answer is a recompile, described
/// by this module's documentation.
pub(super) fn is_vouched_for(recorded: &str, present: &[String]) -> bool {
    present.iter().any(|name| name == recorded) || superseded(recorded, present)
}

/// Whether some present name is a different build of the unit `recorded` names.
///
/// Both names must share a directory, a stem and an output form, and differ in
/// the metadata hash Cargo put between them. Sharing the form is what keeps a
/// surviving `.rmeta` from answering for a `.rlib` that was removed.
fn superseded(recorded: &str, present: &[String]) -> bool {
    let Some(name) = UnitName::parse(recorded) else {
        return false;
    };
    present.iter().any(|candidate| {
        UnitName::parse(candidate).is_some_and(|other| {
            other.directory == name.directory
                && other.stem == name.stem
                && other.suffix == name.suffix
                && other.hash != name.hash
        })
    })
}

/// A Cargo unit output name, split into the parts that identify the unit.
///
/// Split so a recompiled unit can be recognised: everything except the hash
/// stays fixed across a recompile, and the hash is precisely what changes.
/// Names that do not carry Cargo's `<stem>-<hash><suffix>` shape — a build
/// script's output directory entry, a fingerprint record — parse as `None` and
/// are therefore never treated as a replacement for anything.
struct UnitName<'a> {
    /// The directory below the profile that holds the unit, as recorded.
    directory: &'a str,
    /// The stem Cargo derives from the crate name and target.
    stem: &'a str,
    /// The 16 hexadecimal characters Cargo hashes the unit's metadata into.
    hash: &'a str,
    /// Everything after the hash: the extension, and any further tail.
    suffix: &'a str,
}

impl<'a> UnitName<'a> {
    /// Split `name` into its directory, stem, hash and suffix.
    ///
    /// The hash is Cargo's 16-character lowercase hexadecimal string. Requiring
    /// exactly that shape is what makes the split trustworthy: a stem that
    /// itself ends in something hex-like is separated at the final `-` only
    /// when the text after it is precisely a hash.
    fn parse(name: &'a str) -> Option<Self> {
        let (directory, file) = name.rsplit_once('/').unwrap_or(("", name));
        let (stem, rest) = file.rsplit_once('-')?;
        // `split_at_checked` refuses a short tail, so the slice below is exactly
        // `HASH_LEN` characters and only the characters need checking.
        let (hash, suffix) = rest.split_at_checked(HASH_LEN)?;
        if !hash.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return None;
        }
        Some(Self {
            directory,
            stem,
            hash,
            suffix,
        })
    }
}

/// The length of the metadata hash Cargo puts in a unit name.
const HASH_LEN: usize = 16;

#[cfg(test)]
mod tests;
