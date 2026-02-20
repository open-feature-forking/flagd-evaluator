# flagd-evaluator .NET

.NET wrapper for the flagd-evaluator WASM module. Provides high-performance feature flag evaluation using a pool of Wasmtime WASM instances with lock-free pre-evaluated caching and context key filtering.

## Requirements

- .NET 8.0 SDK
- Rust toolchain with `wasm32-unknown-unknown` target (for building WASM from source)

## Quick Start

```csharp
using FlagdEvaluator;

// Create evaluator (pool size defaults to Environment.ProcessorCount)
using var evaluator = new FlagEvaluator();

// Load flag configuration
var result = evaluator.UpdateState("""
{
    "flags": {
        "my-flag": {
            "state": "ENABLED",
            "defaultVariant": "on",
            "variants": { "on": true, "off": false },
            "targeting": {
                "if": [
                    { "==": [{ "var": "email" }, "admin@example.com"] },
                    "on", "off"
                ]
            }
        }
    }
}
""");

// Evaluate with context
var context = new Dictionary<string, object?> { ["email"] = "admin@example.com" };
var eval = evaluator.EvaluateFlag("my-flag", context);
// eval.Value = true, eval.Variant = "on", eval.Reason = "TARGETING_MATCH"

// Typed convenience methods (return default on error)
bool value = evaluator.EvaluateBool("my-flag", context, defaultValue: false);
```

## API Reference

### `FlagEvaluator`

| Method | Description |
|--------|-------------|
| `FlagEvaluator(FlagEvaluatorOptions?)` | Create evaluator with optional configuration |
| `UpdateState(string configJson)` | Update flag configuration across all instances |
| `EvaluateFlag(string flagKey, Dictionary<string, object?>?)` | Evaluate a flag, returns full `EvaluationResult` |
| `EvaluateBool(flagKey, context, defaultValue)` | Evaluate boolean flag with default fallback |
| `EvaluateString(flagKey, context, defaultValue)` | Evaluate string flag with default fallback |
| `EvaluateInt(flagKey, context, defaultValue)` | Evaluate integer flag with default fallback |
| `EvaluateDouble(flagKey, context, defaultValue)` | Evaluate double flag with default fallback |
| `PoolSize` | Number of WASM instances |
| `Dispose()` | Release all resources |

### `FlagEvaluatorOptions`

| Property | Default | Description |
|----------|---------|-------------|
| `PoolSize` | `Environment.ProcessorCount` | Number of WASM instances for parallel evaluation |
| `PermissiveValidation` | `false` | Accept invalid configs with warnings instead of rejecting |

### `EvaluationResult`

| Property | Type | Description |
|----------|------|-------------|
| `Value` | `JsonElement?` | The resolved flag value |
| `Variant` | `string` | The variant name |
| `Reason` | `string` | `STATIC`, `DEFAULT`, `TARGETING_MATCH`, `DISABLED`, `ERROR`, `FLAG_NOT_FOUND` |
| `ErrorCode` | `string?` | Error code if evaluation failed |
| `ErrorMessage` | `string?` | Error description |
| `FlagMetadata` | `Dictionary<string, JsonElement>?` | Flag metadata |
| `IsError` | `bool` | True if evaluation resulted in an error |

## Build & Test

```bash
cd dotnet

# Build WASM from Rust source
make wasm

# Build .NET solution
make build

# Run tests
make test

# Run benchmarks
make bench
```

## Architecture

- **WASM Instance Pool**: `BlockingCollection<WasmInstance>` sized to `ProcessorCount` for parallel evaluation
- **Pre-evaluated Cache**: Static/disabled flags served lock-free via `volatile` reference swap
- **Context Key Filtering**: Only serialize context keys referenced in targeting rules (32-34x speedup for large contexts)
- **Index-based Evaluation**: `evaluate_by_index(u32)` avoids flag key string serialization
- **Generation Guard**: Detects stale cache snapshots when `UpdateState` races with `EvaluateFlag`
- **Pre-allocated Buffers**: Each instance pre-allocates 256B (flag key) + 1MB (context) buffers in WASM memory

## Thread Safety

`FlagEvaluator` is fully thread-safe:
- `UpdateState()` serialized via lock; drains all pool instances, updates in parallel, swaps cache atomically
- `EvaluateFlag()` acquires a pool instance, evaluates, returns it — concurrent up to pool size
- Pre-evaluated flags bypass the pool entirely (lock-free volatile read)
