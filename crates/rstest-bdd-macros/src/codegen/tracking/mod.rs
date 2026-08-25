//! Cargo rebuild-dependency tracking for bound `.feature` files (ADR-010,
//! roadmap item 10.3.3).
//!
//! Cargo decides whether to recompile a crate from its dep-info file, and
//! `rustc` only records a file there when it was pulled in through
//! `include_str!`, `include_bytes!`, `include!`, or a build script's
//! `rerun-if-changed` directive. A procedural macro that opens a `.feature`
//! file with ordinary filesystem calls is invisible to that machinery, so a
//! `.feature`-only edit silently skips recompilation — in a testing framework
//! the worst possible failure mode.
//!
//! The mechanism this module implements is the *tracking binding* (Decision
//! D0 in the 10.3.3 ExecPlan): each macro emits, once per bound feature file,
//! an anonymous `const` whose value is `include_bytes!(concat!(
//! env!("CARGO_MANIFEST_DIR"), "/", <relative literal>))`. The deferred path
//! construction keeps the emitted token stream free of absolute paths, and
//! rustc registers the file in dep-info without the file contents or the
//! path ever landing in a binary (the anonymous `const` is unused and is
//! elided; measured in `ExecPlan` transcripts A and B).
//!
//! The regression tests that guard this behaviour are the two scenario-bound
//! tests in `crates/rstest-bdd/tests/feature_rebuild_invalidation.rs` (the
//! dep-info contract and the edit-and-rebuild experiment) and the token-shape
//! tests in the sibling `tests` module.
//!
//! The relative literal follows the normalization rules in Table 1 of the
//! `ExecPlan`. It is derived from the exact string handed to
//! `parse_and_load_feature` — never from `paths.rs::normalize`, which is a
//! cache-key helper whose `..` collapsing would name a different file through
//! a symlink. Reusing it would be a silent correctness bug.

use std::path::{Component, Path, PathBuf};

use proc_macro2::{Span, TokenStream};
use quote::quote;

/// Why a feature file cannot be registered as a Cargo rebuild dependency.
#[derive(Debug)]
pub(crate) enum Untrackable {
    /// The path shares no filesystem root with `CARGO_MANIFEST_DIR`
    /// (different Windows drive, UNC prefix).
    UnrelatableRoot(PathBuf),
    /// The path is not valid UTF-8 and cannot be written as a string literal.
    NonUtf8(PathBuf),
    /// The path is empty.
    Empty,
}

/// A feature-file path expressed relative to `CARGO_MANIFEST_DIR`.
///
/// Always uses `/` separators, never begins with a separator or a drive
/// prefix, and may contain `..` segments (see Table 1 in the `ExecPlan`).
pub(crate) struct TrackedFeaturePath(String);

impl TrackedFeaturePath {
    /// Expresses `path` relative to `CARGO_MANIFEST_DIR`, computing a
    /// component-wise `..` offset when `path` is absolute.
    pub(crate) fn try_new(path: &Path) -> Result<Self, Untrackable> {
        let manifest = std::env::var("CARGO_MANIFEST_DIR")
            .map(PathBuf::from)
            .map_err(|_| Untrackable::UnrelatableRoot(path.to_path_buf()))?;
        Self::try_new_from(path, &manifest)
    }

    /// Expresses `path` relative to `manifest`, which is
    /// `CARGO_MANIFEST_DIR` in production. Split out from [`Self::try_new`]
    /// so the offset computation is unit-testable against synthetic roots.
    pub(crate) fn try_new_from(path: &Path, manifest: &Path) -> Result<Self, Untrackable> {
        if path.as_os_str().is_empty() {
            return Err(Untrackable::Empty);
        }
        if path.is_absolute() {
            Self::relative_to_manifest(path, manifest)
        } else {
            Ok(Self(normalize_relative(path)?))
        }
    }

    /// Compute a component-wise `..` offset from `manifest` to an absolute
    /// `path`, erroring when the two share no filesystem root.
    fn relative_to_manifest(path: &Path, manifest: &Path) -> Result<Self, Untrackable> {
        if !paths_share_root(path, manifest) {
            return Err(Untrackable::UnrelatableRoot(path.to_path_buf()));
        }

        let target: Vec<Component<'_>> = path.components().collect();
        let base: Vec<Component<'_>> = manifest.components().collect();
        let mut common = 0;
        while let (Some(a), Some(b)) = (target.get(common), base.get(common)) {
            if !components_eq(a.as_os_str(), b.as_os_str()) {
                break;
            }
            common += 1;
        }

        let mut parts: Vec<String> = Vec::new();
        for _ in common..base.len() {
            parts.push("..".to_owned());
        }
        let unmatched: Result<Vec<String>, Untrackable> = target
            .iter()
            .skip(common)
            .map(|component| {
                component
                    .as_os_str()
                    .to_str()
                    .map(str::to_owned)
                    .ok_or_else(|| Untrackable::NonUtf8(path.to_path_buf()))
            })
            .collect();
        parts.extend(unmatched?);
        Ok(Self(parts.join("/")))
    }

