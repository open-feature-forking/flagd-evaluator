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

- `UpdateStateResult updateState(String jsonConfig)` - Updates flag configuration
- `<T> EvaluationResult<T> evaluateFlag(Class<T> type, String flagKey, String contextJson)` - Type-safe flag evaluation with JSON context
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
- Type-safe evaluation returns `EvaluationResult<T>` with compile-time type checking
- Evaluations are synchronized for thread safety

## Performance

- **Startup**: WASM module compiled once during class loading (~100ms)
- **Evaluation**: ~13,000 ops/s with realistic layered contexts (JIT compiled)
- **Memory**: ~3MB for WASM module + Chicory runtime
- **Object Churn**: ~12.8 KB allocated per layered evaluation, ~6.9 KB per simple evaluation

### Benchmarks

The library includes JMH (Java Microbenchmark Harness) benchmarks for performance tracking:

```bash
# Run JMH benchmarks
./mvnw exec:java@run-jmh-benchmark
```

**Benchmark Results** (example from development machine):
```
Benchmark                                              Mode  Cnt       Score        Error  Units
FlagEvaluatorJmhBenchmark.evaluateWithLayeredContext  thrpt    5   13035.383 ±   4173.375  ops/s
FlagEvaluatorJmhBenchmark.evaluateWithSimpleContext   thrpt    5   14748.099 ±   2689.011  ops/s
FlagEvaluatorJmhBenchmark.serializeLayeredContext     thrpt    5  222863.374 ± 151002.720  ops/s
```

**Object Churn (GC allocation rates)**:
```
Benchmark                                              Alloc Rate     Alloc/Op
evaluateWithLayeredContext                             159.6 MB/sec   12.8 KB/op
evaluateWithSimpleContext                              97.8 MB/sec    6.9 KB/op
```

**Benchmark Scenarios:**
- **evaluateWithLayeredContext**: Full flag evaluation with 4-layer context (API, Transaction, Client, Invocation) and 100+ entries per layer
- **evaluateWithSimpleContext**: Baseline evaluation with minimal context
- **serializeLayeredContext**: JSON serialization overhead measurement

To run with GC profiling:
```bash
./mvnw exec:java -Dexec.classpathScope=test -Dexec.mainClass=org.openjdk.jmh.Main \
  -Dexec.args="FlagEvaluatorJmhBenchmark -prof gc -f 0"
```

The JUnit-based benchmark test suite is also available:
```bash
# Run performance benchmark tests
./mvnw test -Dtest=FlagEvaluatorBenchmarkTest
```

## Thread Safety

`FlagEvaluator` is thread-safe and can be shared across threads. All evaluation and state update operations are synchronized.

## Future Improvements

- **AOT Compilation**: When Chicory supports AOT, compile WASM → Java at build time for better performance
- **Async API**: Non-blocking evaluation methods
- **Streaming Updates**: Support for flag configuration streams

## Related Projects

- [flagd-evaluator](https://github.com/open-feature/flagd-evaluator) - Rust-based WASM evaluator
- [OpenFeature Java SDK](https://github.com/open-feature/java-sdk) - Official OpenFeature SDK for Java
- [Chicory](https://github.com/dylibso/chicory) - Pure Java WASM runtime
- [OpenFeature](https://openfeature.dev) - Vendor-agnostic feature flagging

## License

Apache License 2.0
