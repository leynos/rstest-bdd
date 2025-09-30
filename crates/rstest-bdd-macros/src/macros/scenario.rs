//! Implementation of the `#[scenario]` macro.
//! Binds tests to Gherkin scenarios and validates steps when compile-time flags enable it.

use cap_std::{ambient_authority, fs::Dir};
use cfg_if::cfg_if;
use proc_macro::TokenStream;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{LazyLock, RwLock};

use crate::codegen::scenario::{ScenarioConfig, generate_scenario_code};
use crate::parsing::feature::{ScenarioData, extract_scenario_steps, parse_and_load_feature};
use crate::utils::fixtures::extract_function_fixtures;
use crate::validation::parameters::process_scenario_outline_examples;

/// Cache of canonicalised feature paths to avoid repeated filesystem lookups.
static FEATURE_PATH_CACHE: LazyLock<RwLock<HashMap<PathBuf, String>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));

use syn::{
    LitInt, LitStr,
    parse::{Parse, ParseStream},
    punctuated::Punctuated,
    token::Comma,
};

struct ScenarioArgs {
    path: LitStr,
    index: Option<usize>,
}

enum ScenarioArg {
    Path(LitStr),
    Index(usize),
}

impl Parse for ScenarioArg {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        if input.peek(LitStr) {
            let lit: LitStr = input.parse()?;
            Ok(Self::Path(lit))
        } else {
            let ident: syn::Ident = input.parse()?;
            input.parse::<syn::token::Eq>()?;
            if ident == "path" {
                Ok(Self::Path(input.parse()?))
            } else if ident == "index" {
                let li: LitInt = input.parse()?;
                Ok(Self::Index(li.base10_parse()?))
            } else {
                Err(input.error("expected `path` or `index`"))
            }
        }
    }
}

impl Parse for ScenarioArgs {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let args = Punctuated::<ScenarioArg, Comma>::parse_terminated(input)?;
        let mut path = None;
        let mut index = None;

        for arg in args {
            match arg {
                ScenarioArg::Path(lit) => {
                    if path.is_some() {
                        return Err(input.error("duplicate `path` argument"));
                    }
                    path = Some(lit);
                }
                ScenarioArg::Index(i) => {
                    if index.is_some() {
                        return Err(input.error("duplicate `index` argument"));
                    }
                    index = Some(i);
                }
            }
        }

        let path = path.ok_or_else(|| input.error("`path` is required"))?;

        Ok(Self { path, index })
    }
}

pub(crate) fn scenario(attr: TokenStream, item: TokenStream) -> TokenStream {
    let args = syn::parse_macro_input!(attr as ScenarioArgs);
    let item_fn = syn::parse_macro_input!(item as syn::ItemFn);
    match try_scenario(args, item_fn) {
        Ok(tokens) => tokens,
        Err(err) => err,
    }
}

fn try_scenario(
    ScenarioArgs { path, index }: ScenarioArgs,
    mut item_fn: syn::ItemFn,
) -> std::result::Result<TokenStream, TokenStream> {
    let path = PathBuf::from(path.value());
    let attrs = &item_fn.attrs;
    let vis = &item_fn.vis;
    let sig = &mut item_fn.sig;
    let block = &item_fn.block;

    // Retrieve cached feature to avoid repeated parsing.
    let feature = parse_and_load_feature(&path).map_err(proc_macro::TokenStream::from)?;
    let feature_path_str = canonical_feature_path(&path);
    let ScenarioData {
        name: scenario_name,
        steps,
        examples,
    } = extract_scenario_steps(&feature, index).map_err(proc_macro::TokenStream::from)?;

    if let Some(err) = validate_steps_compile_time(&steps) {
        return Err(err);
    }

    process_scenario_outline_examples(sig, examples.as_ref())
        .map_err(proc_macro::TokenStream::from)?;

    let (_args, ctx_inserts) = extract_function_fixtures(sig)
        .map_err(|err| proc_macro::TokenStream::from(err.into_compile_error()))?;

    Ok(generate_scenario_code(
        ScenarioConfig {
            attrs,
            vis,
            sig,
            block,
            feature_path: feature_path_str,
            scenario_name,
            steps,
            examples,
        },
        ctx_inserts.into_iter(),
    ))
}

