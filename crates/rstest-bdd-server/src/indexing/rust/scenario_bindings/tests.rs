//! Unit tests for Rust scenario-binding indexing.

use super::*;

#[test]
fn indexes_scenario_and_scenarios_library_scopes() {
    let file = syn::parse_file(concat!(
        "#[scenario(path = \"tests/features/account.feature\", libraries = [common, accounts])]\n",
        "fn account() {}\n",
        "scenarios!(dir = \"tests/features/files\", libraries = [crate::filesystem]);\n",
    ))
    .expect("Rust source");

    let bindings = index_scenario_bindings(&file);

    assert_eq!(bindings.len(), 2);
    let account_binding = bindings.first().expect("account scenario binding");
    let filesystem_binding = bindings.get(1).expect("filesystem scenarios binding");
    assert_eq!(account_binding.libraries, ["common", "accounts"]);
    assert_eq!(filesystem_binding.libraries, ["filesystem"]);
    assert!(matches!(
        &account_binding.target,
        ScenarioBindingTarget::Feature(path)
            if path == &PathBuf::from("tests/features/account.feature")
    ));
    assert!(matches!(
        &filesystem_binding.target,
        ScenarioBindingTarget::Directory(path)
            if path == &PathBuf::from("tests/features/files")
    ));
}

#[test]
fn defaults_unscoped_scenarios_to_the_global_library() {
    let file =
        syn::parse_file("#[scenario(\"test.feature\")]\nfn test() {}\n").expect("Rust source");

    let bindings = index_scenario_bindings(&file);

    assert_eq!(
        bindings.first().expect("global scenario binding").libraries,
        ["rstest_bdd::global"]
    );
}

#[test]
fn resolves_library_paths_from_the_enclosing_module() {
    let file = syn::parse_file(concat!(
        "mod outer {\n",
        "  mod nested {\n",
        "    #[scenario(path = \"test.feature\", libraries = [self::accounts, ",
        "super::shared, crate::root, rstest_bdd::global, steps::global])]\n",
        "    fn test() {}\n",
        "  }\n",
        "}\n",
    ))
    .expect("Rust source");

    let bindings = index_scenario_bindings(&file);

    assert_eq!(
        bindings.first().expect("nested scenario binding").libraries,
        [
            "outer::nested::accounts",
            "outer::shared",
            "root",
            "rstest_bdd::global",
            "outer::nested::steps::global",
        ]
    );
}

#[test]
fn classifies_ignored_binding_failures() {
    let missing_path = "libraries = [accounts]"
        .parse()
        .expect("valid binding tokens");
    let malformed_path = "path = 7".parse().expect("valid binding tokens");

    assert!(matches!(
        parse_binding_arguments(&missing_path),
        Err(BindingIndexFailure::MissingPath)
    ));
    assert!(matches!(
        parse_binding_arguments(&malformed_path),
        Err(BindingIndexFailure::Malformed(_))
    ));
}
