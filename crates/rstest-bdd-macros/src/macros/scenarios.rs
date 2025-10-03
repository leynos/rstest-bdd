//! Implementation of the `scenarios!` macro.

use cap_std::AmbientAuthority;
use cap_std::ambient_authority;
use cap_std::fs::Dir;
use proc_macro::TokenStream;
use proc_macro2::{Span, TokenStream as TokenStream2};
use quote::{format_ident, quote};
use std::collections::HashSet;
use std::path::{Path, PathBuf};

use walkdir::{DirEntry, WalkDir};

use crate::codegen::scenario::{ScenarioConfig, generate_scenario_code};
use crate::parsing::feature::{extract_scenario_steps, parse_and_load_feature};
use crate::utils::errors::{error_to_tokens, normalized_dir_read_error};
use crate::utils::ident::sanitize_ident;
use gherkin::Feature;

fn is_feature_file(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("feature"))
}

#[cfg(unix)]
fn is_symlink_loop_error(err: &std::io::Error) -> bool {
    err.raw_os_error() == Some(libc::ELOOP)
        || (err.kind() == std::io::ErrorKind::Other && err.to_string().contains("File system loop"))
}

#[cfg(not(unix))]
fn is_symlink_loop_error(err: &std::io::Error) -> bool {
    err.kind() == std::io::ErrorKind::Other && {
        let msg = err.to_string();
        msg.contains("File system loop") || msg.contains("too many levels of symbolic links")
    }
}

fn canonicalize_absolute_path(
    path: &Path,
    authority: AmbientAuthority,
) -> std::io::Result<PathBuf> {
    let root = path
        .ancestors()
        .last()
        .unwrap_or_else(|| Path::new(std::path::MAIN_SEPARATOR_STR));
    let dir = Dir::open_ambient_dir(root, authority)?;
    let relative = path.strip_prefix(root).unwrap_or(path);
    let target = if relative.as_os_str().is_empty() {
        Path::new(".")
    } else {
        relative
    };
    let resolved = dir.canonicalize(target)?;
    if resolved.is_absolute() {
        Ok(resolved)
    } else {
        Ok(PathBuf::from(root).join(resolved))
    }
}

fn canonicalize_relative_path(
    path: &Path,
    authority: AmbientAuthority,
) -> std::io::Result<PathBuf> {
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let dir = Dir::open_ambient_dir(parent, authority)?;
    let target = if parent == Path::new(".") {
        path
    } else {
        path.strip_prefix(parent).unwrap_or(path)
    };
    let resolved = dir.canonicalize(target)?;
    if resolved.is_absolute() {
        Ok(resolved)
    } else if parent == Path::new(".") {
        Ok(std::env::current_dir()?.join(resolved))
    } else {
        Ok(parent.to_path_buf().join(resolved))
    }
}

fn canonicalize_path(path: &Path) -> std::io::Result<PathBuf> {
    let authority = ambient_authority();
    let attempt = if path.is_absolute() {
        canonicalize_absolute_path(path, authority)
    } else {
        canonicalize_relative_path(path, authority)
    };

    match attempt {
        Ok(resolved) => Ok(resolved),
        Err(err)
            if err.kind() == std::io::ErrorKind::PermissionDenied
                && err.to_string().contains("outside of the filesystem") =>
        {
            // cap-std denies canonicalising absolute symlinks that escape a
            // capability root. Falling back to std ensures we still support
            // such links while preferring capability-aware resolution for all
            // other cases.
            std::fs::canonicalize(path)
        }
        Err(err) => Err(err),
    }
}

/// Process a directory entry and return its path when it resolves to a
/// `.feature` file.
///
/// Canonicalisation avoids re-implementing symlink resolution logic while still
/// returning the original (potentially symlinked) path to the caller. Directory
/// entries that do not resolve to `.feature` files return `None`. Any I/O
/// failures bubble up so traversal errors remain visible to the macro user.
fn process_dir_entry(entry: DirEntry) -> Option<std::io::Result<PathBuf>> {
    if entry.file_type().is_dir() {
        return None;
    }

    let original_path = entry.into_path();
    match canonicalize_path(&original_path) {
        Ok(real_path) if real_path.is_file() && is_feature_file(&real_path) => {
            Some(Ok(original_path))
        }
        Ok(_) => None,
        Err(err) => Some(Err(err)),
    }
}

fn convert_walkdir_error(err: walkdir::Error) -> Option<std::io::Error> {
    if err.loop_ancestor().is_some() {
        return None;
    }

    let err_str = err.to_string();
    Some(
        err.into_io_error()
            .unwrap_or_else(|| std::io::Error::other(err_str)),
    )
}

fn should_skip_directory(
    entry: &DirEntry,
    visited_dirs: &mut HashSet<PathBuf>,
) -> std::io::Result<bool> {
    match canonicalize_path(entry.path()) {
        Ok(real_path) => Ok(!visited_dirs.insert(real_path)),
        Err(err) if is_symlink_loop_error(&err) => Ok(true),
        Err(err) => Err(err),
    }
}

