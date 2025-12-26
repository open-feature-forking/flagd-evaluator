# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

flagd-evaluator is a **WebAssembly-based JSON Logic evaluator** designed for feature flag evaluation. It's written in Rust and compiled to WASM (~2.4MB optimized binary) to provide consistent evaluation logic across all OpenFeature flagd providers (Java, JavaScript, .NET, Go, Python, PHP, etc.).

**Key Purpose**: This is the shared evaluation engine used by all in-process flagd providers to ensure uniform behavior across polyglot architectures. See the [flagd providers specification](https://github.com/open-feature/flagd/blob/main/docs/reference/specifications/providers.md) for integration details.

## Essential Commands

### Development

```bash
# Build for native development/testing
cargo build

# Build optimized WASM (production)
cargo build --target wasm32-unknown-unknown --no-default-features --release --lib

# WASM output location
# target/wasm32-unknown-unknown/release/flagd_evaluator.wasm

```

### Testing

```bash
# Run all tests
cargo test

# Run specific test file
cargo test --test integration_tests
cargo test --test gherkin_tests

# Run specific test by name
cargo test test_fractional_operator

# Run with output visible
cargo test -- --nocapture
```

### Code Quality

```bash
# Format code (required before commit)
cargo fmt

# Check formatting without changing files
cargo fmt -- --check

# Lint code (must pass with no warnings)
cargo clippy -- -D warnings
```

## Architecture

### Core Design Principles

1. **WASM-First**: Compiled to WebAssembly for cross-language portability
2. **No External Dependencies**: Single WASM file, no JNI, no JavaScript bindings
3. **Chicory Compatible**: Works with pure Java WASM runtimes (no native code)
4. **Memory Safe**: Explicit alloc/dealloc, no panics, all errors returned as JSON
5. **Size Optimized**: Aggressive compilation flags (`opt-level = "z"`, LTO, panic = "abort")

### Module Organization

```
src/
├── lib.rs              # Main entry point, WASM exports (update_state, evaluate)
├── evaluation.rs       # Core flag evaluation logic, context enrichment ($flagd properties)
├── memory.rs           # WASM memory management (alloc/dealloc, pointer packing)
├── storage/            # Thread-local flag state storage
├── operators/          # Custom JSON Logic operators
│   ├── fractional.rs   # MurmurHash3-based consistent bucketing for A/B testing
│   ├── sem_ver.rs      # Semantic version comparison (=, !=, <, <=, >, >=, ^, ~)
│   ├── starts_with.rs  # String prefix matching
│   └── ends_with.rs    # String suffix matching
├── model/              # Flag configuration data structures
└── validation.rs       # JSON Schema validation against flagd schemas
```

### Key Architectural Concepts

**WASM Exports** (lib.rs:138-625):
- `evaluate_logic(rule_ptr, rule_len, data_ptr, data_len) -> u64` - Direct JSON Logic evaluation
- `update_state(config_ptr, config_len) -> u64` - Store flag configuration, returns changed flags
- `evaluate(flag_key_ptr, flag_key_len, context_ptr, context_len) -> u64` - Evaluate stored flag
- `alloc(len) -> *mut u8` - Allocate WASM memory
- `dealloc(ptr, len)` - Free WASM memory
- `set_validation_mode(mode) -> u64` - Set strict (0) or permissive (1) validation

All functions return **packed u64**: upper 32 bits = pointer, lower 32 bits = length.

**Memory Model**: Caller allocates input buffers, callee allocates result buffers. Caller must free all allocations. UTF-8 JSON strings for all inputs/outputs.

**Context Enrichment** (evaluation.rs): The evaluator automatically injects standard `$flagd` properties into evaluation context:
- `$flagd.flagKey` - The flag being evaluated
- `$flagd.timestamp` - Unix timestamp (seconds) at evaluation time
- `targetingKey` - Defaults to empty string if not provided in context

**Custom Operators**: All registered via `datalogic_rs::Operator` trait (operators/mod.rs). See [flagd custom operations spec](https://flagd.dev/reference/specifications/custom-operations/) for full details.

**Validation**: Uses `jsonschema` crate to validate flag configs against [flagd-schemas](https://github.com/open-feature/flagd-schemas). Two modes:
- Strict (default): Reject invalid configs
- Permissive: Accept with warnings (for legacy compatibility)

**Flag State Management** (storage/mod.rs): Thread-local storage for flag configurations. `update_state` detects and reports changed flags (added, removed, or mutated).

## Important Implementation Details

### Building for WASM

**Critical Build Flags** (Cargo.toml:46-51):
```toml
[profile.release]
opt-level = "z"      # Optimize for size
lto = true           # Link-time optimization
codegen-units = 1    # Single codegen unit for better optimization
strip = true         # Strip symbols
panic = "abort"      # Remove panic unwinding infrastructure
```

**No Default Features for WASM**: Always build with `--no-default-features` to exclude unnecessary dependencies from WASM binary.

### Memory Safety Rules

1. **Never panic in WASM exports**: All errors must be returned as JSON error responses
2. **Always validate UTF-8**: Use `string_from_memory()` which returns Result
3. **Pointer lifetime**: WASM memory is stable within a single function call but may be reallocated between calls
4. **Safety comments required**: All `unsafe` blocks must have `// SAFETY:` comments explaining why they're safe

### Error Handling Patterns

**JSON Logic Evaluation** (lib.rs:173-175):
```rust
match logic.evaluate_json(&rule_str, &data_str) {
    Ok(result) => EvaluationResponse::success(result),
    Err(e) => EvaluationResponse::error(format!("{}", e)),
}
```

**Flag Evaluation** (evaluation.rs): Returns `EvaluationResult` with standardized error codes:
- `FLAG_NOT_FOUND` - Flag key not in configuration
- `PARSE_ERROR` - JSON parsing or rule evaluation error
- `TYPE_MISMATCH` - Resolved value doesn't match expected type
- `GENERAL` - Other errors

Resolution reasons: `STATIC`, `DEFAULT`, `TARGETING_MATCH`, `DISABLED`, `ERROR`, `FLAG_NOT_FOUND`

### Testing Philosophy

**Integration Tests** (tests/integration_tests.rs): Comprehensive tests covering:
- Basic JSON Logic operations
- All custom operators (fractional, sem_ver, starts_with, ends_with)
- Memory management
- Edge cases and error handling
- State management and flag evaluation
- Type-specific evaluation functions
- Context enrichment ($flagd properties)

**Gherkin Tests** (tests/gherkin_tests.rs): Specification compliance tests using the official flagd testbed.

**When to Run Tests**:
- ✅ After making code changes that affect behavior
- ✅ Before creating a PR
- ✅ When explicitly requested by user
- ❌ During initial exploration or code reading
- ❌ When just browsing documentation

### Common Workflows

**Adding a New Custom Operator**:
1. Create new file in `src/operators/` (e.g., `my_operator.rs`)
2. Implement `datalogic_rs::Operator` trait
3. Register in `src/operators/mod.rs` via `create_evaluator()`
4. Add tests in both unit tests and `tests/integration_tests.rs`
5. Document in README.md under "Custom Operators" section

**Modifying Flag Evaluation Logic**:
1. Primary logic is in `src/evaluation.rs`
2. Context enrichment happens in `evaluate_flag()` function
3. State retrieval uses thread-local storage via `get_flag_state()`
4. Always maintain backward compatibility with flagd provider specification
5. Test with targeting rules, disabled flags, and missing flags

**Memory Management Changes**:
1. All WASM-facing functions must use packed u64 returns
2. Use `string_to_memory()` to allocate and pack results
3. Use `string_from_memory()` to read inputs (handles UTF-8 validation)
4. Document caller responsibilities in function doc comments
5. Test with the Java example in `examples/java/`

## Git Workflow & Commit Practices

**Make Regular Commits**: Commit your changes frequently with clear, descriptive messages. Don't wait until the end of a large feature to commit.

**Follow Conventional Commits Format**: Use the same format for regular commits as required for PR titles:

```
<type>(<scope>): <description>

[optional body]

[optional footer]
```

**Examples**:
```bash
# Feature commits
git commit -m "feat(operators): add new string matching operator"
git commit -m "feat(evaluation): support nested context properties"

# Bug fix commits
git commit -m "fix(memory): correct pointer alignment in alloc function"
git commit -m "fix(fractional): handle empty bucket key correctly"

# Documentation commits
git commit -m "docs: update API examples in README"
git commit -m "docs(operators): clarify sem_ver caret range behavior"

# Refactoring commits
git commit -m "refactor(storage): simplify flag state management"

# Test commits
git commit -m "test(fractional): add edge case for zero-weight buckets"

# Chore commits
git commit -m "chore(deps): update datalogic-rs to 4.1"
git commit -m "chore: remove unused chrono shim"
```

**Commit Message Guidelines**:
- Use imperative mood ("add feature" not "added feature")
- Keep subject line under 72 characters
- Add body for complex changes explaining why, not what
- Reference issues when relevant: `Closes #123`

**When to Commit**:
- ✅ After completing a logical unit of work
- ✅ Before switching to a different task
- ✅ After fixing a bug
- ✅ After adding tests
- ✅ Before taking a break or ending work session
- ❌ Don't commit broken code (unless marked with WIP)
- ❌ Don't commit commented-out code or debug statements

## Release Process

This project uses [Release Please](https://github.com/googleapis/release-please) for automated releases:

- **PR Titles Must Follow Conventional Commits**: The PR title becomes the commit message (squash merge)
- Format: `<type>(<scope>): <description>`
- Types that trigger releases:
  - `feat:` - Minor version bump (new feature)
  - `fix:` - Patch version bump (bug fix)
  - `perf:` - Patch version bump (performance improvement)
  - `feat!:` or `BREAKING CHANGE:` - Major version bump
- Types for changelog only (no release): `docs:`, `chore:`, `test:`, `ci:`, `refactor:`, `style:`, `build:`

**PR Title Validation**: GitHub workflow (`.github/workflows/pr-title.yml`) automatically validates format.

## Dependencies

**Core Production**:
- `datalogic-rs` (4.0) - JSON Logic implementation
- `serde`, `serde_json` - JSON serialization (no_std compatible with alloc)
- `boon` (0.6) - JSON Schema validation
- `murmurhash3` - Hash function for fractional operator
- `ahash` - Hash table implementation (SIMD-disabled for Chicory compatibility)
- `getrandom` - Random number generation for WASM

**Dev**:
- `cucumber` - Gherkin/BDD testing
- `tokio` - Async runtime for tests

## Cross-Language Integration

This WASM module is embedded in multiple language providers. Key integration patterns:

**Java (Chicory)**:
```java
// Load WASM, get exports for alloc/dealloc/evaluate_logic
// Allocate memory for inputs, write UTF-8 JSON
// Call function, unpack u64 result
// Read result from memory, deallocate all memory
```

See `examples/java/FlagdEvaluatorExample.java` for complete working example.

**General Pattern**:
1. Load WASM module
2. Get function exports (alloc, dealloc, evaluate_logic, update_state, evaluate)
3. For each call:
   - Allocate memory for inputs using `alloc()`
   - Write UTF-8 encoded JSON strings to WASM memory
   - Call evaluation function with pointers and lengths
   - Unpack returned u64 (ptr = upper 32 bits, len = lower 32 bits)
   - Read result JSON from WASM memory
   - Free all allocations using `dealloc()`

**Memory Lifecycle**: Host application owns all memory allocation/deallocation decisions. WASM module only allocates result memory internally.

## Python Native Bindings

In addition to WASM integration, this project provides **native Python bindings** using PyO3 for better performance and developer experience.

### Structure

```
python/
├── src/
│   └── lib.rs           # PyO3 bindings (FlagEvaluator class)
├── tests/               # Python test suite (pytest)
├── examples/            # Usage examples
├── benchmarks/          # Performance benchmarks
├── Cargo.toml           # PyO3 dependencies
├── pyproject.toml       # Maturin build config
└── README.md            # Python-specific documentation
```

### Building Python Bindings

**Recommended: Using uv (faster)**

```bash
# Install uv
curl -LsSf https://astral.sh/uv/install.sh | sh

# Set up development environment (installs deps and creates venv)
cd python
uv sync --group dev
source .venv/bin/activate

# Build and install locally
maturin develop

# Run tests
pytest tests/ -v

# Build wheels for distribution
maturin build --release
```

**Alternative: Using pip**

```bash
# Install maturin
pip install maturin

# Build and install locally
cd python
maturin develop

# Run tests
pytest tests/ -v

# Build wheels for distribution
maturin build --release
```

### Key Differences from WASM

**API Design**: Pythonic dictionaries instead of JSON strings:
```python
# PyO3 API (native)
evaluator = FlagEvaluator()
evaluator.update_state({"flags": {"myFlag": {...}}})
result = evaluator.evaluate("myFlag", {})

# vs WASM API (for comparison)
config_json = json.dumps({"flags": {"myFlag": {...}}})
update_state_wasm(config_json)
result_json = evaluate_wasm("myFlag", "{}")
result = json.loads(result_json)
```

**State Management**: Python class with internal state instead of thread-local storage:
```python
evaluator = FlagEvaluator()  # Instance-based state
evaluator.update_state(config)
result = evaluator.evaluate_bool("myFlag", {}, False)
```

**Error Handling**: Native Python exceptions instead of JSON error responses:
```python
try:
    result = evaluator.evaluate_bool("nonexistent", {}, False)
except KeyError as e:
    print(f"Flag not found: {e}")
```

### Performance

Native bindings provide **5-10x better performance** than WASM:
- No WASM instantiation overhead
- Direct memory sharing (no serialization)
- Native Python exceptions (no JSON parsing)

### CI/CD

Python wheels are built automatically for:
- **Linux**: x86_64, aarch64 (manylinux)
- **macOS**: x86_64, aarch64 (Apple Silicon)
- **Windows**: x64

See `.github/workflows/python-wheels.yml` for the build configuration.

## Related Documentation

- **Flagd Provider Specification**: https://github.com/open-feature/flagd/blob/main/docs/reference/specifications/providers.md
- **In-Process Resolver**: https://github.com/open-feature/flagd/blob/main/docs/reference/specifications/providers.md#in-process-resolver
- **Custom Operations Spec**: https://flagd.dev/reference/specifications/custom-operations/
- **Flag Definitions Schema**: https://flagd.dev/reference/flag-definitions/
- **JSON Logic**: https://jsonlogic.com/
- **datalogic-rs**: https://github.com/cozylogic/datalogic-rs
- **Chicory WASM Runtime**: https://github.com/nicknisi/chicory

## GitHub Copilot Instructions

The `.github/copilot-instructions.md` file contains extensive context about this project including:
- Architecture and purpose
- WASM function exports and memory management
- Testing guidelines (when to run tests vs when to just read them)
- Custom operators implementation details
- Pull request conventions
- Integration patterns with host languages

Refer to that file for additional context when working on this repository.
