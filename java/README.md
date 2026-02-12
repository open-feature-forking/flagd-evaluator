# flagd-evaluator-java

Java library for [flagd-evaluator](https://github.com/open-feature/flagd-evaluator) with bundled WASM runtime.

## Overview

This library provides a standalone Java artifact that bundles the flagd-evaluator WASM module and Chicory runtime, making it easy to evaluate feature flags in Java applications without manual WASM management.

## Features

- ✅ **OpenFeature SDK Integration** - Built on official OpenFeature SDK types
- ✅ **Type-safe API** - Generic evaluation methods with compile-time type checking
- ✅ **Bundled WASM module** - No need to manually copy WASM files
- ✅ **Thread-safe** - Safe for concurrent use
- ✅ **JIT compiled** - Uses Chicory's JIT compiler for performance
- ✅ **Full feature support** - All flagd evaluation features including targeting rules
- ✅ **Performance benchmarks** - JMH benchmarks for tracking performance over time
- ✅ **Context key filtering** - Only serializes context fields referenced by targeting rules
- ✅ **Index-based evaluation** - Numeric flag indices avoid string key overhead across WASM boundary

## Installation

Add the dependency to your `pom.xml`:

```xml
<dependency>
    <groupId>dev.openfeature</groupId>
    <artifactId>flagd-evaluator-java</artifactId>
    <version>0.1.0-SNAPSHOT</version>
</dependency>
```

This library includes:
- **OpenFeature SDK** (1.19.2) - Provides core types and context management
- **Chicory WASM Runtime** (1.6.1) - Pure Java WebAssembly runtime with JIT compilation
- **Jackson** (2.18.2) - JSON serialization with custom OpenFeature serializers
- **flagd-evaluator WASM module** - Bundled in the JAR
- **JMH Benchmarks** (1.37) - Performance benchmarking suite (test scope)

## Usage

### Basic Example

```java
import dev.openfeature.flagd.evaluator.FlagEvaluator;
import dev.openfeature.flagd.evaluator.EvaluationResult;
import dev.openfeature.flagd.evaluator.UpdateStateResult;

// Create evaluator
FlagEvaluator evaluator = new FlagEvaluator();

// Load flag configuration
String config = """
    {
      "flags": {
        "my-flag": {
          "state": "ENABLED",
          "defaultVariant": "on",
          "variants": {
            "on": true,
            "off": false
          }
        }
      }
    }
    """;

UpdateStateResult updateResult = evaluator.updateState(config);
System.out.println("Flags changed: " + updateResult.getChangedFlags());

// Evaluate boolean flag
EvaluationResult<Boolean> result = evaluator.evaluateFlag(Boolean.class, "my-flag", "{}");
System.out.println("Value: " + result.getValue());
System.out.println("Variant: " + result.getVariant());
System.out.println("Reason: " + result.getReason());
```

### Type-Safe Evaluation

The library supports type-safe flag evaluation for all OpenFeature types:

```java
import dev.openfeature.flagd.evaluator.EvaluationResult;

// Boolean flags
EvaluationResult<Boolean> boolResult = evaluator.evaluateFlag(Boolean.class, "feature-enabled", "{}");
boolean isEnabled = boolResult.getValue();

// String flags
EvaluationResult<String> stringResult = evaluator.evaluateFlag(String.class, "color-scheme", "{}");
String color = stringResult.getValue();

// Integer flags
EvaluationResult<Integer> intResult = evaluator.evaluateFlag(Integer.class, "max-items", "{}");
int maxItems = intResult.getValue();

// Double flags
EvaluationResult<Double> doubleResult = evaluator.evaluateFlag(Double.class, "threshold", "{}");
double threshold = doubleResult.getValue();
```

### With Targeting Context

```java
import java.util.Map;
import dev.openfeature.flagd.evaluator.EvaluationResult;
import com.fasterxml.jackson.databind.ObjectMapper;

Map<String, Object> context = Map.of(
    "targetingKey", "user-123",
    "email", "user@example.com",
    "age", 25
);

String contextJson = new ObjectMapper().writeValueAsString(context);
EvaluationResult<Boolean> result = evaluator.evaluateFlag(Boolean.class, "premium-feature", contextJson);
```

### Targeting Rules

```java
import com.fasterxml.jackson.databind.ObjectMapper;

String config = """
    {
      "flags": {
        "premium-feature": {
          "state": "ENABLED",
          "defaultVariant": "standard",
          "variants": {
            "standard": false,
            "premium": true
          },
          "targeting": {
            "if": [
              {
                "==": [
                  { "var": "email" },
                  "premium@example.com"
                ]
              },
              "premium",
              null
            ]
          }
        }
      }
    }
    """;

evaluator.updateState(config);

ObjectMapper mapper = new ObjectMapper();

// Premium user
Map<String, Object> premiumContext = Map.of("email", "premium@example.com");
String premiumContextJson = mapper.writeValueAsString(premiumContext);
EvaluationResult<Boolean> result = evaluator.evaluateFlag(Boolean.class, "premium-feature", premiumContextJson);
// result.getValue() == true

// Regular user
Map<String, Object> regularContext = Map.of("email", "regular@example.com");
String regularContextJson = mapper.writeValueAsString(regularContext);
result = evaluator.evaluateFlag(Boolean.class, "premium-feature", regularContextJson);
// result.getValue() == false
```

### Validation Modes

```java
// Strict mode (default) - rejects invalid configurations
FlagEvaluator strictEvaluator = new FlagEvaluator();

// Permissive mode - accepts invalid configurations with warnings
FlagEvaluator permissiveEvaluator = new FlagEvaluator(
    FlagEvaluator.ValidationMode.PERMISSIVE
);
```

## API Reference

### FlagEvaluator

Main class for flag evaluation.

#### Constructors

- `FlagEvaluator()` - Creates evaluator with strict validation
- `FlagEvaluator(ValidationMode mode)` - Creates evaluator with specified validation mode

#### Methods

- `UpdateStateResult updateState(String jsonConfig)` - Updates flag configuration. Returns changed flags, pre-evaluated results, required context keys per flag, and flag indices.
- `<T> EvaluationResult<T> evaluateFlag(Class<T> type, String flagKey, EvaluationContext context)` - Type-safe flag evaluation with OpenFeature context (recommended). Automatically applies context key filtering and index-based evaluation when available.
- `<T> EvaluationResult<T> evaluateFlag(Class<T> type, String flagKey, String contextJson)` - Type-safe flag evaluation with pre-serialized JSON context
- `<T> EvaluationResult<T> evaluateFlag(Class<T> type, String flagKey, Map<String, Object> context)` - Type-safe flag evaluation with Map context

**Supported Types:**
- `Boolean.class` - For boolean flags
- `String.class` - For string flags
- `Integer.class` - For integer flags
- `Double.class` - For double/number flags
- `Value.class` - For structured/object flags

### EvaluationResult<T>

Generic result class containing the outcome of flag evaluation.

#### Properties

- `T getValue()` - The resolved value (type-safe based on generic parameter)
- `String getVariant()` - The selected variant name
- `String getReason()` - Resolution reason (STATIC, TARGETING_MATCH, DEFAULT, DISABLED, ERROR, FLAG_NOT_FOUND)
- `boolean isError()` - Whether the evaluation encountered an error
- `String getErrorCode()` - Error code if evaluation failed
- `String getErrorMessage()` - Error message if evaluation failed
- `ImmutableMetadata getMetadata()` - Flag metadata

### UpdateStateResult

Contains the result of updating flag state.

#### Properties

- `boolean isSuccess()` - Whether the update succeeded
- `String getError()` - Error message if update failed
- `List<String> getChangedFlags()` - List of changed flag keys
- `Map<String, EvaluationResult<Object>> getPreEvaluated()` - Pre-evaluated results for static/disabled flags (cached on Java side)
- `Map<String, List<String>> getRequiredContextKeys()` - Per-flag context keys needed by targeting rules (for context filtering)
- `Map<String, Integer> getFlagIndices()` - Flag key to numeric index mapping (for `evaluate_by_index`)

## Building from Source

### Prerequisites

- JDK 11+
- Rust toolchain with `wasm32-unknown-unknown` target

**Note**: Maven is not required - the project includes the Maven wrapper (`mvnw`).

### Build

```bash
# Build the WASM module and Java library
cd java
./mvnw clean install
```

The build process:
1. Compiles the Rust WASM module (from parent directory)
2. Copies the WASM file to Java resources
3. Compiles Java code
4. Runs tests
5. Generates Javadoc
6. Packages JAR with bundled WASM

## How It Works

This library bundles:

1. **WASM Module**: The flagd-evaluator compiled to WebAssembly
2. **Chicory Runtime**: Pure Java WASM runtime with JIT compilation
3. **OpenFeature SDK**: Official OpenFeature SDK for type-safe flag evaluation
4. **Host Functions**: 9 required host functions for WASM interop
5. **Jackson Serialization**: Custom serializers for OpenFeature types
6. **Java API**: Type-safe wrapper around WASM exports

At runtime:
- WASM module is loaded from classpath during class initialization
- Chicory JIT compiles the WASM to optimized bytecode
- Custom Jackson serializers handle OpenFeature SDK types (`ImmutableMetadata`, `LayeredEvaluationContext`)
- Each `FlagEvaluator` instance creates its own WASM instance
- `updateState()` populates three caches: pre-evaluated results, required context keys per flag, and flag index mappings
- `evaluateFlag()` checks the pre-evaluated cache first, then applies context key filtering and index-based WASM evaluation for targeting flags
- Type-safe evaluation returns `EvaluationResult<T>` with compile-time type checking
- All evaluation and state update operations are synchronized for thread safety

## Performance

- **Startup**: WASM module compiled once during class loading (~100ms)
- **Memory**: ~3MB for WASM module + Chicory runtime
- **Static flags**: ~0.02 µs via pre-evaluation cache (no WASM call)
- **Targeting flags**: ~12.8 µs with context key filtering (1000+ attribute context)

### Optimization Pipeline

The evaluator applies three optimizations automatically during `evaluateFlag()`:

1. **Pre-evaluation cache**: Static flags (no targeting rules) and disabled flags are pre-evaluated during `updateState()` and cached on the Java side. `evaluateFlag()` returns instantly without crossing the WASM boundary.

2. **Context key filtering**: During `updateState()`, the WASM module walks each flag's compiled targeting tree to extract which context fields the rule references (e.g., `{"var": "email"}` -> `email`). When evaluating with an `EvaluationContext`, only those fields are serialized — a 1000-attribute context where the rule uses 2 fields shrinks from ~50KB JSON to ~200 bytes.

3. **Index-based evaluation**: Each flag is assigned a stable numeric index during `updateState()`. The WASM `evaluate_by_index(u32, ...)` export avoids flag key string serialization and uses O(1) Vec lookup instead of HashMap lookup on the Rust side.

Context enrichment (`$flagd.flagKey`, `$flagd.timestamp`, `targetingKey`) is also moved to the Java side, eliminating an allocation + clone inside the WASM module.

### WASM Evaluator vs Native JsonLogic

JMH benchmark (`ResolverComparisonBenchmark`) comparing this WASM-based evaluator against a native Java JsonLogic implementation (`json-logic-java`) with a `LayeredEvaluationContext` containing 1000+ attributes:

| Scenario | Native JsonLogic | WASM Evaluator | Speedup |
|---|---|---|---|
| **Simple flag** (no targeting) | 0.023 µs/op | 0.020 µs/op | ~same (both cached) |
| **Targeting match** (1000+ attrs) | 409.3 µs/op | 12.8 µs/op | **32x faster** |
| **Targeting no-match** (small ctx) | 4.4 µs/op | 12.0 µs/op | 0.4x |
| **Many evals** (x1000, 1000+ attrs) | 408.5 µs/op | 11.9 µs/op | **34x faster** |

Key observations:
- **Simple/static flags** are served from the Java-side cache at ~0.02 µs — no WASM call at all.
- **Targeting flags with large contexts** benefit most from context key filtering. The old JsonLogic resolver must iterate all 1000+ attributes on every evaluation, while the WASM evaluator only serializes the 2-3 fields the rule actually uses.
- **Small contexts** (targeting no-match row) show the WASM overhead more clearly — the 12 µs includes the WASM boundary crossing cost. For small contexts, the native resolver is faster since there's little serialization to save.

### Running Benchmarks

```bash
# Build the JMH fat JAR
cd java
./mvnw clean package

# Run the old-vs-new comparison benchmark
java -jar target/benchmarks.jar ResolverComparisonBenchmark

# Run the evaluator benchmarks (layered context, simple context, serialization)
java -jar target/benchmarks.jar FlagEvaluatorJmhBenchmark
```

The JUnit-based benchmark test suite is also available:
```bash
./mvnw test -Dtest=FlagEvaluatorBenchmarkTest
```

## Thread Safety

`FlagEvaluator` is thread-safe and can be shared across threads. All evaluation and state update operations are synchronized.

## Future Improvements

- **Async API**: Non-blocking evaluation methods
- **Streaming Updates**: Support for flag configuration streams

## Related Projects

- [flagd-evaluator](https://github.com/open-feature/flagd-evaluator) - Rust-based WASM evaluator
- [OpenFeature Java SDK](https://github.com/open-feature/java-sdk) - Official OpenFeature SDK for Java
- [Chicory](https://github.com/dylibso/chicory) - Pure Java WASM runtime
- [OpenFeature](https://openfeature.dev) - Vendor-agnostic feature flagging

## License

Apache License 2.0
