//! Compile and manifest checks for the public prelude surface.

use camino::Utf8Path;
use cap_std::{ambient_authority, fs::Dir};
use rstest::rstest;
#[expect(
    unused_imports,
    reason = "every prelude binding must also resolve at the crate root"
)]
use rstest_bdd::prelude::{
    ScenarioState as _,
    Slot as _,
    StepArgs as _,
    StepContext as _,
    StepError as _,
    StepResult as _,
    given as _,
    scenario as _,
    scenarios as _,
    then as _,
    when as _,
};
#[expect(
    unused_imports,
    reason = "name-resolution imports verify the crate-root re-exports"
)]
use rstest_bdd::{
    DataTable as _,
    DataTableRow as _,
    ScenarioState as _,
    StepArgs as _,
    given as _,
    scenario as _,
    scenarios as _,
    step_library as _,
    then as _,
    when as _,
};
#[expect(
    unused_imports,
    reason = "every prelude binding must also resolve at the crate root"
)]
use rstest_bdd::{
    ScenarioState as _,
    Slot as _,
    StepArgs as _,
    StepContext as _,
    StepError as _,
    StepResult as _,
    given as _,
    scenario as _,
    scenarios as _,
    then as _,
    when as _,
};
use toml::Value;

/// The tables a Cargo manifest can declare a dependency in.
const DEPENDENCY_TABLES: [&str; 3] = ["dependencies", "dev-dependencies", "build-dependencies"];

/// Every dependency table reachable from *table*, including nested ones.
///
/// A crate declared under `[target.'cfg(unix)'.dependencies]` reaches the same
/// place as a top-level entry, so a check that reads only the top-level tables
/// would let it through.
fn dependency_tables(table: &toml::map::Map<String, Value>) -> Vec<&toml::map::Map<String, Value>> {
    let mut found = Vec::new();
    for (key, value) in table {
        let Some(nested) = value.as_table() else {
            continue;
        };
        if DEPENDENCY_TABLES.contains(&key.as_str()) {
            found.push(nested);
        } else {
            found.extend(dependency_tables(nested));
        }
    }
    found
}

/// The `[workspace.dependencies]` table of a parsed workspace manifest.
fn workspace_dependencies(manifest: &Value) -> Option<&toml::map::Map<String, Value>> {
    manifest
        .get("workspace")
        .and_then(Value::as_table)
        .and_then(|table| table.get("dependencies"))
        .and_then(Value::as_table)
}

/// The crate one dependency entry declares.
///
/// The key is the name the dependent's source writes, which is not the crate it
/// resolves to once the entry is renamed with `package`, or once it is written
/// as `runtime.workspace = true` and the workspace manifest says which crate
/// `runtime` names. Only the workspace table can answer the second form, so an
/// entry it does not mention falls back to its key.
fn declared_crate<'a>(
    key: &'a str,
    spec: &'a Value,
    workspace: Option<&'a toml::map::Map<String, Value>>,
) -> &'a str {
    let Some(entry) = spec.as_table() else {
        return key;
    };
    if let Some(package) = entry.get("package").and_then(Value::as_str) {
        return package;
    }
    if entry.get("workspace").and_then(Value::as_bool) != Some(true) {
        return key;
    }
    workspace
        .and_then(|table| table.get(key))
        .and_then(|spec| spec.get("package"))
        .and_then(Value::as_str)
        .unwrap_or(key)
}

/// Every crate *manifest* declares, under the name each entry resolves to.
///
/// A manifest that is not a table declares nothing, which is the right answer
/// for this guard: it is asked what a manifest depends on, not whether the
/// manifest is well formed.
fn declared_crates<'a>(
    manifest: &'a Value,
    workspace: Option<&'a toml::map::Map<String, Value>>,
) -> Vec<&'a str> {
    let Some(table) = manifest.as_table() else {
        return Vec::new();
    };
    dependency_tables(table)
        .into_iter()
        .flat_map(|table| table.iter())
        .map(|(key, spec)| declared_crate(key.as_str(), spec, workspace))
        .collect()
}

#[test]
fn macros_crate_does_not_depend_on_runtime() {
    let workspace_root = Utf8Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("runtime crate should be inside the workspace");
    let workspace = Dir::open_ambient_dir(workspace_root, ambient_authority())
        .expect("workspace directory should be readable");
    let manifest_source = workspace
        .read_to_string("crates/rstest-bdd-macros/Cargo.toml")
        .expect("macro crate manifest should be readable");
    let manifest: Value =
        toml::from_str(&manifest_source).expect("macro crate manifest should parse");
    let workspace_source = workspace
        .read_to_string("Cargo.toml")
        .expect("workspace manifest should be readable");
    let workspace_manifest: Value =
        toml::from_str(&workspace_source).expect("workspace manifest should parse");
    let package = manifest
        .get("package")
        .and_then(Value::as_table)
        .expect("macro crate manifest should have a package table");
    assert_eq!(
        package.get("name").and_then(Value::as_str),
        Some("rstest-bdd-macros"),
        "the path should resolve to the macro crate manifest",
    );
    let declared = declared_crates(&manifest, workspace_dependencies(&workspace_manifest));
    assert!(
        declared.contains(&"rstest-bdd-patterns"),
        "positive control: the macro crate should use the shared patterns crate; declared: \
         {declared:?}",
    );
    assert!(
        !declared.contains(&"rstest-bdd"),
        "the macro crate must not depend on the runtime by any spelling; declared: {declared:?}",
    );
}

#[rstest]
#[case::renamed(
    r#"
[dev-dependencies]
runtime = { package = "rstest-bdd", path = "../rstest-bdd" }
"#,
    "a `package` rename declares the runtime under the alias it hides behind"
)]
#[case::target_specific(
    r#"
[target.'cfg(unix)'.dependencies]
rstest-bdd = { path = "../rstest-bdd" }
"#,
    "a target-specific table declares the runtime for the targets it selects"
)]
fn guard_sees_a_runtime_dependency_in_any_spelling(#[case] manifest: &str, #[case] reason: &str) {
    let manifest: Value = toml::from_str(manifest).expect("the inline manifest should parse");

    assert!(
        declared_crates(&manifest, None).contains(&"rstest-bdd"),
        "{reason}",
    );
}

#[test]
fn guard_sees_an_inherited_runtime_dependency() {
    let manifest: Value = toml::from_str(
        r"
[dev-dependencies]
runtime.workspace = true
",
    )
    .expect("the inline manifest should parse");
    let workspace: Value = toml::from_str(
        r#"
[workspace.dependencies]
runtime = { package = "rstest-bdd", path = "../rstest-bdd" }
"#,
    )
    .expect("the inline workspace manifest should parse");

    let declared = declared_crates(&manifest, workspace_dependencies(&workspace));

    assert!(
        declared.contains(&"rstest-bdd"),
        "an inherited entry declares the crate the workspace names, not its local key",
    );
}

#[test]
fn guard_keeps_the_local_key_when_no_workspace_entry_names_it() {
    let manifest: Value = toml::from_str(
        r"
[dev-dependencies]
runtime.workspace = true
",
    )
    .expect("the inline manifest should parse");

    let declared = declared_crates(&manifest, None);

    assert!(
        !declared.contains(&"rstest-bdd"),
        "an unresolved inheritance fallback must not invent the runtime; declared: {declared:?}",
    );
}
