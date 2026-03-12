# flagd-evaluator Copilot Instructions

This document provides comprehensive context about the flagd-evaluator repository for GitHub Copilot and developers.

## Overview of the Repository

flagd-evaluator is a **Rust-based feature flag evaluation engine** that replaces per-language JSON Logic implementations (json-logic-java, json-logic-utils, etc.) with a single core — one implementation, one test suite, consistent behavior everywhere. Thin wrapper libraries expose it via WASM runtimes (Java/Chicory, Go/wazero) or native bindings (Python/PyO3). The best integration strategy is chosen per language based on benchmarks — e.g., Python benchmarks showed PyO3 native bindings outperform WASM (wasmtime-py), while Go and Java perform well with their WASM runtimes. See [BENCHMARKS.md](../BENCHMARKS.md) for the full comparison matrix.

The evaluator is used across all OpenFeature flagd providers to ensure uniform evaluation behavior regardless of the programming language being used. For detailed information about how providers use this evaluator, see the <a href="https://github.com/open-feature/flagd/blob/main/docs/reference/specifications/providers.md">providers.md documentation</a> in the flagd docs.

## Architecture & Purpose

### Architecture at a Glance

```
src/
├── lib.rs          # WASM exports (update_state, evaluate, alloc, dealloc)
├── evaluator.rs    # Instance-based FlagEvaluator, flag evaluation and state logic
├── memory.rs       # WASM memory management, pointer packing
├── error.rs        # Error types and handling
├── types.rs        # EvaluationResult, ErrorCode, ResolutionReason
├── validation.rs   # JSON Schema validation (boon crate)
├── operators/      # Custom operators: fractional, sem_ver (starts_with/ends_with from datalogic-rs)
│   ├── fractional.rs
│   ├── sem_ver.rs
│   └── common.rs
└── model/          # Flag configuration data structures
    └── feature_flag.rs
```

**Key concepts:**
- **Packed u64 returns** — All WASM exports return upper 32 bits = pointer, lower 32 bits = length
- **Thread-local storage** — Flag state stored per-thread; `update_state` detects changed flags
- **Context enrichment** — `$flagd.flagKey`, `$flagd.timestamp`, and `targetingKey` auto-injected
- **Instance-based** — `FlagEvaluator` struct (in `evaluator.rs`) holds state per-instance; no global state

See [ARCHITECTURE.md](../ARCHITECTURE.md) for the full design, memory model, error handling, and cross-language integration patterns.

### In-Process Evaluation

This evaluator implements the **in-process evaluation logic** described in the <a href="https://github.com/open-feature/flagd/blob/main/docs/reference/specifications/providers.md#in-process-resolver">In-Process Resolver section</a> of the flagd providers specification. It allows feature flag evaluation to happen directly within the application process without requiring network calls to a separate flagd server.

Key characteristics:
- Evaluates feature flags locally using stored flag configurations
- Processes targeting rules using JsonLogic with custom operators
- Maintains flag state in memory for fast evaluation
- Returns standardized evaluation results with variant, reason, and error information

### Core Functionality

1. **JSON Logic Evaluation** - Full support for <a href="https://jsonlogic.com/">JSON Logic</a> operations via <a href="https://github.com/cozylogic/datalogic-rs">datalogic-rs</a>
2. **Custom Operators** - Feature-flag specific operators for:
   - `fractional` - Consistent bucketing for A/B testing and gradual rollouts
   - `sem_ver` - Semantic version comparison (=, !=, <, <=, >, >=, ^, ~)
   - `starts_with` / `ends_with` - String prefix/suffix matching (provided by datalogic-rs)
3. **Flag State Management** - Internal storage for flag configurations with `update_state` API
4. **Memory Safe Operations** - Clean memory management with explicit alloc/dealloc functions

## Key Documentation References

### Primary Specifications

- **<a href="https://github.com/open-feature/flagd/blob/main/docs/reference/specifications/providers.md">flagd Providers Specification</a>** - Describes how providers should integrate with flagd
  - **<a href="https://github.com/open-feature/flagd/blob/main/docs/reference/specifications/providers.md#in-process-resolver">In-Process Resolver</a>** - Details on how this evaluator is used
  - Evaluation results format (value, variant, reason, error codes)
  - Flag configuration schema

