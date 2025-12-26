# Chicory WASM "Unreachable Instruction" Issue - RESOLVED

## Problem

When evaluating flags with context in Chicory (Java WASM runtime), the WASM module was crashing with:
```
com.dylibso.chicory.runtime.TrapException: Trapped on unreachable instruction
```

This only occurred when providing evaluation context like:
```json
{
  "sources": "flags/allFlags.json,rawflags/selector-flags.json",
  "email": "ballmer@macrosoft.com",
  "injectedmetadata": "set"
}
```

## Root Cause

The panic was caused by `std::time::SystemTime::now()` in the context enrichment function (evaluation.rs:231).

In WASM without WASI support (like Chicory), `SystemTime::now()` **panics** instead of returning an error. Since this project uses `panic = "abort"` (Cargo.toml:55), the panic translates to the WASM `unreachable` instruction, causing Chicory to throw a TrapException.

The code flow was:
1. Call `evaluate()` with context
2. Enter `enrich_context()` to add `$flagd.timestamp`
3. Call `SystemTime::now()` → **PANIC**
4. Panic → `unreachable` → Chicory TrapException

## Solution

Instead of relying on WASM's unavailable system time, we now use a **host function** that the WASM runtime must provide.

### Changes Made

1. **Added host function import** (lib.rs:44-58):
   ```rust
   #[cfg(target_family = "wasm")]
   #[link(wasm_import_module = "host")]
   extern "C" {
       fn host_get_current_time() -> u64;
   }
   ```

2. **Created wrapper function** (lib.rs:84-99):
   - In WASM: Calls host function with panic catch (defaults to 0)
   - In native code: Uses `SystemTime::now()` for tests/CLI

3. **Updated context enrichment** (evaluation.rs:228-230):
   ```rust
   let timestamp = crate::get_current_time();
   ```

### WASM Import Declaration

Verified with wasm-objdump:
```
Import[9]:
 - func[0] <host.get_current_time_unix_seconds> <- host.get_current_time_unix_seconds
```

## How to Fix Your Java Code

You must provide the host function when loading the WASM module:

```java
import com.dylibso.chicory.runtime.HostFunction;
import com.dylibso.chicory.wasm.types.Value;
import com.dylibso.chicory.wasm.types.ValueType;
import java.util.List;

// Define the host function
HostFunction getCurrentTime = new HostFunction(
    "host",                              // Module name (must match WASM import)
    "get_current_time_unix_seconds",    // Function name (must match WASM import)
    List.of(),                           // No parameters
    List.of(ValueType.I64),             // Returns i64
    (Instance instance, Value... args) -> {
        // Return current Unix timestamp in seconds
        long currentTimeSeconds = System.currentTimeMillis() / 1000;
        return new Value[] { Value.i64(currentTimeSeconds) };
    }
);

// Add to module when instantiating
Module module = Module.builder(wasmBytes)
    .withHostFunction(getCurrentTime)
    .build();

Instance instance = module.instantiate();
```

### Complete Working Example

See `HOST_FUNCTIONS.md` for a complete Java example showing:
- Loading the WASM module with host function
- Updating flag state
- Evaluating flags with context
- Memory management (alloc/dealloc)

## Testing the Fix

1. Rebuild the WASM module:
   ```bash
   cargo build --target wasm32-unknown-unknown --no-default-features --release --lib
   ```

2. Update your Java code to provide the host function (see above)

3. The evaluation should now work with context:
   ```java
   String context = "{\"email\":\"ballmer@macrosoft.com\"}";
   // Should now evaluate successfully without TrapException
   ```

4. Verify `$flagd.timestamp` is available in targeting rules:
   ```json
   {
     "targeting": {
       "if": [
         {">": [{"var": "$flagd.timestamp"}, 1700000000]},
         "variant1",
         "variant2"
       ]
     }
   }
   ```

## Backward Compatibility

If the host function is not provided:
- The WASM module catches the panic and defaults `$flagd.timestamp` to `0`
- Evaluation continues without errors
- Time-based targeting rules will not work correctly

## Related Files

- **HOST_FUNCTIONS.md** - Complete implementation guide for all languages
- **README.md** - Updated with host function requirements
- **src/lib.rs** - Host function import and wrapper
- **src/evaluation.rs** - Context enrichment using host function

## Prevention for Future

All time-sensitive operations in WASM should:
1. Use host functions instead of `std::time::SystemTime`
2. Wrap in `catch_unwind` if panics are possible
3. Provide sensible defaults when unavailable

This pattern should be used for any other system-dependent functionality in WASM.
