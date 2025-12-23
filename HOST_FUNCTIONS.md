# Host Functions

The flagd-evaluator WASM module requires the host environment to provide certain functions. These are imported by the WASM module and must be implemented by the runtime (e.g., Java/Chicory, JavaScript, Go, etc.).

## Required Host Function: `get_current_time_unix_seconds`

**Module:** `host`
**Function name:** `get_current_time_unix_seconds`
**Signature:** `() -> u64`

### Purpose

Provides the current Unix timestamp (seconds since epoch: 1970-01-01 00:00:00 UTC) to the WASM module. This is used for context enrichment to populate the `$flagd.timestamp` property, which can be used in targeting rules.

### Why a Host Function?

The WASM sandbox cannot access system time directly without WASI support. Since Chicory and other pure WASM runtimes don't provide WASI, the host must supply the current time.

### Return Value

- **Type:** `u64`
- **Value:** Unix timestamp in seconds since epoch
- **Example:** `1735689600` (represents 2025-01-01 00:00:00 UTC)

## Implementation Examples

### Java (Chicory)

```java
import com.dylibso.chicory.runtime.HostFunction;
import com.dylibso.chicory.wasm.types.Value;
import com.dylibso.chicory.wasm.types.ValueType;

// Create the host function
HostFunction getCurrentTime = new HostFunction(
    "host",                              // Module name
    "get_current_time_unix_seconds",    // Function name
    List.of(),                           // No parameters
    List.of(ValueType.I64),             // Returns i64
    (Instance instance, Value... args) -> {
        long currentTimeSeconds = System.currentTimeMillis() / 1000;
        return new Value[] { Value.i64(currentTimeSeconds) };
    }
);

// Add to module imports when loading WASM
Module module = Module.builder(wasmBytes)
    .withHostFunction(getCurrentTime)
    .build();
```

### Complete Java Example

```java
import com.dylibso.chicory.runtime.*;
import com.dylibso.chicory.wasm.types.*;
import java.nio.charset.StandardCharsets;

public class FlagdEvaluatorWithHostFunctions {
    public static void main(String[] args) {
        // Load WASM module
        byte[] wasmBytes = Files.readAllBytes(Path.of("flagd_evaluator.wasm"));

        // Define host function for current time
        HostFunction getCurrentTime = new HostFunction(
            "host",
            "get_current_time_unix_seconds",
            List.of(),
            List.of(ValueType.I64),
            (Instance instance, Value... unused) -> {
                long now = System.currentTimeMillis() / 1000;
                return new Value[] { Value.i64(now) };
            }
        );

        // Build module with host function
        Module module = Module.builder(wasmBytes)
            .withHostFunction(getCurrentTime)
            .build();

        Instance instance = module.instantiate();

        // Get WASM exports
        ExportFunction alloc = instance.export("alloc");
        ExportFunction dealloc = instance.export("dealloc");
        ExportFunction updateState = instance.export("update_state");
        ExportFunction evaluate = instance.export("evaluate");

        // Load flag configuration
        String config = """
        {
          "flags": {
            "time-based-flag": {
              "state": "ENABLED",
              "variants": {
                "on": true,
                "off": false
              },
              "defaultVariant": "off",
              "targeting": {
                "if": [
                  {">": [{"var": "$flagd.timestamp"}, 1700000000]},
                  "on",
                  "off"
                ]
              }
            }
          }
        }
        """;

        // Update state
        byte[] configBytes = config.getBytes(StandardCharsets.UTF_8);
        int configPtr = alloc.apply(Value.i32(configBytes.length))[0].asInt();
        instance.memory().write(configPtr, configBytes);

        long packedResult = updateState.apply(
            Value.i32(configPtr),
            Value.i32(configBytes.length)
        )[0].asLong();

        dealloc.apply(Value.i32(configPtr), Value.i32(configBytes.length));

        // Evaluate flag with context
        String flagKey = "time-based-flag";
        String context = "{\"email\":\"user@example.com\"}";

        byte[] flagKeyBytes = flagKey.getBytes(StandardCharsets.UTF_8);
        byte[] contextBytes = context.getBytes(StandardCharsets.UTF_8);

        int flagKeyPtr = alloc.apply(Value.i32(flagKeyBytes.length))[0].asInt();
        int contextPtr = alloc.apply(Value.i32(contextBytes.length))[0].asInt();

        instance.memory().write(flagKeyPtr, flagKeyBytes);
        instance.memory().write(contextPtr, contextBytes);

        long evalResult = evaluate.apply(
            Value.i32(flagKeyPtr),
            Value.i32(flagKeyBytes.length),
            Value.i32(contextPtr),
            Value.i32(contextBytes.length)
        )[0].asLong();

        // Unpack result pointer and length
        int resultPtr = (int) (evalResult >> 32);
        int resultLen = (int) (evalResult & 0xFFFFFFFF);

        byte[] resultBytes = new byte[resultLen];
        instance.memory().read(resultPtr, resultBytes);
        String result = new String(resultBytes, StandardCharsets.UTF_8);

        System.out.println("Evaluation result: " + result);

        // Clean up
        dealloc.apply(Value.i32(flagKeyPtr), Value.i32(flagKeyBytes.length));
        dealloc.apply(Value.i32(contextPtr), Value.i32(contextBytes.length));
        dealloc.apply(Value.i32(resultPtr), Value.i32(resultLen));
    }
}
```

