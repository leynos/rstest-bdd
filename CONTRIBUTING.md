# Contributing

Thank you for helping improve `rstest-bdd`.

Before submitting a pull request:

- run `make fmt` and ensure `git diff --exit-code` reports no changes
- run `make lint`
- run `make test`
- run `make markdownlint`
- run `make nixie` to validate Mermaid diagrams

The CI pipeline runs the same commands and will fail if any step reports an
error.
