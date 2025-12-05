# 🛡️ Reliable testing in Rust via dependency injection

Writing robust, reliable, and parallelizable tests requires an intentional
approach to handling external dependencies such as environment variables, the
filesystem, or the system clock. Functions that directly call `std::env::var`
or `SystemTime::now()` are difficult to test because they depend on global,
non-deterministic state.

This leads to several problems:

- **Flaky Tests:** A test might pass or fail depending on the environment it
  runs in.
- **Parallel Execution Conflicts:** Tests that modify the same global
  environment variable (`std::env::set_var`) will interfere with each other
  when run with `cargo test`.
- **State Corruption:** A test that panics can fail to clean up its changes to
  the environment, poisoning subsequent tests.

The solution is a classic software design pattern: **Dependency Injection
(DI)**. Instead of a function reaching out to the global state, its
dependencies are provided as arguments. The `mockable` crate offers a
convenient set of traits (`Env`, `Clock`, etc.) to implement this pattern for
common system interactions in Rust.

______________________________________________________________________

## ✨ Mocking environment variables

### 1. Add `mockable`

First, add the crate to development dependencies in `Cargo.toml`.

```toml
[dev-dependencies]
mockable = { version = "0.1.4", default-features = false, features = ["clock", "mock"] }
```

### 2. The untestable code (before)

Directly calling `std::env` makes it difficult to test all logic paths
exhaustively.

```rust,no_run
pub fn get_api_key() -> Option<String> {
    match std::env::var("API_KEY") {
        Ok(key) if !key.is_empty() => Some(key),
        _ => None,
    }
}
```

### 3. Refactoring for testability (after)

The function is refactored to accept a generic type that implements the
`mockable::Env` trait.

```rust,no_run
use mockable::Env;

pub fn get_api_key(env: &impl Env) -> Option<String> {
    match env.var("API_KEY") {
        Ok(key) if !key.is_empty() => Some(key),
        _ => None,
    }
}
```

The function's core logic remains unchanged, but its dependency on the
environment is now explicit and injectable.

### 4. Writing isolated unit tests

Tests can use `MockEnv`, an in-memory mock, to simulate any environmental
condition without touching the actual process environment.

```rust,no_run
#[cfg(test)]
mod tests {
    use super::*;
    use mockable::{MockEnv, Env};

    #[test]
    fn test_get_api_key_present() {
        let mut env = MockEnv::new();
        env.set_var("API_KEY", "secret123");
        assert_eq!(get_api_key(&env), Some("secret123".to_string()));
    }

    #[test]
    fn test_get_api_key_missing() {
        let env = MockEnv::new();
        assert_eq!(get_api_key(&env), None);
    }

    #[test]
    fn test_get_api_key_present_but_empty() {
        let mut env = MockEnv::new();
        env.set_var("API_KEY", "");
        assert_eq!(get_api_key(&env), None);
    }
}
```

These tests are fast, completely isolated from each other, and will never fail
due to external state.

### 5. Usage in production code

In production code, inject the "real" implementation, `RealEnv`, which calls
the actual `std::env` functions.

```rust,no_run
use mockable::RealEnv;

fn main() {
    let env = RealEnv::new();
    if let Some(api_key) = get_api_key(&env) {
        println!("API Key found!");
    } else {
        println!("API Key not configured.");
    }
}
```

______________________________________________________________________

## 🔩 Handling other non-deterministic dependencies

This dependency injection pattern also applies to other non-deterministic
dependencies, such as the system clock. The `mockable` crate provides a `Clock`
trait for this purpose.

### Untestable code

```rust,no_run
use std::time::{SystemTime, Duration};

fn is_cache_entry_stale(creation_time: SystemTime) -> bool {
    let timeout = Duration::from_secs(300);
    match SystemTime::now().duration_since(creation_time) {
        Ok(age) => age > timeout,
        Err(_) => false,
    }
}
```

### Testable refactor

```rust,no_run
use mockable::Clock;
use std::time::{SystemTime, Duration};

fn is_cache_entry_stale(creation_time: SystemTime, clock: &impl Clock) -> bool {
    let timeout = Duration::from_secs(300);
    match clock.now().duration_since(creation_time) {
        Ok(age) => age > timeout,
        Err(_) => false,
    }
}
```

### Testing with `MockClock`

```rust,no_run
#[cfg(test)]
mod tests {
    use super::*;
    use mockable::{MockClock, Clock};
    use std::time::{Duration, SystemTime};

    #[test]
    fn test_cache_is_not_stale() {
        let mut clock = MockClock::new();
        let creation_time = clock.now();
        clock.advance(Duration::from_secs(100));
        assert!(!is_cache_entry_stale(creation_time, &clock));
    }

    #[test]
    fn test_cache_is_stale() {
        let mut clock = MockClock::new();
        let creation_time = clock.now();
        clock.advance(Duration::from_secs(301));
        assert!(is_cache_entry_stale(creation_time, &clock));
    }
}
```

In production, an instance of `RealClock::new()` would be used.

______________________________________________________________________

## 📌 Key takeaways

- **The Problem is Non-Determinism:** Directly accessing global state like
  `std::env` or `SystemTime::now` makes code difficult to test exhaustively.
- **The Solution is Dependency Injection:** Pass dependencies into functions as
  arguments.
- **Use** `mockable` **Traits:** Abstract dependencies behind traits such as
  `impl Env` or `impl Clock`.
- **`Mock*` for Tests:** Use `MockEnv` and `MockClock` in unit tests for
  isolated, deterministic control.
- **`Real*` for Production:** Use `RealEnv` and `RealClock` in the application
  to interact with the actual system.
- **`RealEnv` is NOT a Scope Guard:** `RealEnv` directly mutates the global
  process environment without automatic cleanup. For integration tests that
  require modifying the live environment, consider a crate such as `temp_env`.
  For unit tests, `MockEnv` is preferable.
