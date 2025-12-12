# rstest-bdd-server

Language Server Protocol (LSP) implementation for the rstest-bdd
Behaviour-Driven Development (BDD) testing framework. Provides Integrated
Development Environment (IDE) integration for navigation between Rust step
definitions and Gherkin feature files.

## Installation

```bash
cargo install rstest-bdd-server
```

## Usage

The server binary is named `rstest-bdd-lsp` and communicates via JSON-RPC over
stdin/stdout.

```bash
rstest-bdd-lsp --help
```

### Environment Variables

Configuration precedence (lowest to highest): defaults → environment variables
→ CLI flags.

The CLI currently exposes `--log-level`, which overrides
`RSTEST_BDD_LSP_LOG_LEVEL` when provided. Debounce configuration is controlled
via `RSTEST_BDD_LSP_DEBOUNCE_MS`.

| Variable                     | Default | Description                                     |
| ---------------------------- | ------- | ----------------------------------------------- |
| `RSTEST_BDD_LSP_LOG_LEVEL`   | `info`  | Log verbosity (trace, debug, info, warn, error) |
| `RSTEST_BDD_LSP_DEBOUNCE_MS` | `300`   | Delay before processing file changes            |

## Editor Configuration

See the [User Guide](../../docs/users-guide.md) for editor-specific setup
instructions. On crates.io, the GitHub-hosted copy is available at
`https://github.com/leynos/rstest-bdd/blob/main/docs/users-guide.md`.

## Licence

ISC