/// Normalise path components so equivalent inputs share cache entries.
///
/// Policy:
/// - Do not alter absolute or prefixed paths; leave absolute resolution to filesystem canonicalisation.
/// - Collapse internal `.` segments.
/// - Collapse `..` only when a prior non-`..` segment exists; otherwise preserve leading `..`.
fn normalise(path: &Path) -> PathBuf {
    use std::ffi::OsString;
    use std::path::Component;

    if path.is_absolute() {
        return path.to_path_buf();
    }

    let mut segs: Vec<OsString> = Vec::new();
    for c in path.components() {
        match c {
            Component::CurDir => {}
            Component::ParentDir => {
                if segs.last().is_some_and(|s| s != "..") {
                    segs.pop();
                } else {
                    segs.push(OsString::from(".."));
                }
            }
            Component::Normal(s) => segs.push(s.to_os_string()),
            _ => segs.push(c.as_os_str().to_os_string()),
        }
    }
    let mut out = PathBuf::new();
    for s in segs {
        out.push(s);
    }
    out
}

#[cfg(all(test, windows))]
mod windows_paths {
    use super::normalise;
    use std::path::Path;

    #[test]
    fn preserves_drive_relative_parent_segments() {
        let p = Path::new(r"C:foo\..\bar");
        assert_eq!(normalise(p).to_string_lossy(), r"C:bar");
    }

    #[test]
    fn does_not_mangle_unc_prefix() {
        let p = Path::new(r"\\server\share\.\dir\..\file");
        assert_eq!(normalise(p), p);
    }
}

fn canonicalise_with_cap_std(path: &Path) -> Option<PathBuf> {
    let authority = ambient_authority();
    if path.is_absolute() {
        let Some(parent) = path.parent() else {
            return Some(path.to_path_buf());
        };
        let Some(name) = path.file_name() else {
            return Some(path.to_path_buf());
        };
        let name = PathBuf::from(name);
        let dir = Dir::open_ambient_dir(parent, authority).ok()?;
        let resolved = dir.canonicalize(&name).ok()?;
        Some(parent.to_path_buf().join(resolved))
    } else {
        let cwd = std::env::current_dir().ok()?;
        let dir = Dir::open_ambient_dir(&cwd, authority).ok()?;
        let resolved = dir.canonicalize(path).ok()?;
        Some(cwd.join(resolved))
    }
}

/// Canonicalise the feature path for stable diagnostics.
///
/// Resolves symlinks via cap-std directory canonicalisation so diagnostics
/// and generated code reference a consistent absolute path across builds.
/// The returned `String` is produced with [`Path::display`], so non-UTF-8
/// components are lossy.
///
/// # Examples
///
/// ```rust,ignore
/// use std::path::{Path, PathBuf};
///
/// let path = PathBuf::from("features/example.feature");
/// let _ = canonical_feature_path(&path);
/// ```
fn canonical_feature_path(path: &Path) -> String {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").ok().map(PathBuf::from);
    // Scope cache keys by manifest dir to avoid cross-crate collisions.
    let key = if path.is_absolute() {
        normalise(path)
    } else if let Some(ref dir) = manifest_dir {
        dir.join(normalise(path))
    } else {
        normalise(path)
    };

    if let Some(cached) = {
        let cache = FEATURE_PATH_CACHE
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        cache.get(&key).cloned()
    } {
        return cached;
    }

    let canonical = manifest_dir
        .as_ref()
        .map(|d| d.join(path))
        .and_then(|p| canonicalise_with_cap_std(&p))
        .unwrap_or_else(|| PathBuf::from(path))
        .display()
        .to_string();

    let mut cache = FEATURE_PATH_CACHE
        .write()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let entry = cache.entry(key).or_insert_with(|| canonical.clone());
    entry.clone()
}

/// Validate registered steps when compile-time validation is enabled.
///
/// ```rust,ignore
/// let steps = Vec::new();
/// let _ = validate_steps_compile_time(&steps);
/// ```
fn validate_steps_compile_time(
    steps: &[crate::parsing::feature::ParsedStep],
) -> Option<TokenStream> {
    let res: Result<(), syn::Error> = {
        cfg_if! {
            if #[cfg(feature = "strict-compile-time-validation")] {
                crate::validation::steps::validate_steps_exist(steps, true)
            } else if #[cfg(feature = "compile-time-validation")] {
                crate::validation::steps::validate_steps_exist(steps, false)
            } else {
                let _ = steps;
                Ok(())
            }
        }
    };
    res.err()
        .map(|e| proc_macro::TokenStream::from(e.into_compile_error()))
}

