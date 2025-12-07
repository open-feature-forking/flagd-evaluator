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
