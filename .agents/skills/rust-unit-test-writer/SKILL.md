---
name: rust-unit-test-writer
description: Write and improve Rust unit tests for crate code. Use when adding `#[test]` or `#[tokio::test]` cases, increasing coverage, validating edge and error paths, fixing flaky tests, or organizing tests in `src/*.rs` and `tests/*.rs`.
---

# Rust Unit Test Writer

Write deterministic, readable Rust tests that prove behavior instead of implementation details.

## Workflow

1. Read each implementation branch and derive expected outcomes before writing assertions.
2. Choose the right location.
   - Place implementation-focused tests in `#[cfg(test)] mod tests` next to the target code.
   - Place cross-module or public API checks in `tests/` as integration tests.
3. Write tests with Arrange-Act-Assert structure and descriptive names.
4. Cover a minimum of three paths: success path, boundary path, and failure path.
5. Add conversion edge cases when code performs type conversion (for example `as_i64`, `try_from`, parsing).
6. Run narrow tests first, then broader suites.
   - `cargo test <test_name>`
   - `cargo test -p <crate_name>`
   - `cargo test`

## Branch-to-Test Mapping

Before finalizing tests, create a quick mapping from each match/if branch to at least one test case.

- Include at least one unsupported-type case when function input is `serde_json::Value` or enums.
- For numeric conversions, assert behavior for valid numbers, zero, and non-representable numbers (for example float or overflowed integer).
- Prefer table-driven loops for repetitive input/output checks.

## Unit Test Template

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_valid_value() {
        // Arrange
        let input = "42";

        // Act
        let value = parse_value(input).expect("expected valid number");

        // Assert
        assert_eq!(value, 42);
    }
}
```

## Async and Error Patterns

Use async tests only when the target function is async and the runtime already exists in the crate.

```rust
#[tokio::test]
async fn returns_not_found_for_missing_key() {
    let repo = InMemoryRepo::default();

    let result = repo.get("missing").await;

    assert!(matches!(result, Err(RepoError::NotFound)));
}
```

Use panic tests only for explicit panic contracts.

```rust
#[test]
#[should_panic(expected = "capacity must be > 0")]
fn panics_when_capacity_is_zero() {
    RingBuffer::new(0);
}
```

## Assertions and Test Quality Rules

- Assert specific outcomes (`assert_eq!`, `matches!`) instead of broad checks.
- Verify returned errors by variant and meaning, not only by `is_err()`.
- Keep one behavior per test; split mixed assertions into separate tests.
- Avoid asserting private implementation details unless they are part of the contract.
- Use helper builders only when they reduce duplication across multiple tests.
- Remove redundant tests that duplicate the same branch coverage.

## Determinism Rules

- Replace network, clock, randomness, and filesystem dependencies with fakes or injected traits.
- Avoid sleeps and timing-sensitive assertions.
- Avoid shared mutable state across tests.
- Keep tests independent so they can run in any order.

## Completion Checklist

- Ensure new tests fail before the fix and pass after the fix when possible.
- Ensure all tests compile and pass with `cargo test`.
- Ensure names use `snake_case` and describe behavior clearly.
- Ensure setup code is minimal and local to each test unless reused broadly.
- Ensure every branch in the tested function has at least one explicit test.