- **<a href="https://flagd.dev/reference/specifications/custom-operations/">flagd Custom Operations Specification</a>** - Complete documentation of custom operators
  - Fractional operator for A/B testing
  - Semantic version comparison
  - String comparison operators

- **<a href="https://flagd.dev/reference/flag-definitions/">flagd Flag Definitions</a>** - Schema for flag configurations
  - Flag state (ENABLED/DISABLED)
  - Variants and default variant
  - Targeting rules using JsonLogic

### Deep-Dive References

| Topic | File |
|-------|------|
| Architecture, memory model, cross-language integration | [ARCHITECTURE.md](../ARCHITECTURE.md) |
| Build commands, code style, commit conventions, PR process | [CONTRIBUTING.md](../CONTRIBUTING.md) |
| Benchmark matrix, performance expectations, scale testing | [BENCHMARKS.md](../BENCHMARKS.md) |
| Python bindings (PyO3), building, testing, CI/CD | [python/README.md](../python/README.md) |
| Java library, Chicory integration | [java/README.md](../java/README.md) |
| API reference, usage examples, custom operators | [README.md](../README.md) |
| Host function requirements (timestamp, random) | [HOST_FUNCTIONS.md](../HOST_FUNCTIONS.md) |

### Related Technologies

- **<a href="https://jsonlogic.com/">JSON Logic</a>** - The rule evaluation engine
- **<a href="https://github.com/cozylogic/datalogic-rs">datalogic-rs</a>** - Rust implementation of JSON Logic
- **<a href="https://github.com/nicknisi/chicory">Chicory</a>** - Pure Java WebAssembly runtime (no JNI required)
- **<a href="https://pyo3.rs/">PyO3</a>** - Rust-Python native bindings

## Relationship to flagd Ecosystem

### Part of the OpenFeature/flagd Project

This repository is a critical component of the larger <a href="https://openfeature.dev/">OpenFeature</a> and <a href="https://flagd.dev/">flagd</a> ecosystem:

- **OpenFeature** - An open standard for feature flag management
- **flagd** - A feature flag daemon that implements the OpenFeature specification
- **flagd-evaluator** (this repository) - The shared evaluation engine used by all in-process providers

### Used by Multiple Language Providers

Language-specific providers embed this evaluator. The integration approach is chosen per language based on benchmark results:

- **Java** - WASM via Chicory (pure Java runtime, no JNI)
- **Go** - WASM via wazero (pure Go runtime)
- **Python** - Native bindings via PyO3 (benchmarks showed 5–10× faster than wasmtime-py)
- **JavaScript/TypeScript** - WASM via Node.js or browser WASM runtimes
- **.NET** - WASM via Wasmtime or other .NET-compatible WASM runtimes

### Consistent Evaluation Across All Providers

The primary benefit of using a shared evaluator is **consistency**:

- Same targeting logic across all language implementations
- Identical fractional bucketing results regardless of language
- Synchronized custom operator behavior
- Uniform error handling and response formats
- Single source of truth for evaluation logic

## Technical Details

### Exported WASM Functions

The evaluator exports these functions for use by host applications:

1. **`evaluate_logic(rule_ptr, rule_len, data_ptr, data_len) -> u64`**
   - Direct JSON Logic evaluation
   - Returns packed pointer (upper 32 bits = ptr, lower 32 bits = length)

2. **`update_state(config_ptr, config_len) -> u64`**
   - Updates internal flag configuration
   - Must be called before using `evaluate`

3. **`evaluate(flag_key_ptr, flag_key_len, context_ptr, context_len) -> u64`**
   - Evaluates a feature flag from stored configuration
   - Returns standardized evaluation result

4. **`alloc(len) -> *mut u8`**
   - Allocates memory in WASM linear memory

5. **`dealloc(ptr, len)`**
   - Frees previously allocated memory

### Memory Management

- Caller is responsible for allocating input buffers and freeing result buffers
- All data is passed as UTF-8 encoded JSON strings
- Results are returned as packed 64-bit pointers
- No garbage collection - explicit dealloc required

### Custom Operators Implementation

Located in `src/operators/`:
- `fractional.rs` - MurmurHash3-based consistent bucketing
- `sem_ver.rs` - Semantic version parsing and comparison
- `starts_with` / `ends_with` - Provided by datalogic-rs, no separate files

## Development Workflow

### Issue First

Always create a GitHub issue before starting work. This ensures traceability and clear scope.

