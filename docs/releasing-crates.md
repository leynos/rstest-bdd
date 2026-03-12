# Releasing rstest-bdd crates

This guide summarizes the steps for publishing the `rstest-bdd` workspace
crates to [crates.io](https://crates.io/). Each release assumes the workspace
version in `Cargo.toml` has already been bumped and the changelog entries are
prepared. The workspace currently contains seven publishable libraries and the
`cargo-bdd` support tool, so follow the sequence below to keep the dependency
graph satisfied.

`rstest-bdd-harness-gpui` remains developed against the workspace-local GPUI
shim, so the main repository stays free of `async-trait`. The publish-check
automation now synthesizes a standalone package artifact for the crate and
compiles a generated validator crate against the upstream `gpui` dependency, so
the crates.io dependency surface is verified before publication.

1. **Run the full quality gate.** Execute `make fmt`, `make lint`,
   `make markdownlint`, and `make test` from the workspace root. Resolve any
   failures before proceeding.
2. **Publish `rstest-bdd-patterns`.**
   - `cd crates/rstest-bdd-patterns`
   - `cargo publish --dry-run`
   - `cargo publish`
3. **Publish `rstest-bdd-policy`.**
   - `cd crates/rstest-bdd-policy`
   - `cargo publish --dry-run`
   - `cargo publish`
4. **Publish `rstest-bdd-harness`.**
   - `cd crates/rstest-bdd-harness`
   - `cargo publish --dry-run`
   - `cargo publish`
5. **Publish `rstest-bdd-harness-gpui`.**
   - `cd crates/rstest-bdd-harness-gpui`
   - `cargo publish --dry-run`
   - `cargo publish`
6. **Publish `rstest-bdd-harness-tokio`.**
   - `cd crates/rstest-bdd-harness-tokio`
   - `cargo publish --dry-run`
   - `cargo publish`
7. **Publish `rstest-bdd-macros`.**
   - `cd crates/rstest-bdd-macros`
   - `cargo publish --dry-run`
   - `cargo publish`
8. **Publish `rstest-bdd`.**
   - `cd crates/rstest-bdd`
   - `cargo publish --dry-run`
   - `cargo publish`
9. **Publish `cargo-bdd`.** This binary depends on the `rstest-bdd`
   diagnostics feature, so wait until crates.io finishes indexing the library
   release before packaging it.
   - `cd crates/cargo-bdd`
   - Optionally run `cargo install --path . --locked` from the same directory to
     validate that the crate installs correctly before publishing.
   - `cargo publish --dry-run --locked`
   - `cargo publish --locked`
10. **Tag the release.** Create a git tag matching the published version and
   push it to the repository.

    - `git tag -a vX.Y.Z -m "rstest-bdd vX.Y.Z"`
    - `git push origin vX.Y.Z`

The manual steps above are now automated by
`uv run scripts/run_publish_check.py --live`, which exports a clean copy of the
workspace, rewrites manifests to target crates.io, and then executes the
`cargo publish` commands in the required order. The dry-run workflow also
builds a standalone archive for `rstest-bdd-harness-gpui` and compiles a
generated validator crate against upstream `gpui`, while the live workflow
preserves the `--dry-run` guard rails before each publish invocation and
applies `--locked` for `cargo-bdd`.

Cargo enforces that published dependencies already exist on crates.io. This is
why the crates must be released in the order shown above.
