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

## Localisation guidelines

- Keep feature files and step definitions in the same language. If you add a
  new locale, include feature coverage in `tests/features/i18n/` or under an
  example crate so that CI exercises the keyword catalogue.
- Translation resources live under `crates/rstest-bdd/i18n/` and
  `crates/rstest-bdd-macros/i18n/`. Update both English (`en-US`) and the
  translated catalogues when strings change, and document the change in
  `docs/rstest-bdd-design.md`.
- Gherkin keywords must match the canonical catalogue from `gherkin`.
  Cross-check against the upstream list before adding new keywords to a feature
  file.
