# rstest-bdd-server

Language Server Protocol (LSP) implementation for the rstest-bdd BDD testing
framework. Provides IDE integration for navigation between Rust step
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

| Variable                     | Default | Description                                     |
| ---------------------------- | ------- | ----------------------------------------------- |
| `RSTEST_BDD_LSP_LOG_LEVEL`   | `info`  | Log verbosity (trace, debug, info, warn, error) |
| `RSTEST_BDD_LSP_DEBOUNCE_MS` | `300`   | Delay before processing file changes            |

## Editor Configuration

See the [User Guide](../../docs/users-guide.md) for editor-specific setup
instructions.

## License

ISC