#[cfg(test)]
fn clear_feature_path_cache() {
    FEATURE_PATH_CACHE
        .write()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clear();
}

#[cfg(test)]
mod tests {
    use super::{canonical_feature_path, canonicalise_with_cap_std, clear_feature_path_cache};
    use rstest::{fixture, rstest};
    use serial_test::serial;

    use std::env;
    use std::path::{Path, PathBuf};

    #[fixture]
    fn cache_cleared() {
        clear_feature_path_cache();
    }

    fn with_dir<F, T>(path: &Path, f: F) -> std::io::Result<T>
    where
        F: FnOnce(&super::Dir, &Path) -> std::io::Result<T>,
    {
        let authority = super::ambient_authority();
        if path.is_absolute() {
            let parent = path.parent().ok_or_else(|| {
                std::io::Error::new(std::io::ErrorKind::InvalidInput, "path missing parent")
            })?;
            let file_name = path.file_name().ok_or_else(|| {
                std::io::Error::new(std::io::ErrorKind::InvalidInput, "path missing file name")
            })?;
            let relative = PathBuf::from(file_name);
            let dir = super::Dir::open_ambient_dir(parent, authority)?;
            f(&dir, relative.as_path())
        } else {
            let cwd = std::env::current_dir()?;
            let dir = super::Dir::open_ambient_dir(&cwd, authority)?;
            f(&dir, path)
        }
    }

    fn create_dir_all_cap(path: &Path) -> std::io::Result<()> {
        if path.as_os_str().is_empty() || path == Path::new(".") {
            return Ok(());
        }

        if path.file_name().is_none() {
            if let Some(parent) = path.parent() {
                if parent != path {
                    create_dir_all_cap(parent)?;
                }
            }
            return Ok(());
        }

        if let Some(parent) = path.parent() {
            if parent != path {
                create_dir_all_cap(parent)?;
            }
        }

        with_dir(path, |dir, target| match dir.create_dir(target) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => Ok(()),
            Err(error) => Err(error),
        })
    }

    fn write_file_cap(path: &Path, contents: &[u8]) -> std::io::Result<()> {
        with_dir(path, |dir, target| dir.write(target, contents))
    }

    fn remove_file_cap(path: &Path) -> std::io::Result<()> {
        with_dir(path, |dir, target| dir.remove_file(target))
    }

    #[serial]
    #[rstest]
    #[expect(
        clippy::expect_used,
        reason = "tests require explicit failure messages"
    )]
    fn canonicalises_with_manifest_dir(_cache_cleared: ()) {
        let manifest = PathBuf::from(
            env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR is required for tests"),
        );
        let path = Path::new("Cargo.toml");
        let expected = canonicalise_with_cap_std(&manifest.join(path))
            .expect("canonical path")
            .display()
            .to_string();
        assert_eq!(canonical_feature_path(path), expected);
    }

    #[serial]
    #[rstest]
    fn falls_back_on_missing_path(_cache_cleared: ()) {
        let path = Path::new("does-not-exist.feature");
        assert_eq!(canonical_feature_path(path), path.display().to_string());
    }

    #[serial]
    #[rstest]
    fn equivalent_relatives_map_to_same_result(_cache_cleared: ()) {
        let a = Path::new("./features/../features/example.feature");
        let b = Path::new("features/example.feature");
        assert_eq!(canonical_feature_path(a), canonical_feature_path(b));
    }

    #[serial]
    #[rstest]
    #[expect(
        clippy::expect_used,
        reason = "tests require explicit failure messages"
    )]
    fn caches_paths_between_calls(_cache_cleared: ()) {
        use std::time::{SystemTime, UNIX_EPOCH};

        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let file_name = format!("cache_{unique}.feature");
        let manifest = PathBuf::from(
            env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR is required for tests"),
        );
        let tmp_dir = manifest.join("target/canonical-path-cache-tests");
        create_dir_all_cap(&tmp_dir).expect("create tmp dir");
        let file_path = tmp_dir.join(&file_name);
        write_file_cap(&file_path, b"").expect("create temp feature file");

        let rel_path = format!("target/canonical-path-cache-tests/{file_name}");
        let path = Path::new(&rel_path);
        let first = canonical_feature_path(path);

        remove_file_cap(&file_path).expect("remove temp feature file");
        let second = canonical_feature_path(path);

        assert_eq!(first, second);
    }
}
