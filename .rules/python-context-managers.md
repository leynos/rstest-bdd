# Using context managers for cleanup and resource management

Use context managers to encapsulate setup and teardown logic cleanly and
safely. This reduces the risk of forgetting to release resources (files, locks,
connections, etc.) and simplifies error handling.

Context managers can be written either with `contextlib.contextmanager` (for
simple procedural control flow) or by implementing `__enter__` and `__exit__`
in a class (for more complex or stateful use cases).

## Why use context managers?

- **Safety:** Ensures cleanup occurs even if an exception is raised.
- **Clarity:** Reduces boilerplate and visually scopes side effects.
- **Reuse:** Common setup/teardown logic becomes reusable and composable.

______________________________________________________________________

## Example: using `contextlib.contextmanager`

Use this for straightforward procedural setup/teardown:

```python
from contextlib import contextmanager


@contextmanager
def managed_file(path: str, mode: str):
    f = open(path, mode)
    try:
        yield f
    finally:
        f.close()


# Usage:
with managed_file("/tmp/data.txt", "w") as f:
    f.write("hello")
```

This avoids repeating `try/finally` in every file access.

______________________________________________________________________

## Example: using a class-based context manager

Use this when state or lifecycle logic spans methods:

```python
class Resource:
    def __enter__(self):
        self.conn = connect()
        return self.conn

    def __exit__(self, exc_type, exc_val, exc_tb):
        self.conn.close()


# Usage:
with Resource() as conn:
    conn.send("ping")
```

This keeps state encapsulated and makes testing easier.

______________________________________________________________________

## When to use which

- Use `@contextmanager` when control flow is linear and no persistent state is
  required.

- Use a class when:

  - There is internal state or methods tied to the resource lifecycle.
  - Support for re-entry and more advanced context features is required.

______________________________________________________________________

## Common use cases

- File or network resource handling
- Lock acquisition and release
- Temporary changes to environment (e.g., `os.chdir`, `patch`, `tempfile`)
- Logging scope control or tracing
- Transaction control in databases or services

______________________________________________________________________

## Don't do this

```python
f = open("file.txt")
try:
    process(f)
finally:
    f.close()
```

## Do this instead

```python
with open("file.txt") as f:
    process(f)
```

Context managers make intent and error handling explicit. Prefer them over
manual `try/finally` for clearer, safer code.
