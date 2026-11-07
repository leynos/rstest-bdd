//! Compile and manifest checks for the public prelude surface.

use camino::Utf8Path;
use cap_std::{ambient_authority, fs::Dir};
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
    then as _,
    when as _,
};
use toml::Value;

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
    let package = manifest
        .get("package")
        .and_then(Value::as_table)
        .expect("macro crate manifest should have a package table");
    assert_eq!(
        package.get("name").and_then(Value::as_str),
        Some("rstest-bdd-macros"),
        "the path should resolve to the macro crate manifest",
    );
    let dependencies = manifest
        .get("dependencies")
        .and_then(Value::as_table)
        .expect("macro crate should have normal dependencies");
    assert!(
        dependencies.contains_key("rstest-bdd-patterns"),
        "positive control: macro crate should use the shared patterns crate",
    );

    for table_name in ["dependencies", "dev-dependencies", "build-dependencies"] {
        let Some(entries) = manifest.get(table_name) else {
            continue;
        };
        let entries = entries
            .as_table()
            .expect("dependency sections should be tables");
        assert!(
            !entries.contains_key("rstest-bdd"),
            "macro crate must not depend on the runtime through {table_name}",
        );
    }
}
