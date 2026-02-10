# Benchmark Standard

This document defines the standardized benchmark matrix for flagd-evaluator across all language implementations (Rust, Java, Python). All benchmarks should follow this matrix to enable direct cross-language performance comparison.

## Evaluation Scenarios

Every language implementation should benchmark the following scenarios. The combination of **targeting complexity** and **context size** isolates where time is spent (serialization vs rule evaluation).

### Core Evaluation Matrix

| ID | Scenario | Targeting | Context Size | What it measures |
|----|----------|-----------|--------------|------------------|
| E1 | Simple flag, empty context | None (STATIC) | 0 attrs | Baseline: flag lookup + result serialization |
| E2 | Simple flag, small context | None (STATIC) | 5 attrs | Serialization overhead for typical call |
| E3 | Simple flag, large context | None (STATIC) | 100+ attrs | Serialization cost dominance |
| E4 | Simple targeting, small context | Single `==` condition | 5 attrs | Minimal rule evaluation cost |
| E5 | Simple targeting, large context | Single `==` condition | 100+ attrs | Serialization + simple rule |
| E6 | Complex targeting, small context | Nested `and`/`or`, 3+ conditions | 5 attrs | Rule evaluation cost dominance |
| E7 | Complex targeting, large context | Nested `and`/`or`, 3+ conditions | 100+ attrs | Worst case: heavy serialization + complex rules |
| E8 | Targeting match | Rule that matches | 5 attrs | Match code path |
| E9 | Targeting no-match | Rule that doesn't match (default) | 5 attrs | Default/fallback code path |
| E10 | Disabled flag | `state: DISABLED` | 0 attrs | Early exit performance |
| E11 | Missing flag | Non-existent key | 0 attrs | Error path performance |

### Custom Operator Benchmarks

| ID | Scenario | What it measures |
|----|----------|------------------|
| O1 | Fractional (2 buckets) | Typical A/B test bucketing |
| O2 | Fractional (8 buckets) | Multi-variant experiment |
| O3 | Semver equality (`=`) | Version string parsing + comparison |
| O4 | Semver range (`^`, `~`) | Range matching logic |
| O5 | `starts_with` | String prefix matching |
| O6 | `ends_with` | String suffix matching |

### State Management Benchmarks

| ID | Scenario | What it measures |
|----|----------|------------------|
| S1 | Update state (5 flags) | Small config parse + validate |
| S2 | Update state (50 flags) | Medium config scaling |
| S3 | Update state (200 flags) | Large config scaling |
| S4 | Update state (no change) | Change detection overhead |
| S5 | Update state (1 flag changed in 100) | Incremental update efficiency |

### Concurrency Benchmarks

| ID | Scenario | Threads | What it measures |
|----|----------|---------|------------------|
| C1 | Simple flag, single thread | 1 | Baseline (no contention) |
| C2 | Simple flag, 4 threads | 4 | Standard concurrent load |
| C3 | Simple flag, 8 threads | 8 | High contention |
| C4 | Targeting flag, 4 threads | 4 | Concurrent rule evaluation |
| C5 | Mixed workload, 4 threads | 4 | Realistic production mix |
| C6 | Read/write contention | 4 | `evaluate` concurrent with `update_state` |

### Comparison Benchmarks (language-specific)

| ID | Scenario | What it measures |
|----|----------|------------------|
| X1 | Old resolver vs new evaluator (simple) | Baseline improvement |
| X2 | Old resolver vs new evaluator (targeting) | Rule evaluation improvement |
| X3 | Old vs new under concurrency (4 threads) | Thread scaling improvement |

**Java**: Old = `json-logic-java` via `MinimalInProcessResolver`; New = WASM via Chicory
**Python**: Old = `json-logic-utils` (pure Python); New = PyO3 native bindings
**Rust**: N/A (Rust *is* the engine; compare `datalogic-rs` direct vs through evaluator)

## Context Definitions

To ensure comparability, use these standard context shapes:

### Empty Context
```json
{}
```

### Small Context (5 attributes)
```json
{
  "targetingKey": "user-123",
  "tier": "premium",
  "role": "admin",
  "region": "us-east",
  "score": 85
}
```

### Large Context (100+ attributes)
```json
{
  "targetingKey": "user-123",
  "tier": "premium",
  "role": "admin",
  "region": "us-east",
  "score": 85,
  "attr_0": "value-0",
  "attr_1": 42,
  "attr_2": true,
  ...
  "attr_99": "value-99"
}
```

Use deterministic generation (seeded random) so results are reproducible.

## Flag Definitions

### Simple Boolean Flag (no targeting)
```json
{
  "state": "ENABLED",
  "defaultVariant": "on",
  "variants": { "on": true, "off": false }
}
```

### Simple Targeting Flag
```json
{
  "state": "ENABLED",
  "defaultVariant": "off",
  "variants": { "on": true, "off": false },
  "targeting": {
    "if": [{ "==": [{ "var": "tier" }, "premium"] }, "on", "off"]
  }
}
```

### Complex Targeting Flag
```json
{
  "state": "ENABLED",
  "defaultVariant": "basic",
  "variants": { "premium": "premium-tier", "standard": "standard-tier", "basic": "basic-tier" },
  "targeting": {
    "if": [
      { "and": [
        { "==": [{ "var": "tier" }, "premium"] },
        { ">": [{ "var": "score" }, 90] }
      ]},
      "premium",
      { "if": [
        { "or": [
          { "==": [{ "var": "tier" }, "standard"] },
          { ">": [{ "var": "score" }, 50] }
        ]},
        "standard",
        "basic"
      ]}
    ]
  }
}
```

## Running Benchmarks

### Rust
```bash
cargo bench                          # all suites
cargo bench --bench evaluation       # evaluation only
cargo bench -- --quick               # quick run
# HTML reports: target/criterion/
```

### Java
```bash
cd java
./mvnw clean package
java -jar target/benchmarks.jar                              # all benchmarks
java -jar target/benchmarks.jar ConcurrentFlagEvaluatorBenchmark  # concurrent only
java -jar target/benchmarks.jar -prof gc                     # with GC profiling
```

### Python
```bash
cd python
uv sync --group dev && maturin develop
pytest benchmarks/ --benchmark-only -v               # all benchmarks
pytest benchmarks/ --benchmark-only --benchmark-json=results.json  # export
```

## Reporting Results

When reporting benchmark results, always include:

1. **Hardware**: CPU model, core count, RAM
2. **OS**: Distribution and kernel version
3. **Runtime versions**: `rustc --version`, `java --version`, `python --version`
4. **Metrics per scenario**:
   - Throughput (ops/sec)
   - Latency (mean, p50, p99)
   - Allocation rate (if available)
5. **Comparison table** when measuring old vs new

Results should be committed to language-specific README files, not to this document.
