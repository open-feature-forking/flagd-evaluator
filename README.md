# flagd-evaluator

A single Rust-based feature flag evaluation engine that replaces per-language JSON Logic implementations with one core — one implementation, one test suite, consistent behavior everywhere.

[![CI](https://github.com/open-feature-forking/flagd-evaluator/actions/workflows/ci.yml/badge.svg)](https://github.com/open-feature-forking/flagd-evaluator/actions/workflows/ci.yml)
[![License](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](https://opensource.org/licenses/Apache-2.0)

## Language Packages

Thin wrapper libraries expose the evaluator via WASM runtimes or native bindings. Each package bundles the evaluator and handles all memory management internally.

| Language | Runtime | Install | Docs |
|----------|---------|---------|------|
| **Java** | Chicory (pure JVM) | `dev.openfeature:flagd-evaluator-java` | [java/README.md](java/README.md) |
| **Go** | wazero (pure Go) | `go get github.com/open-feature/flagd-evaluator/go` | [go/README.md](go/README.md) |
| **.NET** | Wasmtime | NuGet (coming soon) | [dotnet/README.md](dotnet/README.md) |
| **Python** | PyO3 (native) | `pip install flagd-evaluator` | [python/README.md](python/README.md) |
| **JavaScript** | Node.js WASM | npm (coming soon) | [js/](js/) |
| **Rust** | Native library | `flagd-evaluator` crate | [crates.io](https://crates.io/crates/flagd-evaluator) |

## Quick Start

Every wrapper follows the same pattern: create evaluator, load config, evaluate flags.

### Java

```java
FlagEvaluator evaluator = new FlagEvaluator();
evaluator.updateState(configJson);

EvaluationResult<Boolean> result = evaluator.evaluateFlag(Boolean.class, "my-flag", context);
boolean value = result.getValue();
```

### Go

```go
e, _ := evaluator.NewFlagEvaluator()
defer e.Close()

e.UpdateState(configJSON)
val := e.EvaluateBool("my-flag", ctx, false)
```

### .NET

```csharp
using var evaluator = new FlagEvaluator();
evaluator.UpdateState(configJson);

bool value = evaluator.EvaluateBool("my-flag", context, defaultValue: false);
```

### Python

```python
from flagd_evaluator import FlagEvaluator

evaluator = FlagEvaluator()
evaluator.update_state(config)
value = evaluator.evaluate_bool("my-flag", {}, False)
```

### JavaScript

```typescript
const evaluator = await FlagEvaluator.create("flagd_evaluator.wasm");
evaluator.updateState(configJson);

const result = evaluator.evaluateFlag("my-flag", context);
```

### Rust

```rust
use flagd_evaluator::{FlagEvaluator, ValidationMode};

let mut evaluator = FlagEvaluator::new(ValidationMode::Strict);
evaluator.update_state(&config).unwrap();

let result = evaluator.evaluate_bool("my-flag", &context);
```

All wrappers accept a [flagd flag definition](https://flagd.dev/reference/flag-definitions/) config:

```json
{
  "flags": {
    "my-flag": {
      "state": "ENABLED",
      "defaultVariant": "on",
      "variants": { "on": true, "off": false },
      "targeting": {
        "if": [{ "==": [{ "var": "email" }, "admin@example.com"] }, "on", "off"]
      }
    }
  }
}
```

## How It Works

The Rust core compiles to a ~2.4MB WASM module (or native bindings for Python). Each language wrapper loads the module once and reuses it for all evaluations.

### Evaluation Flow

```
updateState(config)                          evaluateFlag(key, context)
       |                                              |
       v                                              v
  Parse & validate config                   Check pre-evaluated cache
  Detect changed flags             ------>  (static/disabled flags: ~0.02 us)
  Pre-evaluate static flags                         |
  Extract required context keys              Filter context keys
  Assign flag indices                        Serialize only needed fields
       |                                     Call WASM evaluate_by_index
       v                                              |
  Return: changedFlags,                               v
    preEvaluated,                            Return: value, variant, reason
    requiredContextKeys,
    flagIndices
```

### Host-Side Optimizations

The `updateState` response includes metadata that wrappers use automatically:

1. **Pre-evaluated cache** — Static flags (no targeting) and disabled flags are fully resolved at config-load time and cached on the host side. Evaluation returns instantly without crossing the WASM boundary (~0.02 us).

2. **Context key filtering** — The WASM module walks each flag's targeting rule tree to extract which context fields it references (e.g., `{"var": "email"}` -> `email`). When evaluating, only those fields are serialized instead of the entire context. A 1000-attribute context where the rule uses 2 fields shrinks from ~50KB to ~200 bytes.

3. **Index-based evaluation** — Each flag gets a stable numeric index during `updateState`. The WASM `evaluate_by_index(u32, ...)` export avoids flag key string serialization and uses O(1) Vec lookup on the Rust side.

With a 1000+ attribute context, these optimizations deliver a **32-34x speedup** over native JSON Logic implementations. See [BENCHMARKS.md](BENCHMARKS.md) for the full comparison matrix.

## Custom Operators

All [flagd custom operators](https://flagd.dev/reference/specifications/custom-operations/) are implemented:

### fractional

Consistent hashing for A/B testing. Same key always maps to the same bucket.

```json
{"fractional": [{"var": "targetingKey"}, ["control", 50, "treatment", 50]]}
```

### sem_ver

Semantic version comparison with all standard operators plus caret (`^`) and tilde (`~`) ranges.

```json
{"sem_ver": [{"var": "app.version"}, ">=", "2.0.0"]}
```

### starts_with / ends_with

Case-sensitive string prefix and suffix matching.

```json
{"starts_with": [{"var": "email"}, "admin@"]}
{"ends_with": [{"var": "filename"}, ".pdf"]}
```

## Building from Source

```bash
# Dev build + tests
cargo build
cargo test

# WASM build
cargo build --target wasm32-unknown-unknown --no-default-features --release --lib

# Lint (required before commit)
cargo fmt && cargo clippy -- -D warnings
```

The WASM file is output to `target/wasm32-unknown-unknown/release/flagd_evaluator.wasm`.

## Documentation

| Topic | File |
|-------|------|
| Architecture, memory model, WASM API | [ARCHITECTURE.md](ARCHITECTURE.md) |
| Benchmarks, performance matrix | [BENCHMARKS.md](BENCHMARKS.md) |
| Host function requirements | [HOST_FUNCTIONS.md](HOST_FUNCTIONS.md) |
| Contributing guidelines | [CONTRIBUTING.md](CONTRIBUTING.md) |
| Java package | [java/README.md](java/README.md) |
| Go package | [go/README.md](go/README.md) |
| .NET package | [dotnet/README.md](dotnet/README.md) |
| Python bindings | [python/README.md](python/README.md) |

**External specs:**
- [flagd Flag Definitions](https://flagd.dev/reference/flag-definitions/)
- [flagd Custom Operations](https://flagd.dev/reference/specifications/custom-operations/)
- [flagd Provider Specification](https://github.com/open-feature/flagd/blob/main/docs/reference/specifications/providers.md)
- [JSON Logic](https://jsonlogic.com/) | [datalogic-rs](https://github.com/cozylogic/datalogic-rs)

## License

Apache License, Version 2.0 — see [LICENSE](LICENSE).

## Acknowledgments

- [datalogic-rs](https://github.com/cozylogic/datalogic-rs) — JSON Logic engine
- [Chicory](https://github.com/dylibso/chicory) — Pure Java WASM runtime
- [wazero](https://wazero.io/) — Pure Go WASM runtime
- [PyO3](https://pyo3.rs/) — Rust-Python bindings
- [OpenFeature](https://openfeature.dev/) — Open standard for feature flag management
