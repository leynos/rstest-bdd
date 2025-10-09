# Japanese household ledger example

This example crate demonstrates how to write a behaviour-driven test suite in
Japanese using `rstest-bdd`. The tests model a simple household ledger that can
record income and expenses.

## Running the tests

Execute the test suite with:

```bash
cargo test -p japanese-ledger
```

The scenarios live in `tests/features/household_ledger.feature` and use the
`# language: ja` directive so that Gherkin keywords such as `フィーチャ`,
`シナリオ`, `前提`, `もし`, `ならば`, and `しかし` are recognised. Step
definitions in `tests/ledger.rs` are registered with the same Japanese phrases,
illustrating how non-English teams can collaborate on executable specifications.
