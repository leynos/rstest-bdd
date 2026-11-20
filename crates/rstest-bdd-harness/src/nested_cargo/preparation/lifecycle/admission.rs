//! The read-only admission query, separated from the gate that may build.
//!
//! The gate in [`super`] answers "ensure this family is prepared", and under
//! plain `cargo test` the answer includes running the build. That is the right
//! contract for a caller whose position is "prepare if needed", and the wrong
//! one for a caller asking "is this tree reusable right now": a name that reads
//! like a question must not be able to compile a dependency graph.
//!
//! [`admit`] is that question. It takes the runner decision explicitly — it
//! reads no environment variable — and returns [`Admission`], which states
//! *whether* the tree is reusable without acting on the answer. It performs no
//! build, writes no stamp, and takes no lock: its whole body is one stamp read
//! and one directory inspection.
//!
//! The decision it implements is the same one the gate makes, and it is shared
//! rather than reimplemented: [`Admission`] is derived from the same
//! [`StampDecision`] the gate consults, so "the query admits" and "the gate
//! returns at the stamp" cannot disagree. A second spelling of the rule is
//! exactly the drift this module exists to prevent.
//!
//! The typed answer earns its place for a caller that has to *report* the
//! outcome rather than merely act on it: [`Admission::Reusable`] carries no
//! error path, while a refusal names the stamp and the artefacts that went,
//! which is what a gate turns into its message and what a diagnostics-only
//! caller can print without preparing anything.

use std::io;

use super::{
    super::{
        Preparation,
        artefacts,
        stamp::{StampDecision, read_stamp},
    },
    Instruments,
    instruments_of,
};

/// What the admission query found, as a value rather than an error.
///
/// The variants are the three answers the stamp-and-artefact check can give.
/// They are deliberately not collapsed into a `bool`: a caller that reports
/// the outcome has to distinguish a stamp that is simply absent from one that
/// is present but names artefacts that have gone, and turning both into `false`
/// would throw away the diagnosis the gate exists to produce.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Admission {
    /// The stamp matches and every artefact it vouches for is present.
    ///
    /// A timed assertion reaching here may reuse the tree; nothing needs
    /// building.
    Reusable,
    /// No stamp describes this tree, or the one present is stale.
    ///
    /// Under Nextest this is the refusal the gate raises. Under plain
    /// `cargo test` it is the signal to prepare.
    NotPrepared,
    /// A stamp exists but cannot be read, so nothing can be vouched for.
    ///
    /// The read error travels with the variant because the two runners reach
    /// opposite conclusions from it — fail-fast under Nextest, prepare over
    /// under `cargo test` — and the caller needs the original error to explain
    /// either.
    Unreadable(UnreadableStamp),
}

impl Admission {
    /// Whether the tree may be reused as it stands.
    ///
    /// The one convenience a caller that only branches needs; it deliberately
    /// does not distinguish the two refusal variants, so a caller that wants
    /// the diagnosis must match on the value instead.
    #[must_use]
    pub fn is_reusable(&self) -> bool { matches!(self, Self::Reusable) }
}

/// A stamp that exists but could not be read.
///
/// A distinct type rather than a bare [`io::Error`] so the variant can derive
/// `Clone`, `PartialEq` and `Eq` alongside its siblings: `io::Error` is not
/// comparable, and a caller that only wants to know *which* answer it received
/// should not have to give that up. The original error is kept as its display
/// text, which is all the gate's messages ever used it for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnreadableStamp {
    /// The display text of the read failure.
    pub error: String,
}

impl UnreadableStamp {
    /// Capture a read failure for the query's answer.
    fn of(error: &io::Error) -> Self {
        Self {
            error: error.to_string(),
        }
    }
}

/// Ask whether `preparation` is reusable, without preparing it.
///
/// Takes the runner decision explicitly rather than reading
/// [`super::super::super::running_under_nextest`], so the answer is a pure
/// function of the tree and the decision the caller supplied. This is the
/// call-site-visible form the gate's implicit environment read was hiding.
///
/// `under_nextest` does not change *what* is inspected — the stamp and the
/// artefacts are read the same way under both runners — only how an unreadable
/// stamp is reported. Under Nextest it is [`Admission::Unreadable`], because
/// the tree cannot be vouched for and preparing it here would spend a timed
/// test's budget. Under plain `cargo test` it is [`Admission::NotPrepared`],
/// which is the same answer a stale stamp gives: the caller's remedy is to
/// prepare, and a corrupt stamp is a reason to prepare rather than to refuse.
///
/// This function performs **no build**, writes **no stamp**, and takes **no
/// lock**. It reads the stamp file and inspects the artefact directories, and
/// nothing else.
///
/// # Errors
///
/// Returns the probe's error, so a toolchain that cannot be identified stops
/// the query here rather than admitting a tree against an unknown compiler. It
/// also returns the read error when an artefact directory cannot be inspected,
/// so an unreadable tree is reported as itself rather than as a missing one.
/// The stamp read failure is *not* an error: it is the
/// [`Admission::Unreadable`] or [`Admission::NotPrepared`] answer, because it is
/// a fact about the tree the caller asked about rather than a failure of the
/// query.
pub fn admit(preparation: &Preparation, under_nextest: bool) -> io::Result<Admission> {
    let Instruments { probe, .. } = instruments_of(preparation);
    admit_with_probe(preparation, under_nextest, probe)
}

/// [`admit`] with the toolchain probe supplied.
///
/// Exposed so the query's answer can be exercised against a chosen compiler
/// identity without spawning `cargo -vV`, and so a caller that already holds a
/// probe for the same preparation can reuse it instead of paying for a second
/// resolution.
///
/// A query produces no messages — it returns its answer — so there is no
/// reporter here: binding one would be a parameter nothing reads.
///
/// # Errors
///
/// As [`admit`].
pub fn admit_with_probe<P>(
    preparation: &Preparation,
    under_nextest: bool,
    probe: P,
) -> io::Result<Admission>
where
    P: FnOnce() -> io::Result<String>,
{
    let reading = read_stamp(preparation, probe)?;
    let admission = match reading.decision {
        StampDecision::Unreadable(error) if under_nextest => {
            Admission::Unreadable(UnreadableStamp::of(&error))
        }
        // An unreadable stamp under plain `cargo test` is a reason to prepare,
        // which is the same answer a stale stamp gives. Folding it here keeps
        // the two-runner policy in one place rather than at each call site.
        StampDecision::Unreadable(_) | StampDecision::Missing | StampDecision::Stale => {
            Admission::NotPrepared
        }
        StampDecision::Matching(stamp) => {
            if artefacts::all_present(preparation, Some(&stamp))? {
                Admission::Reusable
            } else {
                // The stamp matches but the tree it vouches for has been
                // evicted. That is not `Reusable`, and it is not `Missing`
                // either: the stamp is present and current, and only the
                // artefacts went.
                Admission::NotPrepared
            }
        }
    };
    Ok(admission)
}
