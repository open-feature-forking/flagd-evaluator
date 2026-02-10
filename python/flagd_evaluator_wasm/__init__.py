"""Python WASM-based flag evaluator using wasmtime.

Mirrors the Go (wazero) and Java (Chicory) implementations: filter context
on the host side, serialize only needed keys as JSON, call into the shared
flagd-evaluator WASM binary.
"""

import json
import os
import time
import threading
from pathlib import Path

import wasmtime

__all__ = ["WasmFlagEvaluator"]

_WASM_PATH = Path(__file__).parent / "flagd_evaluator.wasm"

# Pre-allocated buffer sizes (same as Go/Java)
_MAX_FLAG_KEY_SIZE = 256
_MAX_CONTEXT_SIZE = 1024 * 1024  # 1 MB


def _unpack_ptr_len(packed: int) -> tuple:
    """Unpack a u64 into (ptr_u32, len_u32)."""
    ptr = (packed >> 32) & 0xFFFFFFFF
    length = packed & 0xFFFFFFFF
    return ptr, length


class WasmFlagEvaluator:
    """Feature flag evaluator backed by the flagd-evaluator WASM module.

    Thread-safe: all WASM calls are serialized with a lock.
    """

    def __init__(self, *, permissive: bool = False):
        engine = wasmtime.Engine()
        self._store = wasmtime.Store(engine)
        linker = wasmtime.Linker(engine)

        # Register host functions before instantiation
        self._register_host_functions(linker)

        module = wasmtime.Module.from_file(engine, str(_WASM_PATH))
        instance = linker.instantiate(self._store, module)

        # Look up WASM exports
        self._memory = instance.exports(self._store)["memory"]
        self._alloc = instance.exports(self._store)["alloc"]
        self._dealloc = instance.exports(self._store)["dealloc"]
        self._update_state_fn = instance.exports(self._store)["update_state"]
        self._eval_reusable_fn = instance.exports(self._store)["evaluate_reusable"]

        # evaluate_by_index may not exist in older WASM builds
        try:
            self._eval_by_index_fn = instance.exports(self._store)["evaluate_by_index"]
        except KeyError:
            self._eval_by_index_fn = None

        try:
            self._set_validation_fn = instance.exports(self._store)["set_validation_mode"]
        except KeyError:
            self._set_validation_fn = None

        # Pre-allocate buffers
        self._flag_key_buf_ptr = self._alloc(self._store, _MAX_FLAG_KEY_SIZE)
        self._context_buf_ptr = self._alloc(self._store, _MAX_CONTEXT_SIZE)

        # Set validation mode
        if self._set_validation_fn is not None:
            mode = 1 if permissive else 0
            self._set_validation_fn(self._store, mode)

        # Host-side caches (populated by update_state)
        self._pre_evaluated: dict = {}
        self._required_context_keys: dict = {}
        self._flag_indices: dict = {}

        self._lock = threading.Lock()
        self._closed = False

    # ------------------------------------------------------------------
    # Host function registration
    # ------------------------------------------------------------------

    def _register_host_functions(self, linker: wasmtime.Linker):
        """Register the 9 host functions required by the WASM module."""
        store = self._store
        i32 = wasmtime.ValType.i32()
        i64 = wasmtime.ValType.i64()
        f64 = wasmtime.ValType.f64()

        # --- Module "host" ---
        linker.define(
            store,
            "host",
            "get_current_time_unix_seconds",
            wasmtime.Func(
                store,
                wasmtime.FuncType([], [i64]),
                lambda: [int(time.time())],
            ),
        )

        # --- Module "__wbindgen_placeholder__" ---
        wbp = "__wbindgen_placeholder__"

        # getRandomValues(self: i32, buffer_ptr: i32) -> void
        # Needs access_caller=True to write random bytes into WASM memory
        def _get_random_values(caller, _self_ref, buf_ptr):
            mem = caller["memory"]
            random_bytes = os.urandom(32)
            mem.write(caller, random_bytes, buf_ptr)

        linker.define(
            store,
            wbp,
            "__wbg_getRandomValues_1c61fac11405ffdc",
            wasmtime.Func(
                store,
                wasmtime.FuncType([i32, i32], []),
                _get_random_values,
                access_caller=True,
            ),
        )

        # new_0() -> i32  (Date constructor stub)
        linker.define(
            store,
            wbp,
            "__wbg_new_0_23cedd11d9b40c9d",
            wasmtime.Func(store, wasmtime.FuncType([], [i32]), lambda: [0]),
        )

        # getTime(self: i32) -> f64  (Date.getTime in millis)
        linker.define(
            store,
            wbp,
            "__wbg_getTime_ad1e9878a735af08",
            wasmtime.Func(
                store,
                wasmtime.FuncType([i32], [f64]),
                lambda _s: [time.time() * 1000.0],
            ),
        )

        # __wbindgen_throw(ptr: i32, len: i32) -> void
        # Needs access_caller=True to read error message from WASM memory
        def _wbindgen_throw(caller, ptr, length):
            mem = caller["memory"]
            data = mem.read(caller, ptr, ptr + length)
            msg = bytes(data).decode("utf-8", errors="replace")
            raise RuntimeError(f"WASM threw: {msg}")

        linker.define(
            store,
            wbp,
            "__wbg___wbindgen_throw_dd24417ed36fc46e",
            wasmtime.Func(
                store,
                wasmtime.FuncType([i32, i32], []),
                _wbindgen_throw,
                access_caller=True,
            ),
        )

        # object_drop_ref(idx: i32) -> void
        linker.define(
            store,
            wbp,
            "__wbindgen_object_drop_ref",
            wasmtime.Func(
                store, wasmtime.FuncType([i32], []), lambda _idx: None
            ),
        )

        # describe(idx: i32) -> void
        linker.define(
            store,
            wbp,
            "__wbindgen_describe",
            wasmtime.Func(
                store, wasmtime.FuncType([i32], []), lambda _idx: None
            ),
        )

        # --- Module "__wbindgen_externref_xform__" ---
        xform = "__wbindgen_externref_xform__"

        # table_grow(delta: i32) -> i32
        linker.define(
            store,
            xform,
            "__wbindgen_externref_table_grow",
            wasmtime.Func(
                store, wasmtime.FuncType([i32], [i32]), lambda _d: [128]
            ),
        )

        # table_set_null(idx: i32) -> void
        linker.define(
            store,
            xform,
            "__wbindgen_externref_table_set_null",
            wasmtime.Func(
                store, wasmtime.FuncType([i32], []), lambda _idx: None
            ),
        )

    # ------------------------------------------------------------------
    # Memory helpers
    # ------------------------------------------------------------------

    def _write_to_wasm(self, data: bytes) -> tuple:
        """Allocate WASM memory and write data. Returns (ptr, len).
        Caller must dealloc."""
        data_len = len(data)
        ptr = self._alloc(self._store, data_len)
        self._memory.write(self._store, data, ptr)
        return ptr, data_len

    def _write_to_prealloc(self, buf_ptr: int, buf_size: int, data: bytes):
        """Write data into a pre-allocated buffer with bounds check."""
        if len(data) > buf_size:
            raise ValueError(f"data size {len(data)} exceeds buffer size {buf_size}")
        self._memory.write(self._store, data, buf_ptr)

    def _read_from_wasm(self, ptr: int, length: int) -> bytes:
        """Read bytes from WASM memory (returns a copy)."""
        return bytes(self._memory.read(self._store, ptr, ptr + length))

    # ------------------------------------------------------------------
    # Context serialization
    # ------------------------------------------------------------------

    def _serialize_filtered_context(
        self, flag_key: str, context: dict, required_keys: set
    ) -> bytes:
        """Serialize only the keys the targeting rule references, plus enrichment."""
        filtered = {}
        for key in required_keys:
            if key in ("targetingKey", "$flagd.flagKey", "$flagd.timestamp"):
                continue
            if key in context:
                filtered[key] = context[key]

        # Always include targetingKey
        filtered["targetingKey"] = context.get("targetingKey", "")

        # $flagd enrichment
        filtered["$flagd"] = {
            "flagKey": flag_key,
            "timestamp": int(time.time()),
        }
        return json.dumps(filtered, separators=(",", ":")).encode("utf-8")

    # ------------------------------------------------------------------
    # Public API
    # ------------------------------------------------------------------

    def update_state(self, config: dict) -> dict:
        """Update flag configuration. Accepts a dict, returns result dict.

        Populates internal caches for pre-evaluated flags, required context
        keys, and flag indices.
        """
        with self._lock:
            config_bytes = json.dumps(config, separators=(",", ":")).encode("utf-8")
            config_ptr, config_len = self._write_to_wasm(config_bytes)
            try:
                packed = self._update_state_fn(self._store, config_ptr, config_len)
            finally:
                self._dealloc(self._store, config_ptr, config_len)

            result_ptr, result_len = _unpack_ptr_len(packed)
            result_bytes = self._read_from_wasm(result_ptr, result_len)
            self._dealloc(self._store, result_ptr, result_len)

            result = json.loads(result_bytes)

            # Populate pre-evaluated cache
            self._pre_evaluated = result.get("preEvaluated") or {}

            # Populate required context keys cache (list -> set)
            raw_keys = result.get("requiredContextKeys") or {}
            self._required_context_keys = {
                k: set(v) for k, v in raw_keys.items()
            }

            # Populate flag index cache
            self._flag_indices = result.get("flagIndices") or {}

            return result

    def evaluate(self, flag_key: str, context: dict) -> dict:
        """Evaluate a flag and return the full result dict."""
        with self._lock:
            return self._evaluate_locked(flag_key, context)

    def evaluate_bool(self, flag_key: str, context: dict, default: bool) -> bool:
        """Evaluate a boolean flag. Returns default on error."""
        with self._lock:
            result = self._evaluate_locked(flag_key, context)
        if result.get("errorCode") or result.get("reason") == "ERROR":
            return default
        value = result.get("value")
        if isinstance(value, bool):
            return value
        return default

    def evaluate_string(self, flag_key: str, context: dict, default: str) -> str:
        """Evaluate a string flag. Returns default on error."""
        with self._lock:
            result = self._evaluate_locked(flag_key, context)
        if result.get("errorCode") or result.get("reason") == "ERROR":
            return default
        value = result.get("value")
        if isinstance(value, str):
            return value
        return default

    def evaluate_int(self, flag_key: str, context: dict, default: int) -> int:
        """Evaluate an integer flag. Returns default on error."""
        with self._lock:
            result = self._evaluate_locked(flag_key, context)
        if result.get("errorCode") or result.get("reason") == "ERROR":
            return default
        value = result.get("value")
        if isinstance(value, (int, float)):
            return int(value)
        return default

    def evaluate_float(self, flag_key: str, context: dict, default: float) -> float:
        """Evaluate a float flag. Returns default on error."""
        with self._lock:
            result = self._evaluate_locked(flag_key, context)
        if result.get("errorCode") or result.get("reason") == "ERROR":
            return default
        value = result.get("value")
        if isinstance(value, (int, float)):
            return float(value)
        return default

    def close(self):
        """Release WASM resources."""
        with self._lock:
            if self._closed:
                return
            self._dealloc(self._store, self._flag_key_buf_ptr, _MAX_FLAG_KEY_SIZE)
            self._dealloc(self._store, self._context_buf_ptr, _MAX_CONTEXT_SIZE)
            self._closed = True

    # ------------------------------------------------------------------
    # Internal evaluation pipeline
    # ------------------------------------------------------------------

    def _evaluate_locked(self, flag_key: str, context: dict) -> dict:
        """Internal evaluation pipeline. Caller must hold self._lock."""
        # Fast path: pre-evaluated cache hit (static/disabled flags)
        cached = self._pre_evaluated.get(flag_key)
        if cached is not None:
            return cached

        # Determine context serialization strategy
        context_bytes = b""
        required_keys = self._required_context_keys.get(flag_key)
        if required_keys is not None and context:
            # Filtered path: only serialize keys the targeting rule references
            context_bytes = self._serialize_filtered_context(
                flag_key, context, required_keys
            )
        elif context:
            # Full serialization (no context key info available)
            enriched = dict(context)
            enriched.setdefault("targetingKey", "")
            enriched["$flagd"] = {
                "flagKey": flag_key,
                "timestamp": int(time.time()),
            }
            context_bytes = json.dumps(enriched, separators=(",", ":")).encode("utf-8")

        # Choose evaluation path
        flag_index = self._flag_indices.get(flag_key)
        if (
            flag_index is not None
            and self._eval_by_index_fn is not None
            and required_keys is not None
        ):
            return self._evaluate_by_index(flag_index, context_bytes)

        return self._evaluate_reusable(flag_key, context_bytes)

    def _evaluate_by_index(self, flag_index: int, context_bytes: bytes) -> dict:
        """Call evaluate_by_index WASM export."""
        context_ptr = 0
        context_len = 0

        if context_bytes:
            self._write_to_prealloc(
                self._context_buf_ptr, _MAX_CONTEXT_SIZE, context_bytes
            )
            context_ptr = self._context_buf_ptr
            context_len = len(context_bytes)

        packed = self._eval_by_index_fn(
            self._store, flag_index, context_ptr, context_len
        )
        return self._read_eval_result(packed)

    def _evaluate_reusable(self, flag_key: str, context_bytes: bytes) -> dict:
        """Call evaluate_reusable WASM export."""
        flag_bytes = flag_key.encode("utf-8")
        self._write_to_prealloc(
            self._flag_key_buf_ptr, _MAX_FLAG_KEY_SIZE, flag_bytes
        )

        context_ptr = 0
        context_len = 0
        if context_bytes:
            self._write_to_prealloc(
                self._context_buf_ptr, _MAX_CONTEXT_SIZE, context_bytes
            )
            context_ptr = self._context_buf_ptr
            context_len = len(context_bytes)

        packed = self._eval_reusable_fn(
            self._store,
            self._flag_key_buf_ptr,
            len(flag_bytes),
            context_ptr,
            context_len,
        )
        return self._read_eval_result(packed)

    def _read_eval_result(self, packed: int) -> dict:
        """Read and parse an evaluation result from a packed u64."""
        result_ptr, result_len = _unpack_ptr_len(packed)
        result_bytes = self._read_from_wasm(result_ptr, result_len)
        self._dealloc(self._store, result_ptr, result_len)
        return json.loads(result_bytes)
