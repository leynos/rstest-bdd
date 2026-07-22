# Releasing rstest-bdd crates

This guide summarizes the steps for publishing the `rstest-bdd` workspace
crates to [crates.io](https://crates.io/). Each release assumes the workspace
version in `Cargo.toml` has already been bumped and the changelog entries are
prepared. The workspace currently contains seven publishable libraries and the
`cargo-bdd` support tool, so follow the sequence below to keep the dependency
graph satisfied.

After bumping workspace versions:

- Run `make update-ui-lints-lock` and commit the updated
  `crates/rstest-bdd/tests/ui_lints/Cargo.lock` to capture any new transitive
  dependencies introduced since the last release.

`rstest-bdd-harness-gpui` remains developed against the workspace-local GPUI
shim through a `version` plus `path` workspace dependency, so local builds use
the stable-compatible shim while the publish path still declares the matching
crates.io `gpui` version.

1. **Run the full quality gate.** Execute `make fmt`, `make lint`,
   `make markdownlint`, and `make test` from the workspace root. Resolve any
   failures before proceeding.
2. **Run the publish dry run.** Execute `make publish-check`, which delegates to
   `lading publish --workspace-root . --allow-unpublished-workspace-deps` using
   the release order in `lading.toml`. The unpublished-workspace override is
   for dry runs only; it lets CI validate a release train whose workspace
   crates share a new version before any crate has been uploaded.
3. **Publish the crates.** Execute
   `uv run lading publish --workspace-root . --live` from the workspace root.
   The configured order is:

   - `rstest-bdd-patterns`
   - `rstest-bdd-policy`
   - `rstest-bdd-harness`
   - `rstest-bdd-macros`
   - `rstest-bdd`
   - `rstest-bdd-harness-gpui`
   - `rstest-bdd-harness-tokio`
   - `cargo-bdd`

   `rstest-bdd-server` remains excluded from publication by `lading.toml`.
4. **Tag the release.** Create a git tag matching the published version and
   push it to the repository.

    - `git tag -a vX.Y.Z -m "rstest-bdd vX.Y.Z"`
    - `git push origin vX.Y.Z`

`lading publish` stages a clean workspace copy, strips local patches according
to `publish.strip_patches = "per-crate"`, runs pre-flight Cargo checks, and
then packages and publishes crates in the configured order. Cargo enforces that
published dependencies already exist on crates.io. This is why the crates must
be released in the order shown above.
