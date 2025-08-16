# todo-cli

Minimal command-line todo list demonstrating `rstest-bdd`.

## Build and test

```bash
cargo test
# NOTE: tasks exist only for the duration of a single process
cargo run -- add "Buy milk"   # adds in this process, then exits
cargo run -- list             # runs in a new process; prints an empty list
```

Each invocation starts with an empty list; no state is persisted between runs.