fn push_if_feature(entry: DirEntry, files: &mut Vec<PathBuf>) -> std::io::Result<()> {
    if let Some(result) = process_dir_entry(entry) {
        files.push(result?);
    }

    Ok(())
}

enum WalkDecision {
    Continue,
    SkipDir,
    File(DirEntry),
}

fn classify_entry(
    next: walkdir::Result<DirEntry>,
    visited_dirs: &mut HashSet<PathBuf>,
) -> Result<WalkDecision, Option<std::io::Error>> {
    let entry = match next {
        Ok(entry) => entry,
        Err(err) => return Err(convert_walkdir_error(err)),
    };

    if entry.file_type().is_dir() {
        let should_skip = should_skip_directory(&entry, visited_dirs).map_err(Some)?;
        if should_skip {
            return Ok(WalkDecision::SkipDir);
        }

        return Ok(WalkDecision::Continue);
    }

    Ok(WalkDecision::File(entry))
}

/// Recursively collect all `.feature` files under `base`.
fn collect_feature_files(base: &Path) -> std::io::Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    let mut visited_dirs: HashSet<PathBuf> = HashSet::new();
    let mut iterator = WalkDir::new(base).follow_links(true).into_iter();

    while let Some(next) = iterator.next() {
        match classify_entry(next, &mut visited_dirs) {
            Ok(WalkDecision::SkipDir) => iterator.skip_current_dir(),
            Ok(WalkDecision::File(entry)) => push_if_feature(entry, &mut files)?,
            Err(Some(err)) => return Err(err),
            Ok(WalkDecision::Continue) | Err(None) => {}
        }
    }

    files.sort();
    Ok(files)
}

/// Generate the test for a single scenario within a feature.
/// Context for generating a scenario test.
struct ScenarioTestContext<'a> {
    feature: &'a Feature,
    scenario_idx: usize,
    feature_stem: &'a str,
    manifest_dir: &'a Path,
    rel_path: &'a Path,
}

fn dedupe_name(base: &str, used: &mut HashSet<String>) -> String {
    let mut name = base.to_string();
    let mut counter = 1usize;
    while used.contains(&name) {
        name = format!("{base}_{counter}");
        counter += 1;
    }
    used.insert(name.clone());
    name
}