```bash
gh issue create --title "feat(go): add Go WASM bindings" --body "Description of the work"
```

### Work in Worktrees

All feature work happens in git worktrees under `./worktrees/`. This keeps the main working directory clean and allows parallel work on multiple issues.

```bash
# Create a branch and worktree for the issue
git worktree add worktrees/<short-name> -b feat/<short-name>

# Example for issue #42
git worktree add worktrees/go-bindings -b feat/go-bindings

# Work inside the worktree
cd worktrees/go-bindings
```

Branch naming should match the issue scope (e.g., `feat/go-bindings`, `fix/memory-leak`, `refactor/storage`).

### Plan Before Implementing

Before writing any code for an issue, **always enter planning mode** first. This ensures the approach is sound before investing effort.

- Present the plan for approval before writing code
- Clarify ambiguous requirements before starting

### Use Sub-Agents

Leverage sub-agents liberally:

- **Explore agents** for codebase research and understanding existing patterns
- **General-purpose agents** for multi-step research and implementation tasks

Launch multiple agents **in parallel** when their work is independent. This maximizes throughput.

### Workflow Summary

1. **Create a GitHub issue** describing the work
2. **Create a worktree** under `./worktrees/` on a feature branch
3. **Plan the approach** before writing code
4. **Implement** with regular commits referencing the issue
5. **Run tests** before creating a PR
6. **Create a PR** linking back to the issue

### Building

```bash
# Native build (for development/testing)
cargo build

# WASM build (for production)
cargo build --target wasm32-unknown-unknown --no-default-features --release --lib

# Python bindings
cd python && uv sync --group dev && maturin develop
```

### Testing

```bash
# Run all tests
cargo test

# Run specific test file
cargo test --test integration_tests
cargo test --test gherkin_tests

# Run specific test
cargo test test_fractional_operator

# Python tests
cd python && pytest tests/ -v
```

### Code Quality

```bash
# Format code
cargo fmt

# Lint code (required before commit)
cargo clippy -- -D warnings
```

### Pull Request Title Conventions

This repository uses **squash and merge** for all PRs, which means the PR title becomes the commit message in the main branch. PR titles must follow the <a href="https://www.conventionalcommits.org/">Conventional Commits</a> format to enable automated changelog generation and semantic versioning via Release Please.

**Format:**
```
<type>(<optional-scope>): <description>
```

**Allowed Types:**
- `feat` - New feature (triggers minor version bump)
- `fix` - Bug fix (triggers patch version bump)
- `perf` - Performance improvement (triggers patch version bump)
- `docs` - Documentation changes
- `chore` - Maintenance tasks
- `refactor` - Code refactoring
- `test` - Test updates
- `ci` - CI/CD changes
- `build` - Build system changes
- `style` - Code style/formatting

**Examples:**
```
feat(operators): add string comparison operator
fix(wasm): correct memory allocation bug
docs: update API examples in README
chore(deps): update rust dependencies
feat(api)!: redesign evaluation API (breaking change)
```

**Automatic Validation:**

A GitHub Actions workflow (`.github/workflows/pr-title.yml`) automatically validates PR titles when opened, edited, or synchronized. Invalid titles will fail the check with a clear error message.

**Breaking Changes:**

Use `!` after the type/scope or include `BREAKING CHANGE:` in the PR body for breaking changes, which trigger a major version bump.

For more details, see the <a>PR template</a> and [Contributing Guide](../CONTRIBUTING.md).

## Testing Guidelines

### Test Suite Structure

The flagd-evaluator repository has a comprehensive test suite:

#### `tests/integration_tests.rs` - Comprehensive Integration Tests

Integration tests verify the complete evaluation flow including memory management, JSON parsing, custom operators, and error handling. These tests cover:

- **Basic JSON Logic Operations**: equality, comparison, boolean, conditional
- **Variable Access**: simple references, nested paths, missing variables, defaults
- **Array Operations**: `in`, `merge`
- **Arithmetic Operations**: `+`, `-`, `*`, `/`, `%`
- **Custom Fractional Operator**: bucketing, consistency, variable refs, distribution
- **Custom starts_with / ends_with Operators**: prefix/suffix matching, edge cases
- **Custom sem_ver Operator**: all comparison operators, pre-release, caret/tilde ranges
- **Memory Management**: `alloc`/`dealloc`, pointer packing
- **Error Handling**: invalid JSON, operator validation errors
- **State Management**: `update_state`, changed flags detection, metadata
- **Response Format Validation**: success/error JSON structure

