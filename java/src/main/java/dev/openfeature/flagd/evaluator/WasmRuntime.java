package dev.openfeature.flagd.evaluator;

import com.dylibso.chicory.runtime.HostFunction;
import com.dylibso.chicory.runtime.Instance;
import com.dylibso.chicory.runtime.Memory;
import com.dylibso.chicory.runtime.Store;
import com.dylibso.chicory.wasm.WasmModule;
import com.dylibso.chicory.wasm.types.FunctionImport;
import com.dylibso.chicory.wasm.types.FunctionType;
import com.dylibso.chicory.wasm.types.ValType;
import java.security.SecureRandom;
import java.util.List;

/**
 * Runtime environment for the flagd-evaluator WASM module.
 *
 * <p>This class handles:
 * <ul>
 *   <li>Loading and compiling the WASM module from classpath
 *   <li>Providing required host functions for the WASM module
 *   <li>Creating configured WASM instances with AOT compilation
 * </ul>
 *
 * <p>Host functions are registered dynamically by inspecting the WASM module's import
 * section. This eliminates hardcoded wasm-bindgen hash suffixes that break when Rust
 * dependencies change. Functions are matched by stable prefix patterns:
 * <ul>
 *   <li>{@code host::get_current_time_unix_seconds} — Unix timestamp
 *   <li>{@code __wbg_getRandomValues_*} — cryptographic entropy
 *   <li>{@code __wbg_new_0_*} / {@code __wbg_getTime_*} — Date shim (legacy)
 *   <li>{@code __wbindgen_throw_*} — error propagation
 *   <li>All other wasm-bindgen imports — no-ops
 * </ul>
 */
public final class WasmRuntime {

    private static final SecureRandom SECURE_RANDOM = new SecureRandom();

    private WasmRuntime() {
        // Utility class - prevent instantiation
    }

    /**
     * Create a configured WASM instance ready for flag evaluation.
     *
     * <p>Loads the WASM module, inspects its imports, dynamically registers matching
     * host functions, and builds the instance with Chicory's JIT compiler.
     *
     * @return a WASM instance ready for use
     */
    public static Instance createInstance() {
        WasmModule module = CompiledEvaluator.load();
        Store store = createStoreWithHostFunctions(module);
        return Instance.builder(module)
                .withImportValues(store.toImportValues())
                .withMachineFactory(CompiledEvaluator::create)
                .build();
    }

    /**
     * Create a Store with host functions matched to the WASM module's actual imports.
     *
     * <p>Iterates the module's import section and registers handlers based on
     * prefix matching, so wasm-bindgen hash changes don't require code updates.
     *
     * @param module the loaded WASM module to inspect
     * @return a Store configured with all required host functions
     */
    static Store createStoreWithHostFunctions(WasmModule module) {
        Store store = new Store();

        module.importSection().stream()
                .filter(FunctionImport.class::isInstance)
                .map(FunctionImport.class::cast)
                .forEach(fi -> {
                    HostFunction hf = createHostFunction(fi.module(), fi.name());
                    if (hf != null) {
                        store.addFunction(hf);
                    }
                });

        return store;
    }

    // ========================================================================
    // Dynamic Host Function Matching
    // ========================================================================

    /**
     * Create a host function for the given import, matched by module and name prefix.
     *
     * @return a HostFunction implementation, or null if unrecognized
     */
    private static HostFunction createHostFunction(String module, String name) {
        if ("host".equals(module)) {
            return createStableHostFunction(name);
        }
        if ("__wbindgen_placeholder__".equals(module)) {
            return createWbindgenFunction(module, name);
        }
        if ("__wbindgen_externref_xform__".equals(module)) {
            return createExternrefFunction(module, name);
        }
        return null;
    }

    /**
     * Create a handler for stable host:: imports (known names, no hashes).
     */
    private static HostFunction createStableHostFunction(String name) {
        if ("get_current_time_unix_seconds".equals(name)) {
            return new HostFunction(
                    "host", name,
                    FunctionType.of(List.of(), List.of(ValType.I64)),
                    (Instance instance, long... args) -> {
                        long currentTimeSeconds = System.currentTimeMillis() / 1000;
                        return new long[] {currentTimeSeconds};
                    });
        }
        // Unknown host function — register a no-op to avoid link errors
        return null;
    }

    /**
     * Create a handler for __wbindgen_placeholder__ imports, matched by name prefix.
     * The hash suffix is ignored so these survive dependency updates.
     */
    private static HostFunction createWbindgenFunction(String module, String name) {
        if (name.startsWith("__wbg_getRandomValues_")) {
            // Cryptographic entropy for ahash (boon validation hash table seeding)
            return new HostFunction(
                    module, name,
                    FunctionType.of(List.of(ValType.I32, ValType.I32), List.of()),
                    (Instance instance, long... args) -> {
                        int bufferPtr = (int) args[1];
                        byte[] randomBytes = new byte[32];
                        SECURE_RANDOM.nextBytes(randomBytes);
                        instance.memory().write(bufferPtr, randomBytes);
                        return null;
                    });
        }
        if (name.startsWith("__wbg_new_0_")) {
            // Date constructor shim — return dummy reference
            return new HostFunction(
                    module, name,
                    FunctionType.of(List.of(), List.of(ValType.I32)),
                    (Instance instance, long... args) -> new long[] {0L});
        }
        if (name.startsWith("__wbg_getTime_")) {
            // Date.getTime shim — return current time in milliseconds as f64
            return new HostFunction(
                    module, name,
                    FunctionType.of(List.of(ValType.I32), List.of(ValType.F64)),
                    (Instance instance, long... args) -> {
                        return new long[] {
                            Double.doubleToRawLongBits((double) System.currentTimeMillis())
                        };
                    });
        }
        if (name.contains("__wbindgen_throw")) {
            // Error propagation — read message from WASM memory and throw
            return new HostFunction(
                    module, name,
                    FunctionType.of(List.of(ValType.I32, ValType.I32), List.of()),
                    (Instance instance, long... args) -> {
                        int ptr = (int) args[0];
                        int len = (int) args[1];
                        String message = instance.memory().readString(ptr, len);
                        throw new RuntimeException("WASM threw: " + message);
                    });
        }
        // All other wbindgen functions (describe, object_drop_ref, etc.) — no-op
        return new HostFunction(
                module, name,
                FunctionType.of(List.of(ValType.I32), List.of()),
                (Instance instance, long... args) -> null);
    }

    /**
     * Create a handler for __wbindgen_externref_xform__ imports.
     */
    private static HostFunction createExternrefFunction(String module, String name) {
        if (name.equals("__wbindgen_externref_table_grow")) {
            return new HostFunction(
                    module, name,
                    FunctionType.of(List.of(ValType.I32), List.of(ValType.I32)),
                    (Instance instance, long... args) -> new long[] {128L});
        }
        // table_set_null and any other externref functions — no-op
        return new HostFunction(
                module, name,
                FunctionType.of(List.of(ValType.I32), List.of()),
                (Instance instance, long... args) -> null);
    }
}
