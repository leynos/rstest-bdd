# Python exception design, raising, handling, and logging — Ruff TRY/BLE/EM/LOG, N818, PERF203

This guide distils the intent behind Ruff’s Tryceratops (TRY), Blind Except
(BLE), flake8‑errmsg (EM), flake8‑logging (LOG), pep8‑naming N818, and Perflint
PERF203, aligned with practical engineering practice.

## 1) Design a coherent exception hierarchy (N818 + practice)

**Principle:** model failure semantics with a small tree of domain exceptions;
suffix concrete error classes with `Error` (N818). A single package‑level base
class enables callers to catch all domain failures without vendor leakage.

```python
class PaymentsError(Exception):
    """All payment-layer errors."""


class CardDeclinedError(PaymentsError):  # ✅ ends with Error (N818)
    def __init__(self, code: str, *, retry_after: int | None = None):
        super().__init__(f"Card declined ({code})")
        self.code = code
        self.retry_after = retry_after
```

**Practice notes:** group exceptions under a common base; add structured
attributes (codes, identifiers, retry hints) so that business logic need not
parse free‑form strings.

## 2) Raise the right thing, with the right cause (TRY003/TRY004/TRY200/TRY201)

### Prefer specific built‑ins or domain errors over “vanilla” exceptions

```python
# ❌ Avoid
raise Exception("Bad input")

# ✅ Prefer
raise ValueError("Percent must be between 0 and 100")
# …or a domain error
raise CardDeclinedError("insufficient_funds")
```

TRY003 discourages raising `Exception` directly. TRY004 encourages appropriate
built‑ins (`TypeError` for wrong types, `ValueError` for bad values, etc.) or
domain‑specific classes.

### Preserve causal chains with `raise … from …`

```python
try:
    token = decode_jwt(payload)
except jwt.InvalidTokenError as exc:
    raise AuthenticationError("Invalid session token") from exc  # ✅ TRY201
```

When transforming low‑level failures into domain errors, `raise … from …`
retains traceback lineage (TRY201). Avoid discarding causes in contexts
expected to preserve them (TRY200).

## 3) Catch narrowly; avoid blind handlers (BLE001), and use `else` for the happy path (TRY300)

### Avoid blind `except`

```python
# ❌ BLE001: masks defects and unrelated failures
try:
    process(row)
except Exception:
    pass

# ✅ Catch only actionable failures
try:
    process(row)
except (TimeoutError, RateLimitError) as exc:
    backoff_and_retry(exc)
```

BLE001 warns on `except:` and `except Exception:`. Handlers should target
exceptions that can be meaningfully handled.

### Separate success flow with `else`

```python
def reciprocal(n: float) -> float:
    try:
        result = 1 / n
    except ZeroDivisionError:
        log.warning("n was zero")
        return float("inf")
    else:  # ✅ TRY300
        return result
```

`else` emphasizes the happy path and avoids odd control‑flow within `try`
blocks.

## 4) Message construction for raises (EM101/EM102) and logging practice (LOG004/LOG007/LOG009/LOG014/LOG015, TRY401)

### Exception messages: construct once, pass once

```python
name = user.name
# ❌ EM102: f-string passed directly into constructor
raise RuntimeError(f"User {name!r} not found")

# ✅ Build the message, then pass a single object
msg = f"User {name!r} not found"
raise RuntimeError(msg)
```

EM101/EM102 prefer a single message object; this reduces duplication and
clarifies intent.

### Logging: parameterized messages, module loggers, correct APIs

```python
import logging

logger = logging.getLogger(__name__)

# ❌ LOG issues
logging.warning(f"failed for {user_id}")  # f-string (LOG004/LOG014)
logging.warning("failed for %s" % user_id)  # %-formatting (LOG007)
logging.warn("deprecated")  # warn() (LOG009)
logging.error("bad root logger")  # root logger usage (LOG015)

# ✅ Correct
logger.warning("Failed for user_id=%s", user_id)  # lazy interpolation
logger.error("Task %s crashed", task_id)
```

### Logging exceptions: no duplication

```python
try:
    risky()
except ValueError:
    logger.exception("Risky operation failed")  # ✅ includes traceback; no %s with exc
```

`logger.exception` records the active exception and traceback; appending the
exception object to the format arguments is redundant (TRY401).

**Operational note:** log once at a boundary (e.g., request or worker entry
point). Inner layers should handle or re‑raise without logging to avoid
duplicate noise.

## 5) Performance considerations in loops (PERF203)

```python
# ✅ Preserve per-item recovery when later items must still be parsed
for item in items:
    try:
        parse(item)
    except ParseError:
        continue

# ✅ Hoist the try when aborting on the first ParseError is intended
try:
    for item in items:
        parse(item)
except ParseError:
    handle_parse_failure()
```

These forms are not behaviourally equivalent: the inner handler skips a failed
item and continues parsing, while the hoisted handler aborts at the first
`ParseError`. Choose the form that preserves the required recovery semantics.
Exception handling carries overhead on the exceptional path; hoisting the block
can improve throughput in hot loops (PERF203). Treat that as a
micro-optimization guided by profiling.

## 6) Testing: assert specific failures (B017)

