# Host Functions

The flagd-evaluator WASM module requires the host environment to provide certain functions. These are imported by the WASM module and must be implemented by the runtime (e.g., Java/Chicory, Go/wazero, JavaScript, .NET/Wasmtime).

## Import Overview

The WASM module declares a single import:

| Module | Function | Stability |
|--------|----------|-----------|
| `host` | `get_current_time_unix_seconds` | Stable — name never changes |

> **Note:** Previous versions of this module also imported functions from `__wbindgen_placeholder__` and `__wbindgen_externref_xform__` (wasm-bindgen runtime shims for getrandom and chrono). These were eliminated by switching to a custom getrandom backend (`getrandom_backend="custom"`) and using `datalogic-rs` without its optional `wasm` feature. Host implementations no longer need to provide these functions.

## `host::get_current_time_unix_seconds`

**Signature:** `() -> i64`

Provides the current Unix timestamp (seconds since epoch) for `$flagd.timestamp` context enrichment. The WASM sandbox cannot access system time without WASI, so the host must supply it.

**Return value:** Unix timestamp in seconds (e.g., `1735689600` for 2025-01-01 00:00:00 UTC).

**If not provided:** The module defaults `$flagd.timestamp` to `0`. Time-based targeting won't work, but evaluation continues without errors.

## Implementation Examples

### Java (Chicory)

```java
HostFunction timeFunc = new HostFunction(
    (Instance instance, Value... args) -> new Value[]{ Value.i64(Instant.now().getEpochSecond()) },
    "host",
    "get_current_time_unix_seconds",
    List.of(),
    List.of(ValueType.I64)
);
Instance instance = Instance.builder(module).withHostFunction(timeFunc).build();
```

### Go (wazero)

```go
hostBuilder := r.NewHostModuleBuilder("host")
hostBuilder.NewFunctionBuilder().
    WithFunc(func() int64 { return time.Now().Unix() }).
    Export("get_current_time_unix_seconds")
_, err = hostBuilder.Instantiate(ctx)
```

### JavaScript

```javascript
const importObject = {
    host: {
        get_current_time_unix_seconds: () => BigInt(Math.floor(Date.now() / 1000))
    }
};
const { instance } = await WebAssembly.instantiate(wasmBytes, importObject);
```

### .NET (Wasmtime)

```csharp
linker.DefineFunction("host", "get_current_time_unix_seconds",
    () => DateTimeOffset.UtcNow.ToUnixTimeSeconds());
```

### Python (wasmtime)

```python
def get_current_time_unix_seconds() -> int:
    return int(time.time())

instance = Instance(store, module, [Func(store, FuncType([], [ValType.i64()]),
    get_current_time_unix_seconds)])
```

## Testing

Verify the host function works by evaluating a flag with time-based targeting:

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
