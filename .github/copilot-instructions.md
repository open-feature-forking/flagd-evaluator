# flagd-evaluator Copilot Instructions

This document provides comprehensive context about the flagd-evaluator repository for GitHub Copilot and developers.

## Overview of the Repository

This repository contains the **core evaluation logic for flagd**, a feature flag management system. The evaluator is designed as a **WebAssembly (WASM) module** that provides consistent feature flag evaluation across multiple language implementations.

The evaluator is used across all OpenFeature flagd providers (Java, JavaScript, .NET, Go, Python, PHP, etc.) to ensure uniform evaluation behavior regardless of the programming language being used. For detailed information about how providers use this evaluator, see the [providers.md documentation](https://github.com/open-feature/flagd/blob/main/docs/reference/specifications/providers.md) in the flagd docs.

## Architecture & Purpose

### Built as a WASM Module

The flagd-evaluator is:
- **Written in Rust** and compiled to WebAssembly for maximum portability
- **Language-agnostic** - can be embedded in any language with a WASM runtime
- **Single binary deployment** - approximately 1.5MB WASM file with no external dependencies
- **Optimized for size and performance** using aggressive compilation flags

### In-Process Evaluation

This evaluator implements the **in-process evaluation logic** described in the [In-Process Resolver section](https://github.com/open-feature/flagd/blob/main/docs/reference/specifications/providers.md#in-process-resolver) of the flagd providers specification. It allows feature flag evaluation to happen directly within the application process without requiring network calls to a separate flagd server.

Key characteristics:
- Evaluates feature flags locally using stored flag configurations
- Processes targeting rules using JsonLogic with custom operators
- Maintains flag state in memory for fast evaluation
- Returns standardized evaluation results with variant, reason, and error information

### Core Functionality

1. **JSON Logic Evaluation** - Full support for [JSON Logic](https://jsonlogic.com/) operations via [datalogic-rs](https://github.com/cozylogic/datalogic-rs)
2. **Custom Operators** - Feature-flag specific operators for:
   - `fractional` - Consistent bucketing for A/B testing and gradual rollouts
   - `sem_ver` - Semantic version comparison (=, !=, <, <=, >, >=, ^, ~)
   - `starts_with` - String prefix matching
   - `ends_with` - String suffix matching
3. **Flag State Management** - Internal storage for flag configurations with `update_state` API
4. **Memory Safe Operations** - Clean memory management with explicit alloc/dealloc functions

## Key Documentation References

### Primary Specifications

- **[flagd Providers Specification](https://github.com/open-feature/flagd/blob/main/docs/reference/specifications/providers.md)** - Describes how providers should integrate with flagd
  - **[In-Process Resolver](https://github.com/open-feature/flagd/blob/main/docs/reference/specifications/providers.md#in-process-resolver)** - Details on how this evaluator is used
  - Evaluation results format (value, variant, reason, error codes)
  - Flag configuration schema

- **[flagd Custom Operations Specification](https://flagd.dev/reference/specifications/custom-operations/)** - Complete documentation of custom operators
  - Fractional operator for A/B testing
  - Semantic version comparison
  - String comparison operators

- **[flagd Flag Definitions](https://flagd.dev/reference/flag-definitions/)** - Schema for flag configurations
  - Flag state (ENABLED/DISABLED)
  - Variants and default variant
  - Targeting rules using JsonLogic

### Related Technologies

- **[JSON Logic](https://jsonlogic.com/)** - The rule evaluation engine
- **[datalogic-rs](https://github.com/cozylogic/datalogic-rs)** - Rust implementation of JSON Logic
- **[Chicory](https://github.com/nicknisi/chicory)** - Pure Java WebAssembly runtime (no JNI required)

## Relationship to flagd Ecosystem

### Part of the OpenFeature/flagd Project

This repository is a critical component of the larger [OpenFeature](https://openfeature.dev/) and [flagd](https://flagd.dev/) ecosystem:

- **OpenFeature** - An open standard for feature flag management
- **flagd** - A feature flag daemon that implements the OpenFeature specification
- **flagd-evaluator** (this repository) - The shared evaluation engine used by all in-process providers

### Used by Multiple Language Providers

Language-specific providers embed this WASM module to evaluate feature flags:

- **Java Provider** - Uses Chicory (pure Java WASM runtime)
- **JavaScript/TypeScript Provider** - Uses Node.js or browser WASM runtimes
- **.NET Provider** - Uses Wasmtime or other .NET-compatible WASM runtimes
- **Go Provider** - Uses wazero or other Go WASM runtimes
- **Python Provider** - Uses wasmer-python or other Python WASM runtimes
- **PHP Provider** - Uses wasm extension or FFI bindings
- **And more...** - Any language with WASM support can use this evaluator

### Consistent Evaluation Across All Providers

The primary benefit of using a shared WASM evaluator is **consistency**:

- Same targeting logic across all language implementations
- Identical fractional bucketing results regardless of language
- Synchronized custom operator behavior
- Uniform error handling and response formats
- Single source of truth for evaluation logic

This eliminates the need to reimplement complex evaluation logic in each language and ensures that feature flags behave identically across polyglot architectures.

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
- `starts_with.rs` - String prefix matching
- `ends_with.rs` - String suffix matching

## Project Structure

```
flagd-evaluator/
├── src/
│   ├── lib.rs              # Main library entry point
│   ├── evaluation.rs       # Core evaluation logic
│   ├── memory.rs           # WASM memory management (alloc/dealloc)
│   ├── error.rs            # Error types and handling
│   ├── storage/            # Flag state storage
│   ├── operators/          # Custom operator implementations
│   │   ├── fractional.rs   # Fractional bucketing
│   │   ├── sem_ver.rs      # Semantic version comparison
│   │   ├── starts_with.rs  # String prefix matching
│   │   └── ends_with.rs    # String suffix matching
│   ├── model/              # Data models
│   │   └── feature_flag.rs # Flag configuration models
│   └── bin/
│       └── flagd-eval.rs   # CLI tool for testing
├── tests/                  # Integration tests
├── examples/               # Usage examples (Java, rules, etc.)
├── Cargo.toml             # Rust dependencies and build config
└── README.md              # Comprehensive usage documentation
```

## Development Workflow

### Building

```bash
# Native build (for development/testing)
cargo build

# WASM build (for production)
cargo build --target wasm32-unknown-unknown --release
```

### Testing

```bash
# Run all tests
cargo test

# Run specific test
cargo test test_fractional_operator
```

### Code Quality

```bash
# Format code
cargo fmt

# Lint code
cargo clippy -- -D warnings
```

### CLI Tool

A CLI tool (`flagd-eval`) is available for testing rules without WASM:

```bash
# Evaluate a rule
cargo run --bin flagd-eval -- eval --rule '{"==": [1, 1]}' --data '{}'

# Run test suite
cargo run --bin flagd-eval -- test examples/rules/test-suite.json

# List available operators
cargo run --bin flagd-eval -- operators
```

## Testing Guidelines

### Test Suite Structure

The flagd-evaluator repository has a comprehensive test suite organized into two main test files:

#### `tests/cli_tests.rs` - CLI Integration Tests

CLI integration tests verify that the `flagd-eval` binary works correctly end-to-end. These tests cover:

- **Help and Version Commands** - Verifying command-line interface displays correct information
- **Eval Command** - Testing JSON Logic evaluation functionality:
  - Inline JSON rules and data
  - File-based inputs (rules and data from files using `@` prefix)
  - Comparison operators with variable references
  - Custom fractional operator with basic usage
  - Pretty-printed output formatting
  - Invalid JSON error handling
  - Missing file error handling
  - Variable resolution failures
- **Test Command** - Running test suites:
  - Test suite execution from JSON files
  - Verbose output mode showing rule, data, and expected values
  - Missing file error handling
  - Invalid test format error handling
- **Operators Command** - Documentation:
  - Listing all available operators
  - Showing operator syntax examples
- **Edge Cases**:
  - Empty data objects
  - Complex nested rules with multiple conditions
  - Unicode string handling in both rules and data

#### `tests/integration_tests.rs` - Comprehensive Integration Tests

Integration tests verify the complete evaluation flow including memory management, JSON parsing, custom operators, and error handling. These tests include **70+ test cases** covering:

- **Basic JSON Logic Operations**:
  - Equality (`==`) and strict equality (`===`)
  - Comparison operators (`>`, `<`, `>=`, `<=`)
  - Boolean operations (`and`, `or`, `!`)
  - Conditional logic (`if-then-else` with nested conditions)

- **Variable Access**:
  - Simple variable references (`{"var": "name"}`)
  - Nested path access (`{"var": "user.profile.name"}`)
  - Missing variable handling (returns null)
  - Default values for missing variables

- **Array Operations**:
  - `in` operator for membership testing
  - `merge` operator for array concatenation

- **Arithmetic Operations**:
  - Addition (`+`), subtraction (`-`), multiplication (`*`)
  - Division (`/`), modulo (`%`)

- **Custom Fractional Operator** (A/B testing and gradual rollouts):
  - Basic bucketing with percentage distributions
  - Consistency verification (same key always returns same bucket)
  - Variable references for bucket keys
  - Nested variable path resolution
  - Single bucket edge case (100% allocation)
  - Numeric key handling
  - Distribution verification over 1000 iterations
  - Missing buckets error handling
  - Empty buckets error handling
  - Missing variable error handling

- **Custom starts_with Operator** (string prefix matching):
  - Basic prefix matching with variable references
  - Literal string matching
  - Empty prefix handling (always true)
  - Case-sensitive comparison verification
  - False case testing

- **Custom ends_with Operator** (string suffix matching):
  - Basic suffix matching with variable references
  - Literal string matching
  - Empty suffix handling (always true)
  - Case-sensitive comparison verification
  - False case testing

- **Custom sem_ver Operator** (semantic version comparison):
  - Equality (`=`) and inequality (`!=`) comparisons
  - Less than (`<`) and less than or equal (`<=`)
  - Greater than (`>`) and greater than or equal (`>=`)
  - Caret range (`^`) - compatible with major version
  - Tilde range (`~`) - compatible with minor version
  - Pre-release version handling
  - Literal version comparisons (without variables)
  - Invalid version error handling
  - Missing version parts (treated as 0)
  - Complex targeting rules combining sem_ver with variable paths

- **Memory Management**:
  - `alloc` and `dealloc` functions
  - Zero-byte allocation handling
  - Multiple allocations and deallocations
  - Pack and unpack pointer/length operations

- **Error Handling**:
  - Invalid JSON in rules
  - Invalid JSON in data
  - Fractional operator validation errors
  - Semantic version parsing errors

- **Edge Cases**:
  - Empty rules and data
  - Null value comparisons
  - Unicode string handling
  - Large number arithmetic
  - Deeply nested data structures (4+ levels)
  - Complex nested rules with multiple conditions

- **Response Format Validation**:
  - Success response structure (`success: true`, `result`)
  - Error response structure (`success: false`, `error`)
  - JSON serialization of responses

- **State Management**:
  - `update_state` with flag configurations
  - Invalid JSON error handling
  - Missing 'flags' field error handling
  - State replacement behavior
  - Flags with targeting rules (JsonLogic conditions)
  - Metadata fields (`$schema`, `$evaluators`)
  - Empty flags object handling
  - Multiple flags storage
  - Invalid flag structure error handling

### When NOT to Run Tests

Tests are **resource-intensive** and should **NOT** be run during:

- **Initial exploration or code analysis** - Understanding repository structure, reading code to learn architecture
- **Repository structure understanding** - Browsing directories, viewing file organization
- **Documentation review** - Reading README, contributing guides, or other documentation
- **Issue triage or discussion** - Understanding requirements, asking clarifying questions
- **Understanding existing implementations** - Reading through source code to learn how features work
- **Reading through code to learn architecture** - Studying design patterns, code organization, and implementation details
- **Answering questions about the codebase** - Providing information about how the code works
- **Planning phases** - Creating implementation plans, discussing approaches

**Key principle**: If you're not changing code, don't run tests. Understanding test coverage through documentation is sufficient for exploration.

### When to Run Tests

Tests should **ONLY** be run when:

- **Explicitly requested by the user** - User specifically asks to run tests
- **Implementing new features** - Adding new operators, functionality, or WASM exports
- **Making bug fixes** - Fixing identified issues that affect behavior
- **Making code changes that could affect behavior** - Modifying evaluation logic, operators, or core functionality
- **Debugging specific test failures** - Investigating why a particular test is failing
- **Validating changes before creating a PR** - Final verification that all changes work correctly
- **Verifying custom operator implementations** - After adding or modifying fractional, sem_ver, starts_with, ends_with operators
- **Testing WASM build** - After making changes that affect WASM compilation or exports

**Key principle**: Only run tests when you need to verify that code changes work correctly.

### Running Tests Efficiently

When you do need to run tests:

```bash
# Run all tests (use sparingly - takes significant time)
cargo test

# Run specific test file (more efficient)
cargo test --test integration_tests
cargo test --test cli_tests

# Run specific test function (most efficient)
cargo test test_fractional_operator
cargo test test_sem_ver_operator_equal

# Run tests matching a pattern
cargo test fractional
cargo test starts_with
```

### Performance Considerations

- **Test execution is time-consuming** - The full test suite includes 70+ integration tests and 20+ CLI tests (90+ total)
- **Build time** - Compiling the project and tests takes time
- **The test suite is comprehensive** - Tests cover JSON Logic, custom operators, memory management, error handling, and edge cases
- **Focus on understanding first** - Read the test files to understand coverage without executing them
- **Run tests intentionally and purposefully** - Only execute when validating actual code changes
- **Avoid redundant test runs** - Don't re-run tests if nothing has changed since the last execution

### Test-Driven Development

When making changes:

1. **First**: Understand existing test coverage by reading test files
2. **Then**: Make your code changes
3. **Finally**: Run relevant tests to validate changes
4. **Avoid**: Running tests before understanding what needs to be tested

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

### How to Update

1. Edit `.github/copilot-instructions.md`
2. Add new sections or expand existing ones with relevant context
3. Keep the structure consistent and well-organized
4. Use clear, concise language
5. Include code examples where helpful
6. Link to relevant documentation or specifications
7. Commit changes with descriptive messages

### What to Include

Good additions to this file include:
- ✅ Architectural patterns and design decisions
- ✅ Integration patterns with host languages
- ✅ Performance characteristics and optimization tips
- ✅ Testing strategies and important test scenarios
- ✅ Common debugging techniques
- ✅ Links to relevant external documentation
- ✅ Explanations of complex algorithms (e.g., fractional bucketing)

Avoid including:
- ❌ Temporary notes or TODO lists
- ❌ Code that's already well-documented in source files
- ❌ Overly detailed implementation specifics
- ❌ Information that frequently changes (versions, URLs that change often)

## Important Considerations

### Chicory Compatibility

This evaluator is designed to work with [Chicory](https://github.com/nicknisi/chicory), a pure Java WebAssembly runtime that requires **no JNI** or native dependencies. To ensure compatibility:

- Avoid WASM features that require JavaScript bindings (`wasm-bindgen`)
- Don't use browser-specific APIs
- Keep the module self-contained with no external imports (except memory)
- Test with Chicory when making significant changes

### Optimization for Size

The WASM binary is aggressively optimized for size:
- Uses `opt-level = "z"` for size optimization
- Enables LTO (Link Time Optimization)
- Strips debug symbols
- Uses `panic = "abort"` to eliminate panic infrastructure
- Patches chrono to remove wasm-bindgen dependencies

Current size: ~1.5MB (includes full JSON Logic implementation with 50+ operators)

### Error Handling

All errors are returned as JSON, never as panics:
- Invalid JSON input → `{"success": false, "error": "..."}`
- Evaluation errors → `{"errorCode": "PARSE_ERROR", "errorMessage": "..."}`
- Flag not found → `{"reason": "ERROR", "errorCode": "FLAG_NOT_FOUND"}`

This ensures the WASM module never crashes the host application.

## Resources

- [Repository README](../README.md) - Comprehensive usage guide
- [Contributing Guide](../CONTRIBUTING.md) - Development guidelines
- [OpenFeature Documentation](https://openfeature.dev/)
- [flagd Documentation](https://flagd.dev/)
- [JSON Logic](https://jsonlogic.com/)
