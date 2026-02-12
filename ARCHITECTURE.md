# Architecture

flagd-evaluator is a Rust-based feature flag evaluation engine designed to replace per-language JSON Logic implementations with a single core — one implementation to maintain, one test suite, consistent behavior across all languages. Thin wrapper libraries keep language-specific code minimal. It ships as:

- **WASM module** (~2.4MB) — for Java (Chicory), Go (wazero), JavaScript, .NET, and other WASM runtimes
- **Native bindings** — Python (PyO3), with more planned
- **Rust library** — direct API for Rust consumers

The best integration strategy is chosen per language based on benchmarks. For example, Python benchmarks showed PyO3 native bindings outperform WASM via wasmtime-py, while Go (wazero) and Java (Chicory) perform well with embedded WASM. Each wrapper must meet or exceed the performance of the language-native approach it replaces. Cross-language [benchmarks](BENCHMARKS.md) validate this.

## Design Principles

1. **WASM-First** — Compiled to WebAssembly for cross-language portability
2. **No External Dependencies** — Single WASM file, no JNI, no JavaScript bindings
3. **Chicory Compatible** — Works with pure Java WASM runtimes (no native code)
4. **Memory Safe** — Explicit alloc/dealloc, no panics, all errors returned as JSON
5. **Size Optimized** — Aggressive compilation flags (`opt-level = "z"`, LTO, `panic = "abort"`)

## Module Organization

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

## WASM Exports

All WASM export functions return a **packed u64**: upper 32 bits = pointer, lower 32 bits = length.

| Export | Signature | Description |
|--------|-----------|-------------|
| `evaluate_logic` | `(rule_ptr, rule_len, data_ptr, data_len) -> u64` | Direct JSON Logic evaluation |
| `update_state` | `(config_ptr, config_len) -> u64` | Store flag configuration, returns changed flags |
| `evaluate` | `(flag_key_ptr, flag_key_len, context_ptr, context_len) -> u64` | Evaluate a stored flag |
| `alloc` | `(len) -> *mut u8` | Allocate WASM memory |
| `dealloc` | `(ptr, len)` | Free WASM memory |
| `set_validation_mode` | `(mode) -> u64` | Set strict (0) or permissive (1) validation |

## Memory Model

Caller allocates input buffers, callee allocates result buffers. Caller must free all allocations. UTF-8 JSON strings for all inputs/outputs.

### Memory Safety Rules

1. **Never panic in WASM exports** — All errors must be returned as JSON error responses
2. **Always validate UTF-8** — Use `string_from_memory()` which returns `Result`
3. **Pointer lifetime** — WASM memory is stable within a single function call but may be reallocated between calls
4. **Safety comments required** — All `unsafe` blocks must have `// SAFETY:` comments

### WASM Build Flags

From `Cargo.toml` release profile:

```toml
[profile.release]
opt-level = "z"      # Optimize for size
lto = true           # Link-time optimization
codegen-units = 1    # Single codegen unit for better optimization
strip = true         # Strip symbols
panic = "abort"      # Remove panic unwinding infrastructure
```

Always build WASM with `--no-default-features` to exclude unnecessary dependencies.

## Context Enrichment

The evaluator automatically injects standard `$flagd` properties into the evaluation context (see [flagd provider spec](https://flagd.dev/reference/specifications/providers/#in-process-resolver)):

| Property | Description |
|----------|-------------|
| `$flagd.flagKey` | The flag being evaluated |
| `$flagd.timestamp` | Unix timestamp (seconds) at evaluation time |
| `targetingKey` | Defaults to empty string if not provided |

## Custom Operators

All registered via `datalogic_rs::Operator` trait in `src/operators/mod.rs`. See the [flagd custom operations spec](https://flagd.dev/reference/specifications/custom-operations/) for full details.

## Validation

Uses the `boon` crate to validate flag configs against [flagd-schemas](https://github.com/open-feature/flagd-schemas):
- **Strict** (default): Reject invalid configs
- **Permissive**: Accept with warnings (for legacy compatibility)

## Flag State Management

Thread-local storage for flag configurations (`src/storage/mod.rs`). `update_state` detects and reports changed flags (added, removed, or mutated).

## Error Handling

**JSON Logic evaluation** (lib.rs):
```rust
match logic.evaluate_json(&rule_str, &data_str) {
    Ok(result) => EvaluationResponse::success(result),
    Err(e) => EvaluationResponse::error(format!("{}", e)),
}
```

**Flag evaluation** returns `EvaluationResult` with standardized error codes:

| Error Code | Meaning |
|------------|---------|
| `FLAG_NOT_FOUND` | Flag key not in configuration |
| `PARSE_ERROR` | JSON parsing or rule evaluation error |
| `TYPE_MISMATCH` | Resolved value doesn't match expected type |
| `GENERAL` | Other errors |

Resolution reasons: `STATIC`, `DEFAULT`, `TARGETING_MATCH`, `DISABLED`, `ERROR`, `FLAG_NOT_FOUND`

## Common Workflows

### Adding a New Custom Operator

1. Create new file in `src/operators/` (e.g., `my_operator.rs`)
2. Implement `datalogic_rs::Operator` trait
3. Register in `src/operators/mod.rs` via `create_evaluator()`
4. Add tests in both unit tests and `tests/integration_tests.rs`
5. Document in README.md under "Custom Operators"

### Modifying Flag Evaluation Logic

1. Primary logic is in `src/evaluation.rs`
2. Context enrichment happens in `evaluate_flag()` function
3. State retrieval uses thread-local storage via `get_flag_state()`
4. Always maintain backward compatibility with the flagd provider specification
5. Test with targeting rules, disabled flags, and missing flags

### Memory Management Changes

1. All WASM-facing functions must use packed u64 returns
2. Use `string_to_memory()` to allocate and pack results
3. Use `string_from_memory()` to read inputs (handles UTF-8 validation)
4. Document caller responsibilities in function doc comments
5. Test with the Java example in `examples/java/`

## Cross-Language Integration

This WASM module is embedded in multiple language providers. The general integration pattern:

1. Load WASM module
2. Get function exports (`alloc`, `dealloc`, `evaluate_logic`, `update_state`, `evaluate`)
3. For each call:
   - Allocate memory for inputs using `alloc()`
   - Write UTF-8 encoded JSON strings to WASM memory
   - Call evaluation function with pointers and lengths
   - Unpack returned u64 (`ptr = upper 32 bits`, `len = lower 32 bits`)
   - Read result JSON from WASM memory
   - Free all allocations using `dealloc()`

**Memory lifecycle**: Host application owns all memory allocation/deallocation decisions. WASM module only allocates result memory internally.

See `examples/java/FlagdEvaluatorExample.java` for a complete Java (Chicory) integration example. See `python/README.md` for native Python bindings via PyO3.

## Dependencies

### Production

| Crate | Version | Purpose |
|-------|---------|---------|
| `datalogic-rs` | 4.0 | JSON Logic implementation |
| `serde`, `serde_json` | — | JSON serialization (no_std compatible with alloc) |
| `boon` | 0.6 | JSON Schema validation |
| `murmurhash3` | — | Hash function for fractional operator |
| `ahash` | — | Hash table implementation (SIMD-disabled for Chicory) |
| `getrandom` | — | Random number generation for WASM |

### Dev

| Crate | Purpose |
|-------|---------|
| `cucumber` | Gherkin/BDD testing |
| `tokio` | Async runtime for tests |
