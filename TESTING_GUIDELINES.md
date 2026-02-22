# Testing Guidelines

## Rust Matcher Style

- Prefer `googletest` matcher assertions (`assert_that!`) over `assert_eq!` and boolean-style assertions.
- For `Option`, prefer semantic matchers:
  - `assert_that!(value, none())`
  - `assert_that!(value, some(eq(expected)))`
- Avoid indirect boolean checks like `assert_that!(value.is_none(), eq(true))`.
- Keep matcher style consistent inside the same module.