#### `tests/gherkin_tests.rs` - Gherkin Specification Tests

BDD-style tests based on the official flagd specification scenarios (see [GHERKIN_TESTS.md](../tests/GHERKIN_TESTS.md)).

#### `tests/metadata_merging_tests.rs` - Metadata Merging Tests

Tests for flag-set metadata merging behaviour.

### When NOT to Run Tests

Tests are **resource-intensive** and should **NOT** be run during:

- **Initial exploration or code analysis**
- **Documentation review**
- **Issue triage or planning phases**
- **Answering questions about the codebase**

**Key principle**: If you're not changing code, don't run tests.

### When to Run Tests

Tests should **ONLY** be run when:

- **Explicitly requested by the user**
- **Implementing new features or bug fixes**
- **Validating changes before creating a PR**

### Running Tests Efficiently

```bash
# Run all tests (use sparingly)
cargo test

# Run specific test file (more efficient)
cargo test --test integration_tests
cargo test --test gherkin_tests

# Run specific test function (most efficient)
cargo test test_fractional_operator
cargo test test_sem_ver_operator_equal

# Run tests matching a pattern
cargo test fractional
cargo test starts_with
```

## Key Rules

**Memory safety (WASM exports):**
- Never panic — return JSON error responses
- Always validate UTF-8 via `string_from_memory()`
- All `unsafe` blocks require `// SAFETY:` comments
- Build WASM with `--no-default-features`

**Commits:**
- Follow [Conventional Commits](https://www.conventionalcommits.org/): `<type>(<scope>): <description>`
- Commit regularly after logical units of work
- See [CONTRIBUTING.md](../CONTRIBUTING.md) for full commit and PR guidelines

## Extension Instructions

### Updating This File

During agent sessions or development work, **important information should be added to this file** when:

- New architectural decisions are made
- Important patterns or conventions are discovered
- Integration details with other systems are learned
- Common pitfalls or gotchas are identified
- New custom operators are added
- Changes to the WASM API are made
- Performance optimizations are documented

Good additions to this file include:
- ✅ Architectural patterns and design decisions
- ✅ Integration patterns with host languages
- ✅ Performance characteristics and optimization tips
- ✅ Testing strategies and important test scenarios
- ✅ Common debugging techniques
- ✅ Links to relevant external documentation

Avoid including:
- ❌ Temporary notes or TODO lists
- ❌ Code that's already well-documented in source files
- ❌ Information that frequently changes (versions, URLs that change often)

## Important Considerations

### Chicory Compatibility

This evaluator is designed to work with <a href="https://github.com/nicknisi/chicory">Chicory</a>, a pure Java WebAssembly runtime that requires **no JNI** or native dependencies. To ensure compatibility:

- Avoid WASM features that require JavaScript bindings (`wasm-bindgen`)
- Don't use browser-specific APIs
- Keep the module self-contained with no external imports (except memory)
- Test with Chicory when making significant changes

### Optimization for Performance

The WASM binary is optimized for runtime performance:
- Uses `opt-level = 2` for speed
- Enables LTO (Link Time Optimization)
- Strips debug symbols in release
- Uses `panic = "abort"` to eliminate panic infrastructure

### Error Handling

All errors are returned as JSON, never as panics:
- Invalid JSON input → `{"success": false, "error": "..."}`
- Evaluation errors → `{"errorCode": "PARSE_ERROR", "errorMessage": "..."}`
- Flag not found → `{"reason": "ERROR", "errorCode": "FLAG_NOT_FOUND"}`

This ensures the WASM module never crashes the host application.

## External Specifications

- <a href="https://github.com/open-feature/flagd/blob/main/docs/reference/specifications/providers.md">flagd Provider Specification</a>
- <a href="https://flagd.dev/reference/specifications/custom-operations/">flagd Custom Operations</a>
- <a href="https://flagd.dev/reference/flag-definitions/">Flag Definitions Schema</a>
- <a href="https://jsonlogic.com/">JSON Logic</a>
- <a href="https://github.com/cozylogic/datalogic-rs">datalogic-rs</a>
- <a href="https://github.com/nicknisi/chicory">Chicory WASM Runtime</a>
- <a href="https://pyo3.rs/">PyO3 Rust-Python bindings</a>
