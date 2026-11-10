//! Resolves library selections against the lexical Rust module path.

use std::collections::HashSet;

/// Context required to resolve a library path lexically.
struct LibraryPathContext<'a> {
    /// Parsed library path selected by a scenario binding.
    path: &'a syn::Path,
    /// Stringified path segments for qualifier inspection.
    segments: &'a [String],
    /// Enclosing inline-module path of the binding.
    module_path: &'a [String],
    /// Known inline-module paths in the indexed source file.
    module_paths: &'a HashSet<Vec<String>>,
}

/// Lexical origin from which to resolve a selected library path.
enum PathAnchor {
    /// A path rooted outside the enclosing Rust module.
    Root,
    /// A path relative to the enclosing Rust module.
    CurrentModule,
    /// A path relative to an ancestor of the enclosing Rust module.
    ParentModules(usize),
    /// A bare path whose first segment names a known inline module.
    LocalModule,
    /// A bare path belonging to another crate.
    External,
}

/// Resolve a selected library as an ordinary path from its enclosing module.
pub(super) fn resolve_library_path(
    path: &syn::Path,
    module_path: &[String],
    module_paths: &HashSet<Vec<String>>,
) -> String {
    let segments = path
        .segments
        .iter()
        .map(|segment| segment.ident.to_string())
        .collect::<Vec<_>>();
    let context = LibraryPathContext {
        path,
        segments: &segments,
        module_path,
        module_paths,
    };
    let mut resolved = path_prefix(&context);
    resolved.extend(path_suffix(&segments));
    resolved.join("::")
}

/// Select the lexical base for a Rust library path.
fn path_prefix(context: &LibraryPathContext<'_>) -> Vec<String> {
    match classify_path_anchor(context) {
        PathAnchor::Root | PathAnchor::External => Vec::new(),
        PathAnchor::CurrentModule | PathAnchor::LocalModule => context.module_path.to_vec(),
        PathAnchor::ParentModules(levels) => parent_module_prefix(context.module_path, levels),
    }
}

/// Classify a selected path before deriving its lexical prefix.
fn classify_path_anchor(context: &LibraryPathContext<'_>) -> PathAnchor {
    if context.path.leading_colon.is_some() {
        return PathAnchor::Root;
    }
    match context.segments.first().map(String::as_str) {
        Some("crate") => PathAnchor::Root,
        Some("self") => PathAnchor::CurrentModule,
        Some("super") => PathAnchor::ParentModules(leading_super_count(context.segments)),
        _ if is_builtin_global_path(context.segments) => PathAnchor::Root,
        _ if has_known_local_module(context) => PathAnchor::LocalModule,
        _ => PathAnchor::External,
    }
}

/// Count leading `super` qualifiers in a selected path.
fn leading_super_count(segments: &[String]) -> usize {
    segments
        .iter()
        .take_while(|segment| *segment == "super")
        .count()
}

/// Return the enclosing module after moving up `levels` parents.
fn parent_module_prefix(module_path: &[String], levels: usize) -> Vec<String> {
    let mut prefix = module_path.to_vec();
    for _ in 0..levels {
        prefix.pop();
    }
    prefix
}

/// Determine whether a bare path starts at a known inline module.
fn has_known_local_module(context: &LibraryPathContext<'_>) -> bool {
    let mut local_candidate = context.module_path.to_vec();
    if let Some(segment) = context.segments.first() {
        local_candidate.push(segment.clone());
    }
    context.module_paths.contains(&local_candidate)
}

/// Return path segments after leading Rust-relative qualifiers.
fn path_suffix(segments: &[String]) -> impl Iterator<Item = String> + '_ {
    segments
        .iter()
        .skip_while(|segment| matches!(segment.as_str(), "crate" | "self" | "super"))
        .cloned()
}

/// Recognize the built-in global library when named through the runtime crate.
fn is_builtin_global_path(segments: &[String]) -> bool {
    matches!(segments, [runtime, global] if runtime == "rstest_bdd" && global == "global")
}
