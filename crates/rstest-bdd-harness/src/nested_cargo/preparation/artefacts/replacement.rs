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
//! The rule covers the fingerprint records a stamp also holds, without a second
//! code path. Cargo files a unit's records under a directory named after the
//! unit, so a recompile renames that directory and leaves the file names inside
//! it alone. Naming the artefact by what surrounds its name — the directories
//! above it and the file names below it — therefore answers for a compiled
//! output and for a fingerprint record at once.
//!
//! Excusing anything with a matching stem would reopen the hole the exact layer
//! exists to close, so a replacement must also carry the *same surroundings*:
//! an `.rlib` that went must be answered by another `.rlib` for that stem, never
//! by a surviving `.rmeta`. That keeps
//! `an_rlib_removed_beside_a_surviving_rmeta_is_an_eviction` refusing, because
//! no second `.rlib` exists there, while a recompiled unit passes.
//!
//! Nor may a replacement be a name the stamp itself recorded. A profile can
//! hold two builds of one crate under different hashes, and if one of those
//! `.rlib` files then goes, its recorded sibling is not a replacement: it is a
//! separate artefact the stamp already accounts for. Answering from it would
//! report nothing missing while the profile had lost a compiled unit.

/// Whether `recorded` is still answered for by what the profile now holds.
///
/// Exact presence is the ordinary answer: the unit the preparation left is
/// still there under the same name. The other answer is a recompile, described
/// by this module's documentation. `replacements` must exclude every name the
/// stamp recorded, so a recorded sibling cannot answer for a name that also
/// went.
pub(super) fn is_vouched_for(recorded: &str, present: &[String], replacements: &[String]) -> bool {
    present.iter().any(|name| name == recorded) || superseded(recorded, replacements)
}

/// Whether some name in `replacements` is a different build of `recorded`.
///
/// Both names must agree on everything outside the unit's own name — the
/// directories holding it and the file names beneath it — and differ there.
/// Requiring the surroundings to match is what keeps a surviving `.rmeta` from
/// answering for a `.rlib` that was removed, and what keeps a same-stem unit
/// elsewhere from answering at all.
fn superseded(recorded: &str, replacements: &[String]) -> bool {
    let Some(name) = UnitPath::parse(recorded) else {
        return false;
    };
    replacements
        .iter()
        .filter_map(|candidate| UnitPath::parse(candidate))
        .any(|other| other.replaces(&name))
}

/// A recorded artefact name, split around the Cargo unit component inside it.
///
/// The unit is the last path component carrying Cargo's `<stem>-<hash>` shape:
/// a compiled output's file name, or the directory a unit's fingerprint records
/// were filed in. Everything else is kept verbatim, because everything else has
/// to stay fixed across a recompile — the directories Cargo placed the artefact
/// in, and the file names it wrote beneath the unit.
struct UnitPath<'a> {
    /// The text above the unit component, as recorded.
    above: &'a str,
    /// The stem Cargo derives from the crate name and target.
    stem: &'a str,
    /// The 16 hexadecimal characters Cargo hashes the unit's metadata into.
    hash: &'a str,
    /// Everything after the hash: the output's extension, and any further tail.
    suffix: &'a str,
    /// The text below the unit component, as recorded.
    below: &'a str,
}

impl<'a> UnitPath<'a> {
    /// Split `name` around its last unit component.
    ///
    /// Returns `None` when no component carries that shape. That is the
    /// ordinary state for an unhashed name: there is nothing Cargo could have
    /// renamed, so nothing may be read as its replacement.
    fn parse(name: &'a str) -> Option<Self> {
        // The components are walked with their byte offsets so the parts stay
        // slices of the recorded name. Rebuilding them would reintroduce a
        // separator the recorded form may not have had, and the recorded form
        // is what the stamp compares.
        //
        // The match is the *last* such component, because the unit is the
        // innermost one: a store named after a unit can itself sit below a
        // package directory that carries a hash, and only the inner name moves
        // when the unit is rebuilt.
        let mut offset = 0_usize;
        let mut found: Option<(usize, usize, UnitName<'a>)> = None;
        for part in name.split('/') {
            let end = offset + part.len();
            if let Some(unit) = UnitName::parse(part) {
                found = Some((offset, end, unit));
            }
            // Every component is followed by exactly one separator.
            offset = end + 1;
        }
        let (start, end, unit) = found?;
        // Everything above the unit is the text before its component; the text
        // below it starts after the separator that follows. A unit at either
        // end of the name therefore has an empty part on that side, which is
        // what makes the comparison exact for every shape.
        let above = start.checked_sub(1).and_then(|end| name.get(..end));
        let below = name.get(end + 1..);
        Some(Self {
            above: above.unwrap_or_default(),
            stem: unit.stem,
            hash: unit.hash,
            suffix: unit.suffix,
            below: below.unwrap_or_default(),
        })
    }

    /// Whether this path is a different build of `other`'s artefact.
    ///
    /// Everything but the metadata hash must agree. The text around the unit is
    /// where Cargo put the artefact and what it named the file beneath it; the
    /// stem and the suffix are what the unit is and which output form it
    /// produced. A change to any of those names a different artefact rather
    /// than the same one rebuilt.
    fn replaces(&self, other: &Self) -> bool {
        self.above == other.above
            && self.below == other.below
            && self.stem == other.stem
            && self.suffix == other.suffix
            && self.hash != other.hash
    }
}

/// One unit component, split into the parts a recompile keeps and the one it
/// changes.
struct UnitName<'a> {
    /// The stem Cargo derives from the crate name and target.
    stem: &'a str,
    /// The 16 hexadecimal characters Cargo hashes the unit's metadata into.
    hash: &'a str,
    /// Everything after the hash.
    suffix: &'a str,
}

impl<'a> UnitName<'a> {
    /// Split `part` into its stem, hash and suffix.
    ///
    /// The hash is Cargo's 16-character hexadecimal string. Requiring exactly
    /// that shape is what makes the split trustworthy: a stem that itself ends
    /// in something hex-like is separated at the final `-` only when the text
    /// after it begins with precisely a hash.
    fn parse(part: &'a str) -> Option<Self> {
        let (stem, rest) = part.rsplit_once('-')?;
        if stem.is_empty() {
            return None;
        }
        // `split_at_checked` refuses a short tail, so the slice below is
        // exactly `HASH_LEN` characters and only the characters need checking.
        let (hash, suffix) = rest.split_at_checked(HASH_LEN)?;
        if !hash.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return None;
        }
        // A suffix is an extension or nothing: `libunit-<hash>.rlib` is a unit,
        // while `<hash>zzz` is a longer hash-shaped word rather than one.
        if !(suffix.is_empty() || suffix.starts_with('.')) {
            return None;
        }
        Some(Self { stem, hash, suffix })
    }
}

/// The length of the metadata hash Cargo puts in a unit name.
const HASH_LEN: usize = 16;

#[cfg(test)]
mod tests;