```python
# ❌ Overly broad; test may pass for the wrong reason
with pytest.raises(Exception):
    parse("not-json")

# ✅ Narrow and expressive
with pytest.raises(JSONDecodeError, match=r"Expecting value"):
    parse("not-json")
```

B017 flags overly broad exception assertions. Tests should specify the expected
type and, when useful, constrain the message via regex.

## 7) Practical patterns and anti‑patterns

**Handle vs bubble:** handle locally when the code can correct the condition
(retry, substitute, degrade) or add essential context and re‑raise with `from`.
Otherwise, allow bubbling to a layer capable of policy decisions (transaction
rollback, HTTP 5xx, CLI exit code).

**No “log and re‑raise” chains:** log exactly once at a suitable boundary.
Intermediate layers should either resolve the problem or propagate it.

**Built‑ins with intent:** `ValueError` for bad values, `TypeError` for wrong
types, `NotImplementedError` for abstract methods; avoid `RuntimeError` as a
catch‑all where a domain error or specific built‑in communicates intent better.

## 8) Reference examples (good vs bad)

### Wrapping vendor errors into domain errors

```python
def charge(amount_pennies: int, card_token: str) -> str:
    try:
        return gateway.charge(amount_pennies, card_token)
    except gateway.Timeout as exc:
        raise PaymentsError("Gateway timeout") from exc  # ✅ TRY201
    except gateway.CardDeclined as exc:
        raise CardDeclinedError(exc.code, retry_after=60) from exc
```

### Boundary logging (single place)

```python
def worker_main() -> None:
    try:
        process_job()
    except PaymentsError:
        logger.exception("Job failed due to payments error")  # log once, then propagate
        raise
```

### Building exception messages (EM) and logging payloads (LOG)

```python
def must_have_key(d: dict, key: str) -> None:
    if key not in d:
        msg = f"Missing required key: {key!r}"
        raise KeyError(msg)


order_id = "ord_123"
shop_id = "shop_456"
logger.info("Dispatching order_id=%s to shop_id=%s", order_id, shop_id)  # structured
```

### `try/except` in loops (PERF203) and `else` usage (TRY300)

```python
def parse_all(raw_items: list[str]) -> list[Record]:
    parsed: list[Record] = []
    try:  # ✅ PERF203 hoist
        for raw in raw_items:
            rec = parse_record(raw)
            parsed.append(rec)
    except ParseError:
        logger.exception("Parsing aborted")
    else:  # ✅ TRY300: success-only post-processing
        logger.info("Parsed %s records", len(parsed))
    return parsed
```

### Tests with specific exceptions (B017)

```python
def test_amount_must_be_int() -> None:
    with pytest.raises(TypeError, match="amount_pennies"):
        charge("12.34", "tok_abc")  # wrong type triggers TypeError
```

## 9) Minimal Ruff configuration to enforce these rules

```toml
# pyproject.toml
[tool.ruff]
target-version = "py314"

[tool.ruff.lint]
select = [
  "TRY",      # Tryceratops
  "BLE",      # blind-except
  "EM",       # flake8-errmsg
  "LOG",      # flake8-logging
  "N818",     # exception names end with Error
  "PERF203",  # try/except in loop
  "B017",     # assert-raises-exception
]
```

## 10) One‑page policy for repositories

> **Exceptions are part of the public API.** Define a small hierarchy with a
> package base and `*Error` suffix; raise specific types; wrap external
> failures with `raise … from …`; catch only what can be handled; use `else`
> for the happy path; avoid `try/except` in hot loops; never format log
> messages directly; log exceptions once at a boundary via `logger.exception`.
> Enforce with Ruff (TRY/BLE/EM/LOG/N818/PERF203/B017).

## 11) References

- Ruff rules: Tryceratops (TRY), Blind Except (BLE001), flake8‑errmsg
  (EM101/EM102), flake8‑logging (LOG004/LOG007/LOG009/LOG014/LOG015), N818,
  PERF203, B017.
  - [https://docs.astral.sh/ruff/rules/#tryceratops-try](https://docs.astral.sh/ruff/rules/#tryceratops-try)
  - [https://docs.astral.sh/ruff/rules/blind-except/](https://docs.astral.sh/ruff/rules/blind-except/)
  - [https://docs.astral.sh/ruff/rules/assert-raises-exception/](https://docs.astral.sh/ruff/rules/assert-raises-exception/)
  - [https://docs.astral.sh/ruff/rules/#flake8-errmsg-em](https://docs.astral.sh/ruff/rules/#flake8-errmsg-em)
  - [https://docs.astral.sh/ruff/rules/#flake8-logging-log](https://docs.astral.sh/ruff/rules/#flake8-logging-log)
  - [https://docs.astral.sh/ruff/rules/error-suffix-on-exception-name/](https://docs.astral.sh/ruff/rules/error-suffix-on-exception-name/)
  - [https://docs.astral.sh/ruff/rules/try-except-in-loop/](https://docs.astral.sh/ruff/rules/try-except-in-loop/)
- Gui Commits practice notes:
  - Exception structure:
    [https://guicommits.com/how-to-structure-exception-in-python-like-a-pro/](https://guicommits.com/how-to-structure-exception-in-python-like-a-pro/)
  - Logging guidance:
    [https://guicommits.com/how-to-log-in-python-like-a-pro/](https://guicommits.com/how-to-log-in-python-like-a-pro/)
