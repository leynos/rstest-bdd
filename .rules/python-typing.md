# Advanced Typing and Language Features (Python 3.12)

> This section documents Python 3.12 typing features and best
> practices to improve clarity, correctness, and tooling support. Use these
> features to write expressive, modern Python.

Python Enhancement Proposal (PEP) numbers are used throughout this guide for
the relevant typing changes.

## `enum.Enum`, `enum.IntEnum`, `enum.StrEnum`

Use `Enum` for fixed sets of related constants. Use `enum.auto()` to avoid
repeating values manually. Use `IntEnum` or `StrEnum` when interoperability
with integers or strings is required (e.g. for database or JSON serialization).

```python
import enum


class Status(enum.Enum):
    PENDING = enum.auto()
    COMPLETE = enum.auto()


class ErrorCode(enum.IntEnum):
    OK = 0
    NOT_FOUND = 404


class Role(enum.StrEnum):
    ADMIN = enum.auto()
    GUEST = enum.auto()
```

Use `auto()` when exact values are unimportant and duplication adds no value.
Avoid `auto()` in `IntEnum` where numeric meaning matters.

## `match` / `case` (Structural Pattern Matching)

Use structural pattern matching for branching over structured data. This is
especially useful for enums, discriminated unions, or pattern-rich data
structures.

```python
def handle_status(status: Status) -> str:
    match status:
        case Status.PENDING:
            return "Still processing"
        case Status.COMPLETE:
            return "Done"
```

## Generic Class Declarations (PEP 695)

Use bracketed class-level type variables directly for generic class
declarations.

```python
class Box[T]:
    def __init__(self, value: T):
        self.value = value
```

This is cleaner and avoids the indirection of separate `TypeVar` declarations.

## `Self` Type (PEP 673)

Use `Self` in fluent interfaces and builder-style APIs to indicate the method
returns the same instance.

```python
import typing


class Builder:
    def add(self, value: int) -> typing.Self:
        self.values.append(value)
        return self
```

This improves tool support and enforces correct chaining semantics.

## `@override` Decorator (PEP 698)

Use `@override` to indicate that a method overrides one from a superclass. This
enables static analysis tools to detect typos and signature mismatches.

```python
import typing


class Base:
    def run(self) -> None: ...


class Child(Base):
    @typing.override
    def run(self) -> None:
        print("Running")
```

This decorator is a no-op at runtime but improves tooling correctness.

## `TypeGuard` (PEP 647)

Use `TypeGuard[T]` to define custom runtime type guards that narrow types in
type checkers.

```python
import typing


def is_str_list(val: list[object]) -> typing.TypeGuard[list[str]]:
    return all(isinstance(x, str) for x in val)
```

Unlike `isinstance`, this informs the type checker that `val` is now
`list[str]` when the guard returns true. The later `TypeIs` (PEP 742) is not
available on this repository's Python 3.12 baseline; use it only through a
deliberately supported compatibility dependency.

## Defaults for TypeVars (PEP 696)

TypeVar defaults were introduced after Python 3.12. Do not pass `default=` to
`typing.TypeVar` in code targeting this repository's baseline. Use an explicit
overload, factory, or value default instead.

```python
T = typing.TypeVar("T")


class Box[T]:
    def __init__(self, value: T):
        self.value = value
```

Choose the default explicitly at the API boundary, for example with a factory
whose return type is `Box[int]`.

## Standard Library Generics (PEP 585)

Use built-in generics from the standard library (`list`, `dict`, `tuple`, etc.)
instead of `typing.List`, `typing.Dict`, etc.

```python
names: list[str] = ["Alice", "Bob"]
```

This reduces imports and reflects the modern style.

## Union Syntax and Optional (PEP 604)

Use `|` to write union types, and `A | None` instead of `Optional[A]`.

```python
value: int | None = None
```

This is more concise and readable, especially for nested types.

## Type Aliases using `type`

Use the `type` keyword to create type aliases with better IDE and runtime
support.

```python
type StrDict = dict[str, str]
```

This replaces `StrDict = TypeAlias = ...` and is preferred in modern Python.

When a documented compatibility contract requires Python < 3.12, keep the
older `typing.TypeAlias` syntax and use a narrowly scoped
`# ruff: ignore[non-pep695-type-alias]` with the compatibility reason. Place
alias definitions after the import block and group shared aliases in a shared
types module to avoid duplication.

## `from __future__ import annotations`

Python 3.12 evaluates annotations at definition time by default. Use this
import when forward references, circular imports, or types imported only under
`TYPE_CHECKING` would otherwise fail at runtime.

```python
from __future__ import annotations
```

When a runtime consumer inspects concrete annotation objects, omit the import
or provide the referenced types at runtime instead. The project baseline is
Python 3.12.

## `if typing.TYPE_CHECKING`

Use this conditional to guard imports required only for static typing.

```python
import typing

if typing.TYPE_CHECKING:
    from mypackage.internal import InternalType
```

This avoids runtime import costs or circular imports.

## Standard Aliases

Use the following import aliases consistently:

```python
import datetime as dt
import collections.abc as cabc
```

This simplifies common types such as `dt.datetime`, `cabc.Iterable`,
`cabc.Callable`, and helps disambiguate usage.

______________________________________________________________________

These conventions promote clarity, tool compatibility, and maintainable Python.