    /// The manifest-relative literal, using `/` separators throughout.
    pub(crate) fn relative_literal(&self) -> &str { &self.0 }

    /// Emits the Cargo rebuild-dependency item for this feature file.
    pub(crate) fn binding(&self) -> TokenStream {
        let rel = syn::LitStr::new(self.relative_literal(), Span::call_site());
        quote! {
            #[doc = #TRACKING_BINDING_DOC]
            const _: &[u8] = include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/", #rel));
        }
    }
}

/// Normalize a relative path into its slash-joined literal form by walking
/// its parsed components: `.` segments are dropped, `..` segments are
/// **retained** (they are legal in `include_bytes!`, and collapsing them
/// could name a different file through a symlink). Walking components rather
/// than splitting the raw text is what makes the backslash behaviour correct:
/// on Windows the parser already turned `\` into separators, while on POSIX a
/// backslash remains an ordinary filename character and is preserved.
///
/// A relative path carries no root or prefix, so encountering one is a
/// programming-error class of input; it is reported as unrelatable rather
/// than being emitted as a misleading absolute-looking literal.
fn normalize_relative(path: &Path) -> Result<String, Untrackable> {
    let mut parts: Vec<String> = Vec::new();
    for component in path.components() {
        match component {
            Component::CurDir | Component::Prefix(_) | Component::RootDir => {}
            Component::ParentDir => parts.push("..".to_owned()),
            Component::Normal(text) => {
                let Some(text) = text.to_str() else {
                    return Err(Untrackable::NonUtf8(path.to_path_buf()));
                };
                parts.push(text.to_owned());
            }
        }
    }
    Ok(parts.join("/"))
}

/// Compare two path components' text, folding case on Windows where the
/// filesystem is case-insensitive.
fn components_eq(a: &std::ffi::OsStr, b: &std::ffi::OsStr) -> bool {
    #[cfg(windows)]
    {
        a.to_string_lossy().to_lowercase() == b.to_string_lossy().to_lowercase()
    }
    #[cfg(not(windows))]
    {
        a == b
    }
}

/// Whether two absolute paths can be related without crossing filesystem roots.
///
/// Windows drive and UNC prefixes must agree, while POSIX has one root and no
/// `Prefix` component to compare.
fn paths_share_root(path: &Path, manifest: &Path) -> bool {
    match (path.components().next(), manifest.components().next()) {
        (Some(Component::Prefix(path_prefix)), Some(Component::Prefix(manifest_prefix))) => {
            path_prefix.kind() == manifest_prefix.kind()
                && components_eq(path_prefix.as_os_str(), manifest_prefix.as_os_str())
        }
        (Some(Component::Prefix(_)), _) | (_, Some(Component::Prefix(_))) => false,
        _ => true,
    }
}

/// The doc text attached to the tracking `const`, shared with the token-shape
/// tests so the exact-equality contract and the emitted item cannot drift.
///
/// Built with `concat!` from adjacent literals rather than with
/// backslash-newline continuations: proc-macro2 prints a literals raw source
/// text, so continuation indentation would leak into `TokenStream::to_string`
/// and make the emitted form depend on the file's formatting.
const TRACKING_BINDING_DOC: &str = concat!(
    "Registers the bound `.feature` file as a Cargo rebuild dependency (ADR-010). ",
    "Deleting this makes `.feature`-only edits silently skip recompilation; see ",
    "rstest-bdd::feature_rebuild_invalidation.",
);

/// Resolves `path` and emits either the tracking item or a `compile_error!`.
///
/// This is the only place that decides what happens on failure, so the
/// decision cannot be forgotten at a call site.
pub(crate) fn feature_tracking_item(path: &Path, span: Span) -> TokenStream {
    match TrackedFeaturePath::try_new(path) {
        Ok(tracked) => tracked.binding(),
        Err(untrackable) => untrackable_error(untrackable, span),
    }
}

/// Render the D4 diagnostic for a feature file that cannot be tracked.
fn untrackable_error(untrackable: Untrackable, span: Span) -> TokenStream {
    let message = match untrackable {
        Untrackable::UnrelatableRoot(path) => {
            let manifest = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_default();
            format!(
                "feature file `{}` shares no filesystem root with the crate manifest directory \
                 (`{manifest}`), so it cannot be registered as a Cargo rebuild dependency. Use a \
                 manifest-relative path, or a path on the same filesystem root.",
                path.display()
            )
        }
        Untrackable::NonUtf8(path) => format!(
            "feature file path `{}` is not valid UTF-8, so it cannot be registered as a Cargo \
             rebuild dependency.",
            path.display()
        ),
        Untrackable::Empty => "feature file path is empty, so it cannot be registered as a Cargo \
                               rebuild dependency."
            .to_owned(),
    };
    syn::Error::new(span, message).into_compile_error()
}

#[cfg(test)]
mod tests;