fn generate_scenario_test(
    ctx: &ScenarioTestContext<'_>,
    used_names: &mut HashSet<String>,
) -> Result<TokenStream2, TokenStream> {
    let data = extract_scenario_steps(ctx.feature, Some(ctx.scenario_idx))?;
    let base_name = format!("{}_{}", ctx.feature_stem, sanitize_ident(&data.name));
    let fn_name = dedupe_name(&base_name, used_names);
    let fn_ident = format_ident!("{}", fn_name);

    let attrs: Vec<syn::Attribute> = Vec::new();
    let vis = syn::Visibility::Inherited;
    let sig: syn::Signature = data.examples.as_ref().map_or_else(
        || syn::parse_quote! { fn #fn_ident() },
        |ex| {
            let params = ex.headers.iter().map(|h| {
                let param_ident = format_ident!("{}", sanitize_ident(h));
                quote! { #[case] #param_ident: &str }
            });
            syn::parse_quote! { fn #fn_ident( #(#params),* ) }
        },
    );
    let block: syn::Block = syn::parse_quote!({});

    let feature_path = ctx.manifest_dir.join(ctx.rel_path).display().to_string();

    let config = ScenarioConfig {
        attrs: &attrs,
        vis: &vis,
        sig: &sig,
        block: &block,
        feature_path,
        scenario_name: data.name,
        steps: data.steps,
        examples: data.examples,
    };
    Ok(TokenStream2::from(generate_scenario_code(
        config,
        std::iter::empty(),
    )))
}

/// Resolve the Cargo manifest directory or return a compile error.
///
/// # Examples
///
/// ```rust,ignore
/// std::env::set_var("CARGO_MANIFEST_DIR", "/tmp");
/// let path =
///     resolve_manifest_directory().expect("CARGO_MANIFEST_DIR is set");
/// assert_eq!(path, std::path::PathBuf::from("/tmp"));
/// ```
#[expect(
    clippy::single_match_else,
    clippy::option_if_let_else,
    reason = "explicit match clarifies control flow"
)]
fn resolve_manifest_directory() -> Result<PathBuf, TokenStream> {
    match std::env::var("CARGO_MANIFEST_DIR") {
        Ok(v) => Ok(PathBuf::from(v)),
        Err(_) => {
            let err = syn::Error::new(
                Span::call_site(),
                "CARGO_MANIFEST_DIR is not set. This macro must run within Cargo.",
            );
            Err(error_to_tokens(&err).into())
        }
    }
}

/// Generate the test code for every scenario inside a single feature file.
///
/// Deduplicates test names using `used_names` and collects errors without
/// short-circuiting.
fn process_feature_file(
    abs_path: &Path,
    manifest_dir: &Path,
    used_names: &mut HashSet<String>,
) -> (Vec<TokenStream2>, Vec<TokenStream2>) {
    let rel_path = abs_path
        .strip_prefix(manifest_dir)
        .map_or_else(|_| abs_path.to_path_buf(), Path::to_path_buf);

    let mut tests = Vec::new();
    let mut errors = Vec::new();

    // Load feature from cache, parsing once per unique path.
    match parse_and_load_feature(rel_path.as_path()) {
        Ok(feature) => {
            let feature_stem = sanitize_ident(
                rel_path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("feature"),
            );
            for (idx, _) in feature.scenarios.iter().enumerate() {
                let ctx = ScenarioTestContext {
                    feature: &feature,
                    scenario_idx: idx,
                    feature_stem: &feature_stem,
                    manifest_dir,
                    rel_path: &rel_path,
                };
                match generate_scenario_test(&ctx, used_names) {
                    Ok(ts) => tests.push(ts),
                    Err(err) => errors.push(TokenStream2::from(err)),
                }
            }
        }
        Err(err) => errors.push(err),
    }

    (tests, errors)
}

/// Generate tests for the provided feature paths, returning any errors.
fn generate_tests_from_features(
    feature_paths: Vec<PathBuf>,
    manifest_dir: &Path,
) -> (Vec<TokenStream2>, Vec<TokenStream2>) {
    let mut used_names = HashSet::new();
    let mut tests = Vec::new();
    let mut errors = Vec::new();
    for abs_path in feature_paths {
        let (mut t, mut errs) =
            process_feature_file(abs_path.as_path(), manifest_dir, &mut used_names);
        tests.append(&mut t);
        errors.append(&mut errs);
    }
    (tests, errors)
}

/// Generate test modules for all scenarios within a directory of feature files.
pub(crate) fn scenarios(input: TokenStream) -> TokenStream {
    let dir_lit = syn::parse_macro_input!(input as syn::LitStr);
    let dir = PathBuf::from(dir_lit.value());

    let manifest_dir = match resolve_manifest_directory() {
        Ok(dir) => dir,
        Err(err_tokens) => return err_tokens,
    };

    let search_dir = manifest_dir.join(&dir);
    let feature_paths_res = collect_feature_files(&search_dir);
    if let Err(err) = feature_paths_res {
        let msg = normalized_dir_read_error(&search_dir, &err);
        let err = syn::Error::new(Span::call_site(), msg);
        return error_to_tokens(&err).into();
    }
    let Ok(feature_paths) = feature_paths_res else {
        unreachable!("checked Err above");
    };

    let (tests, errors) = generate_tests_from_features(feature_paths, &manifest_dir);

    let module_ident = {
        let base = dir
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("scenarios");
        format_ident!("{}_scenarios", sanitize_ident(base))
    };
    let module_doc = format!("Scenarios auto-generated from `{}`.", dir_lit.value());

    TokenStream::from(quote! {
        #[doc = #module_doc]
        mod #module_ident {
            use super::*;
            #(#tests)*
            #(#errors)*
        }
    })
}

#[cfg(test)]
mod tests {
    use super::dedupe_name;
    use std::collections::HashSet;

    #[test]
    fn deduplicates_duplicate_titles() {
        let mut used = HashSet::new();
        let first = dedupe_name("dup_same_name", &mut used);
        let second = dedupe_name("dup_same_name", &mut used);
        assert_eq!(first, "dup_same_name");
        assert_eq!(second, "dup_same_name_1");
    }

    #[cfg(unix)]
    mod unix {
        use super::super::collect_feature_files;
        use std::fs;
        use std::io;
        use std::os::unix::fs::symlink;
        use std::path::Path;
        use tempfile::tempdir;

        #[test]
        fn collects_symlinked_feature_files_without_following_directory_loops() -> io::Result<()> {
            let temp = tempdir()?;
            let features_root = temp.path().join("features");
            fs::create_dir_all(features_root.join("nested"))?;

            let feature_path = features_root.join("nested/example.feature");
            fs::write(&feature_path, "Feature: Example\n")?;

            let symlink_path = features_root.join("symlink.feature");
            symlink(&feature_path, &symlink_path)?;

            let relative_symlink_path = features_root.join("relative_link.feature");
            symlink(Path::new("nested/example.feature"), &relative_symlink_path)?;

            let loop_dir = features_root.join("loop");
            symlink(&features_root, &loop_dir)?;

            let files = collect_feature_files(features_root.as_path())?;

            let mut expected = vec![feature_path, symlink_path, relative_symlink_path];
            expected.sort();
            assert_eq!(files, expected);

            Ok(())
        }
    }

    #[cfg(not(unix))]
    #[test]
    fn collects_symlinked_feature_files_without_following_directory_loops() {
        assert!(cfg!(not(unix)));
    }
}
