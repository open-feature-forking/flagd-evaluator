# Contributing to flagd-evaluator

Thank you for your interest in contributing to flagd-evaluator! This document provides guidelines and instructions for contributing.

## Table of Contents

- [Code of Conduct](#code-of-conduct)
- [Development Environment Setup](#development-environment-setup)
- [Building the Project](#building-the-project)
- [Testing](#testing)
- [Code Style Guidelines](#code-style-guidelines)
- [Commit Message Guidelines](#commit-message-guidelines)
- [Pull Request Process](#pull-request-process)
- [Reporting Issues](#reporting-issues)

## Code of Conduct

This project adheres to the [OpenFeature Code of Conduct](https://github.com/open-feature/.github/blob/main/CODE_OF_CONDUCT.md). By participating, you are expected to uphold this code.

## Development Environment Setup

### Prerequisites

1. **Rust**: Install Rust using rustup:
   ```bash
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   ```

2. **WASM Target**: Add the WebAssembly target:
   ```bash
   rustup target add wasm32-unknown-unknown
   ```

3. **Clippy and Rustfmt** (usually included by default):
   ```bash
   rustup component add clippy rustfmt
   ```

### Optional Tools

- **wasm-opt**: For optimizing WASM output size
  ```bash
  cargo install wasm-opt
  ```

- **cargo-watch**: For automatic rebuilding during development
  ```bash
  cargo install cargo-watch
  ```

### Clone and Setup

```bash
git clone https://github.com/open-feature-forking/flagd-evaluator.git
cd flagd-evaluator
cargo build
```

## Building the Project

### Development Build

```bash
cargo build
```

### Release Build (Native)

```bash
cargo build --release
```

### WASM Build

```bash
cargo build --target wasm32-unknown-unknown --release
```

The WASM file will be located at: `target/wasm32-unknown-unknown/release/flagd_evaluator.wasm`

## Testing

### Run All Tests

```bash
cargo test
```

### Run Tests with Output

```bash
cargo test -- --nocapture
```

### Run Specific Test

```bash
cargo test test_name
```

### Run Integration Tests Only

```bash
cargo test --test integration_tests
```

### Test Coverage

We aim for >80% test coverage. Consider adding tests for:
- New functionality
- Edge cases
- Error conditions
- All public APIs

## Code Style Guidelines

### Formatting

All code must be formatted with `cargo fmt`:

```bash
cargo fmt
```

Check formatting without modifying files:

```bash
cargo fmt -- --check
```

### Linting

All code must pass `cargo clippy` with no warnings:

```bash
cargo clippy -- -D warnings
```

### Documentation

- All public APIs must have documentation comments
- Use `///` for item documentation
- Use `//!` for module-level documentation
- Include examples where helpful

```rust
/// Evaluates a JSON Logic rule against the provided data.
///
/// # Arguments
/// * `rule` - The JSON Logic rule as a string
/// * `data` - The context data as a JSON string
///
/// # Returns
/// A JSON string with the evaluation result
///
/// # Example
/// ```
/// let result = evaluate("{\"==\": [1, 1]}", "{}");
/// ```
pub fn evaluate(rule: &str, data: &str) -> String {
    // implementation
}
```

### Safety Comments

All `unsafe` blocks must have a safety comment explaining why the code is safe:

```rust
// SAFETY: The pointer is guaranteed to be valid for `len` bytes
// by the caller, and the memory region does not overlap with any
// other mutable references.
unsafe {
    std::ptr::copy_nonoverlapping(src, dst, len);
}
```

### General Guidelines

- Use descriptive variable and function names
- Keep functions focused and small
- Prefer explicit error handling over panics
- Avoid unwrap() in production code - use proper error handling
- Add comments for complex logic

## Commit Message Guidelines

We follow [Conventional Commits](https://www.conventionalcommits.org/) for commit messages. This enables automated changelog generation and semantic versioning via [Release Please](https://github.com/googleapis/release-please).

### Commit Message Format

```
<type>(<scope>): <description>

[optional body]

[optional footer]
```

### Types

**Types that trigger releases:**
- `feat:` - New feature (minor version bump)
- `fix:` - Bug fix (patch version bump)
- `feat!:` or `BREAKING CHANGE:` - Breaking change (major version bump)
- `perf:` - Performance improvement (patch version bump)

**Types that don't trigger releases (changelog only):**
- `docs:` - Documentation changes
- `chore:` - Maintenance tasks
- `test:` - Test updates
- `ci:` - CI/CD changes
- `refactor:` - Code refactoring
- `style:` - Code style/formatting
- `build:` - Build system changes

### Examples

```bash
# Patch release (0.1.0 -> 0.1.1)
git commit -m "fix(operators): correct fractional operator bucket distribution"

# Minor release (0.1.0 -> 0.2.0)
git commit -m "feat(operators): add sem_ver operator for semantic versioning"

# Major release (0.1.0 -> 1.0.0)
git commit -m "feat(api)!: redesign evaluation API with breaking changes

BREAKING CHANGE: evaluate() now returns Result<Value> instead of Value"

# With scope and body
git commit -m "feat(wasm): add memory optimization for large rules

Implements chunked memory allocation for evaluating rules that
exceed the default memory limit."

# Documentation (no release)
git commit -m "docs: update API examples in README"
```

## Pull Request Process

### Before Submitting

1. **Create an issue first** (for significant changes)
   - Describe the problem or feature
   - Discuss the approach

2. **Fork and branch**
   ```bash
   git checkout -b feature/my-feature
   ```

3. **Make your changes**
   - Follow code style guidelines
   - Add tests for new functionality
   - Update documentation if needed

4. **Verify your changes**
   ```bash
   cargo fmt
   cargo clippy -- -D warnings
   cargo test
   cargo build --target wasm32-unknown-unknown --release
   ```

5. **Commit with meaningful messages** (see [Commit Message Guidelines](#commit-message-guidelines))
   ```
   feat: add support for custom operator X
   
   - Implemented operator parsing
   - Added unit tests
   - Updated documentation
   ```

### Submitting

1. Push to your fork:
   ```bash
   git push origin feature/my-feature
   ```

2. Create a Pull Request against `main`

3. Fill out the PR template with:
   - Description of changes
   - Related issues
   - Testing performed
   - Breaking changes (if any)

### Review Process

- All PRs require at least one approval
- CI must pass (tests, clippy, fmt)
- Address review feedback promptly
- Keep PRs focused and reasonably sized

## Reporting Issues

### Bug Reports

Include:
- Description of the bug
- Steps to reproduce
- Expected behavior
- Actual behavior
- Environment details (Rust version, OS, etc.)
- Minimal reproduction code if possible

### Feature Requests

Include:
- Description of the feature
- Use case / motivation
- Proposed solution (if any)
- Alternatives considered

## Questions?

Feel free to open an issue for questions or reach out to the maintainers.

Thank you for contributing!
