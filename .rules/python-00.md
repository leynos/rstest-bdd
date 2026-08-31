# Python 3.12 Code Style Guidelines (with Ruff, ty, and pytest)

## Naming Conventions

- **Directories:** Use *snake_case* for top-level features or modules (e.g.,
  `data_pipeline`, `user_auth`).
- **Files:** Use *snake_case.py*; name for contents (e.g., `http_client.py`,
  `task_queue.py`).
- **Classes:** Use *PascalCase*.
- **Variables & Functions:** Use *snake_case*.
- **Constants:** Use *UPPER_SNAKE_CASE* for module-level constants.
- **Private/Internal:** Prefix with a single underscore (`_`) for non-exported
  helpers or internal APIs.

## Python Typing Practices

- **Use typing everywhere.** Enable and maintain full static type coverage. Use
  ty for type-checking.
- **Use `TypedDict` or `Dataclass` for structured data where appropriate.** For
  internal-only usage, prefer `@dataclass(slots=True)`.
- **Avoid `Any`.** Use a specific type, generics, or `cast()` when
  necessary—always document why `Any` is acceptable if used.
- **Be explicit with returns.** Use `-> None`, `-> str`, etc., for all public
  functions and class methods.
  - **Favour immutability.** Prefer tuples to lists, and `frozendict` or
    `types.MappingProxyType` where appropriate.

## Tooling and Runtime Practices

- **Enable Ruff.** Use Ruff to lint for performance, security, consistency, and
  style issues. Enable fixers and formatters.
- Use `pyproject.toml` to configure tools like Ruff, ty, and Pytest.
- **Treat all ty diagnostics as CI errors.** Fix the underlying type mismatch.
  Suppress a diagnostic only as a last resort, at the narrowest scope, with an
  explanation of the external constraint.
- **Avoid side effects at import time.** Modules should not modify global state
  or perform actions on import.
- **Use `.env` or settings modules** for environment-specific configuration.
  Never hardcode secrets.

## Linting and Formatting

- **Use Ruff for linting** (replacing flake8, isort, pyflakes, etc.).
- **Use Ruff for formatting**. Let Ruff handle whitespace and formatting
  entirely—don't fight it.

## Documentation

- **Use docstrings.** Document public functions, classes, and modules using
  NumPy format. For example:

```python
def scale(values: list[float], factor: float) -> list[float]:
    """
    Scale a list of numbers by a given factor.

    Parameters
    ----------
    values : list of float
        The list of numeric values to scale.
    factor : float
        The multiplier to apply to each value.

    Returns
    -------
    list of float
        The scaled numeric values.
    """
    return [v * factor for v in values]
```

- **Explain tricky code.** Use inline comments for non-obvious logic or
  decisions.
- **Colocate documentation.** Keep README.md or `docs/` near reusable packages;
  include usage examples.

## Testing with pytest

- **Colocate unit tests with code** using an `unittests` subdirectory and a
  `test_` prefix. This keeps logic and its tests together:

```text
user_auth/
  models.py
  login_flow.py
  unittests/
    test_models.py
    test_login_flow.py
```

- **Structure integration tests separately.** When tests span multiple
  components, use `tests/integration/`:

  ```text
  tests/
    integration/
      test_login_flow.py
      test_user_onboarding.py
  ```

- **Use `pytest` idioms.** Prefer fixtures over setup/teardown methods.
  Parametrize broadly. Avoid unnecessary mocks.

- **Group related tests** using `class` with method names prefixed by `test_`.

- **Write tests from a user's perspective.** Test public behaviour, not
  internals.

- **Avoid mocking too much.** Prefer test doubles only for external services or
  non-deterministic behaviours.

## Example

```python
# login_flow.py
def login_user(username: str, password: str) -> bool:
    """Return True if the user is authenticated."""
    ...


# login_flow_test.py
def test_login_success():
    assert login_user("alice", "correct-password") is True


def test_login_failure():
    assert not login_user("alice", "wrong-password")
```

______________________________________________________________________

This style guide aims to foster clean, consistent, and maintainable Python 3.12
code with modern tooling. The priority is correctness, clarity, and developer
empathy.
