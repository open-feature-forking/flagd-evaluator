# Host Functions

The flagd-evaluator WASM module requires the host environment to provide certain functions. These are imported by the WASM module and must be implemented by the runtime (e.g., Java/Chicory, Go/wazero, JavaScript).

## Import Overview

The WASM module declares imports across 3 modules:

| Module | Functions | Stability |
|--------|-----------|-----------|
| `host` | 1 (stable names) | Stable — names never change |
| `__wbindgen_placeholder__` | ~6 (hashed names) | Names change with Rust dependency updates |
| `__wbindgen_externref_xform__` | ~2 (fixed names) | Names are stable but may appear/disappear |

**Important:** The `__wbindgen_placeholder__` function names include a hash suffix (e.g., `__wbg_getTime_ad1e9878a735af08`) that changes whenever Rust dependencies or wasm-bindgen versions change. Host implementations should **match by prefix**, not by exact name. See the [Dynamic Matching](#dynamic-matching-recommended) section.

## Stable Host Functions

### `host::get_current_time_unix_seconds`

**Signature:** `() -> i64`

Provides the current Unix timestamp (seconds since epoch) for `$flagd.timestamp` context enrichment. The WASM sandbox cannot access system time without WASI, so the host must supply it.

**Return value:** Unix timestamp in seconds (e.g., `1735689600` for 2025-01-01 00:00:00 UTC).

**If not provided:** The module defaults `$flagd.timestamp` to `0`. Time-based targeting won't work, but evaluation continues without errors.

## wasm-bindgen Functions

These imports come from Rust dependencies (chrono, getrandom) using wasm-bindgen. Their names contain hashes that change across builds. Match by prefix.

### `__wbg_getRandomValues_*`

**Module:** `__wbindgen_placeholder__`
**Signature:** `(i32, i32) -> void`
**Purpose:** Cryptographic entropy for hash table seeding (ahash in boon JSON schema validation).

The first argument is an externref index (can be ignored). The second argument is a pointer to a 32-byte buffer in WASM memory. Fill the buffer with random bytes.

### `__wbg_new_0_*`

**Module:** `__wbindgen_placeholder__`
**Signature:** `() -> i32`
**Purpose:** JavaScript `Date` constructor shim (used by chrono's wasmbind feature).

Return `0` (dummy reference). The actual timestamp is provided by `host::get_current_time_unix_seconds`.

### `__wbg_getTime_*`

**Module:** `__wbindgen_placeholder__`
**Signature:** `(i32) -> f64`
**Purpose:** JavaScript `Date.getTime()` shim.

Return current time in **milliseconds** as f64. The argument is the Date reference from `new_0` (ignored).

### `__wbg___wbindgen_throw_*`

**Module:** `__wbindgen_placeholder__`
**Signature:** `(i32, i32) -> void`
**Purpose:** Error propagation from WASM to host.

Arguments are (pointer, length) of a UTF-8 error message in WASM memory. The host should throw/raise an exception with the message.

### `__wbindgen_object_drop_ref`, `__wbindgen_describe`

**Module:** `__wbindgen_placeholder__`
**Signature:** `(i32) -> void`
**Purpose:** wasm-bindgen internals. No-ops — these track JavaScript object references which don't exist in non-browser runtimes.

### `__wbindgen_externref_table_grow`

**Module:** `__wbindgen_externref_xform__`
**Signature:** `(i32) -> i32`
**Purpose:** Grow the external reference table. Return a fixed value (e.g., `128`).

### `__wbindgen_externref_table_set_null`

**Module:** `__wbindgen_externref_xform__`
**Signature:** `(i32) -> void`
**Purpose:** Set externref table entry to null. No-op.

## Dynamic Matching (Recommended)

Instead of hardcoding exact function names, inspect the WASM module's import section at startup and match by prefix. This way, hash changes from Rust dependency updates don't require host code changes.

### Java (Chicory)

```java
WasmModule module = CompiledEvaluator.load();
Store store = new Store();

module.importSection().stream()
    .filter(FunctionImport.class::isInstance)
    .map(FunctionImport.class::cast)
    .forEach(fi -> {
        String mod = fi.module();
        String name = fi.name();

        if ("host".equals(mod) && "get_current_time_unix_seconds".equals(name)) {
            // Register timestamp provider
        } else if (name.startsWith("__wbg_getRandomValues_")) {
            // Register random bytes provider
        } else if (name.startsWith("__wbg_getTime_")) {
            // Register Date.getTime shim
        } else if (name.startsWith("__wbg_new_0_")) {
            // Register Date constructor shim (return 0)
        } else if (name.contains("__wbindgen_throw")) {
            // Register throw handler
        } else {
            // Register no-op for all other wasm-bindgen imports
        }
    });
```

See `WasmRuntime.java` for the full implementation.

### Go (wazero)

```go
// Match by prefix when registering host functions
hostBuilder := r.NewHostModuleBuilder("__wbindgen_placeholder__")

// Use the actual import names from the WASM binary
for _, imp := range wasmModule.ImportedFunctions() {
    moduleName, name, _ := imp.Import()
    switch {
    case strings.HasPrefix(name, "__wbg_getRandomValues_"):
        hostBuilder.NewFunctionBuilder().
            WithFunc(func(ctx context.Context, mod api.Module, externref, bufPtr uint32) { ... }).
            Export(name)
    // ... other prefix matches
    }
}
```

### JavaScript

```javascript
// In JavaScript, wasm-bindgen host functions are usually provided
// automatically by the generated JS glue code. For manual usage:
const importObject = {
    host: {
        get_current_time_unix_seconds: () => BigInt(Math.floor(Date.now() / 1000))
    },
    __wbindgen_placeholder__: new Proxy({}, {
        get: (target, name) => {
            // Dynamic proxy handles any wbindgen function name
            if (name.startsWith('__wbg_getRandomValues_'))
                return (ref, ptr) => { crypto.getRandomValues(new Uint8Array(memory.buffer, ptr, 32)); };
            if (name.startsWith('__wbg_getTime_'))
                return (ref) => Date.now();
            // ... etc
            return () => {};  // no-op fallback
        }
    })
};
```

## Testing

Verify host functions work by evaluating a flag with time-based targeting:

```json
{
  "flags": {
    "test-flag": {
      "state": "ENABLED",
      "variants": {"on": true, "off": false},
      "defaultVariant": "off",
      "targeting": {
        "if": [{">": [{"var": "$flagd.timestamp"}, 0]}, "on", "off"]
      }
    }
  }
}
```

The flag should resolve to `"on"` when the timestamp host function is provided correctly.
