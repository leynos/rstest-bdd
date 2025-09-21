# Releasing rstest-bdd crates

This guide summarises the steps for publishing the `rstest-bdd` workspace
crates to [crates.io](https://crates.io/). Each release assumes the workspace
version in `Cargo.toml` has already been bumped and the changelog entries are
prepared. The workspace contains three libraries and the `cargo-bdd` support
tool, so follow the sequence below to keep the dependency graph satisfied.

1. **Run the full quality gate.** Execute `make fmt`, `make lint`,
   `make markdownlint`, and `make test` from the workspace root. Resolve any
   failures before proceeding.
2. **Publish `rstest-bdd-patterns`.**
   - `cd crates/rstest-bdd-patterns`
   - `cargo publish`
3. **Publish `rstest-bdd-macros`.**
   - `cd crates/rstest-bdd-macros`
   - `cargo publish`
4. **Publish `rstest-bdd`.**
   - `cd crates/rstest-bdd`
   - `cargo publish`
5. **Publish `cargo-bdd`.** This binary depends on the `rstest-bdd`
   diagnostics feature, so wait until crates.io finishes indexing the library
   release before packaging it.
   - `cd crates/cargo-bdd`
   - Optionally run `cargo install --path . --locked` from the same directory
     to validate that the crate installs correctly before publishing.
   - `cargo publish --dry-run --locked`
   - `cargo publish --locked`
6. **Tag the release.** Create a git tag matching the published version and
   push it to the repository.
   - `git tag -a vX.Y.Z -m "rstest-bdd vX.Y.Z"`
   - `git push origin vX.Y.Z`

Cargo enforces that published dependencies already exist on crates.io. This is
why the crates must be released in the order shown above.
