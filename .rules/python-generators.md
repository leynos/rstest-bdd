# Prefer generators over complex loop logic

Using generators improves readability, composability, and memory efficiency.
Functions built as generators are often simpler to test, debug, and refactor.
This guidance encourages breaking apart complex `for`-loops into generator
expressions or functions using `yield`.

## Why prefer generators?

- **Clarity:** Isolating data flow from control flow clarifies logic.
- **Efficiency:** Generators are lazy; they avoid building intermediate data
  structures unless needed.
- **Composability:** Generators can be pipelined with other iterators using
  `itertools` or comprehensions.

## Example: filtering and transforming

### Complex loop (harder to read/test)

```python
def get_names(users):
    result = []
    for user in users:
        if user.active and user.name:
            result.append(user.name.upper())
    return result
```

### Generator-based version (clearer)

```python
def iter_user_names(users):
    for user in users:
        if user.active and user.name:
            yield user.name.upper()


def get_names(users):
    return list(iter_user_names(users))
```

Or with a comprehension:

```python
def get_names(users):
    return [user.name.upper() for user in users if user.active and user.name]
```

## Example: chaining filters and mappings

```python
from itertools import islice


def top_active_emails(users):
    emails = (
        user.email.lower() for user in users if user.active if user.email is not None
    )
    return list(islice(emails, 10))
```

## Use generators when

- Iterating while filtering or mapping data.
- Early returns or short-circuit behaviour need clearer structure.
- The function logically produces a sequence over time.

## Avoid overcomplicating

Don't convert everything into generators unnecessarily. Use them to simplify
logic—not obscure it.

### Bad

```python
def iter_numbers():
    yield from (x * 2 for x in range(10) if x % 2 == 0)
```

### Better

```python
def iter_even_doubles():
    for x in range(10):
        if x % 2 == 0:
            yield x * 2
```

______________________________________________________________________

**Rule of thumb:** If a `for` loop has multiple branches, mutations, or is
hard to explain in one sentence, try rewriting it as a generator.

Prefer clear, linear data flows over deeply nested conditionals and loop bodies.