### JavaScript (Node.js with WASI)

```javascript
const fs = require('fs');

// Load WASM
const wasmBytes = fs.readFileSync('flagd_evaluator.wasm');

// Define host functions
const importObject = {
    host: {
        get_current_time_unix_seconds: () => {
            return BigInt(Math.floor(Date.now() / 1000));
        }
    }
};

// Instantiate WASM with host functions
WebAssembly.instantiate(wasmBytes, importObject)
    .then(({ instance }) => {
        // Use the WASM exports
        const { alloc, dealloc, evaluate } = instance.exports;
        // ... rest of implementation
    });
```

### Go

```go
package main

import (
    "time"
    "github.com/tetratelabs/wazero"
    "github.com/tetratelabs/wazero/api"
)

func main() {
    ctx := context.Background()
    runtime := wazero.NewRuntime(ctx)
    defer runtime.Close(ctx)

    // Define host function
    hostModule := runtime.NewHostModuleBuilder("host")
    hostModule.NewFunctionBuilder().
        WithFunc(func() int64 {
            return time.Now().Unix()
        }).
        Export("get_current_time_unix_seconds")

    _, err := hostModule.Instantiate(ctx)
    if err != nil {
        panic(err)
    }

    // Load and instantiate WASM module
    wasmBytes, _ := os.ReadFile("flagd_evaluator.wasm")
    module, _ := runtime.Instantiate(ctx, wasmBytes)

    // Use the WASM exports
    // ...
}
```

## Behavior Without Host Function

If the host function is not provided:
- The WASM module will catch the panic and default `$flagd.timestamp` to `0`
- This allows targeting rules to detect unavailable time by checking for `timestamp == 0`
- Evaluation will continue without errors, but time-based targeting will not work correctly

## Testing the Host Function

You can verify the host function is working by:

1. Create a flag with time-based targeting:
```json
{
  "flags": {
    "test-flag": {
      "state": "ENABLED",
      "variants": {"on": true, "off": false},
      "defaultVariant": "off",
      "targeting": {
        "if": [
          {">": [{"var": "$flagd.timestamp"}, 0]},
          "on",
          "off"
        ]
      }
    }
  }
}
```

2. Evaluate the flag and check that `$flagd.timestamp` is non-zero in the context

3. The flag should resolve to `"on"` if the timestamp is provided correctly

## Future Host Functions

This document will be updated as additional host functions are added to the WASM module.
