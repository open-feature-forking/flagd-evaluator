package dev.openfeature.flagd.evaluator;

import com.dylibso.chicory.runtime.ExportFunction;
import com.dylibso.chicory.runtime.Instance;
import com.dylibso.chicory.runtime.Memory;
import com.fasterxml.jackson.core.JsonFactory;
import com.fasterxml.jackson.core.JsonGenerator;
import com.fasterxml.jackson.databind.JavaType;
import com.fasterxml.jackson.databind.ObjectMapper;
import com.fasterxml.jackson.databind.module.SimpleModule;
import dev.openfeature.flagd.evaluator.jackson.EvaluationContextSerializer;
import dev.openfeature.flagd.evaluator.jackson.EvaluationResultDeserializer;
import dev.openfeature.flagd.evaluator.jackson.ImmutableMetadataDeserializer;
import dev.openfeature.sdk.EvaluationContext;
import dev.openfeature.sdk.ImmutableMetadata;
import dev.openfeature.sdk.Value;

import java.io.ByteArrayOutputStream;
import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.Collections;
import java.util.HashMap;
import java.util.HashSet;
import java.util.List;
import java.util.Map;
import java.util.Set;
import java.util.concurrent.ArrayBlockingQueue;
import java.util.concurrent.CompletableFuture;
import java.util.concurrent.atomic.AtomicLong;
import java.util.concurrent.locks.ReentrantLock;

/**
 * Thread-safe flag evaluator using a pool of flagd-evaluator WASM instances.
 *
 * <p>This class provides a type-safe API for evaluating feature flags using the
 * bundled WASM module. It maintains a pool of WASM instances sized to the number
 * of available processors, enabling parallel evaluation from multiple threads.
 *
 * <p>Pre-evaluated (static/disabled) flags are served lock-free from a volatile
 * cache snapshot. Targeting flags acquire a WASM instance from the pool, evaluate,
 * and return it. This allows near-linear throughput scaling with thread count.
 *
 * <p>Returns {@link EvaluationResult} objects that contain the resolved value,
 * variant, reason, error information, and metadata.
 *
 * <p><b>Example usage:</b>
 * <pre>{@code
 * FlagEvaluator evaluator = new FlagEvaluator();
 *
 * // Load flag configuration
 * String config = "{\"flags\": {...}}";
 * evaluator.updateState(config);
 *
 * // Evaluate a boolean flag
 * EvaluationContext context = new MutableContext().add("targetingKey", "user-123");
 * EvaluationResult<Boolean> result = evaluator.evaluateFlag(Boolean.class, "my-flag", context);
 * System.out.println("Value: " + result.getValue());
 * System.out.println("Variant: " + result.getVariant());
 * }</pre>
 *
 * <p><b>Thread Safety:</b> This class is thread-safe. Multiple threads can call
 * evaluation methods concurrently with near-linear throughput scaling.
 */
public class FlagEvaluator implements AutoCloseable {

    static final ObjectMapper OBJECT_MAPPER = new ObjectMapper();
    private static final JsonFactory JSON_FACTORY = new JsonFactory();
    private static final Map<Class, JavaType> JAVA_TYPE_MAP = new HashMap<>();
    private static final EvaluationContextSerializer CONTEXT_SERIALIZER = new EvaluationContextSerializer();

    // ThreadLocal buffers for reducing allocations
    private static final ThreadLocal<ByteArrayOutputStream> JSON_BUFFER =
        ThreadLocal.withInitial(() -> new ByteArrayOutputStream(8192));

    // Pre-allocated buffer sizes for WASM memory
    private static final int MAX_FLAG_KEY_SIZE = 256;
    private static final int MAX_CONTEXT_SIZE = 1024 * 1024; // 1MB

    static {
        // Register custom serializers/deserializers with the ObjectMapper
        SimpleModule module = new SimpleModule();
        module.addDeserializer(ImmutableMetadata.class, new ImmutableMetadataDeserializer());
        module.addSerializer(EvaluationContext.class, CONTEXT_SERIALIZER);
        module.addDeserializer(EvaluationResult.class, new EvaluationResultDeserializer<>());
        OBJECT_MAPPER.registerModule(module);
        JAVA_TYPE_MAP.put(Integer.class, OBJECT_MAPPER.getTypeFactory()
                .constructParametricType(EvaluationResult.class, Integer.class));
        JAVA_TYPE_MAP.put(Double.class, OBJECT_MAPPER.getTypeFactory()
                .constructParametricType(EvaluationResult.class, Double.class));
        JAVA_TYPE_MAP.put(String.class, OBJECT_MAPPER.getTypeFactory()
                .constructParametricType(EvaluationResult.class, String.class));
        JAVA_TYPE_MAP.put(Boolean.class, OBJECT_MAPPER.getTypeFactory()
                .constructParametricType(EvaluationResult.class, Boolean.class));
        JAVA_TYPE_MAP.put(Value.class, OBJECT_MAPPER.getTypeFactory()
                .constructParametricType(EvaluationResult.class, Value.class));
    }

    /**
     * Encapsulates a single WASM instance with its exports and pre-allocated buffers.
     * Each instance has its own linear memory and can evaluate independently.
     */
    private static class WasmInstance {
        final Instance instance;
        final ExportFunction updateStateFunction;
        final ExportFunction evaluateReusableFunction;
        final ExportFunction evaluateByIndexFunction; // may be null
        final ExportFunction allocFunction;
        final ExportFunction deallocFunction;
        final Memory memory;
        final long flagKeyBufferPtr;
        final long contextBufferPtr;
        long generation; // stamped during updateState

        WasmInstance(Instance instance) {
            this.instance = instance;
            this.updateStateFunction = instance.export("update_state");
            this.evaluateReusableFunction = instance.export("evaluate_reusable");
            this.allocFunction = instance.export("alloc");
            this.deallocFunction = instance.export("dealloc");
            this.memory = instance.memory();

            ExportFunction evalByIndex = null;
            try {
                evalByIndex = instance.export("evaluate_by_index");
            } catch (Exception e) {
                // Older WASM module without evaluate_by_index
            }
            this.evaluateByIndexFunction = evalByIndex;

            this.flagKeyBufferPtr = allocFunction.apply(MAX_FLAG_KEY_SIZE)[0];
            this.contextBufferPtr = allocFunction.apply(MAX_CONTEXT_SIZE)[0];
        }

        void close() {
            deallocFunction.apply(flagKeyBufferPtr, MAX_FLAG_KEY_SIZE);
            deallocFunction.apply(contextBufferPtr, MAX_CONTEXT_SIZE);
        }
    }

    /**
     * Immutable snapshot of host-side caches. Replaced atomically on updateState.
     */
    private static class CacheSnapshot {
        final long generation;
        final Map<String, EvaluationResult<Object>> preEvaluated;
        final Map<String, Set<String>> requiredContextKeys;
        final Map<String, Integer> flagIndices;

        CacheSnapshot(long generation,
                      Map<String, EvaluationResult<Object>> preEvaluated,
                      Map<String, Set<String>> requiredContextKeys,
                      Map<String, Integer> flagIndices) {
            this.generation = generation;
            this.preEvaluated = preEvaluated;
            this.requiredContextKeys = requiredContextKeys;
            this.flagIndices = flagIndices;
        }

        static final CacheSnapshot EMPTY = new CacheSnapshot(
            0, Collections.emptyMap(), Collections.emptyMap(), Collections.emptyMap());
    }

    // Pool of WASM instances
    private final ArrayBlockingQueue<WasmInstance> pool;
    private final int poolSize;

    // Host-side caches — atomically swapped on updateState
    private volatile CacheSnapshot cache = CacheSnapshot.EMPTY;

    // Generation counter — incremented on each updateState
    private final AtomicLong generation = new AtomicLong(0);

    // Serializes updateState calls
    private final ReentrantLock updateLock = new ReentrantLock();

    /**
     * Creates a new flag evaluator with strict validation mode and default pool size.
     */
    public FlagEvaluator() {
        this(ValidationMode.STRICT);
    }

    /**
     * Creates a new flag evaluator with the specified validation mode and default pool size.
     *
     * @param validationMode the validation mode to use
     */
    public FlagEvaluator(ValidationMode validationMode) {
        this(validationMode, Runtime.getRuntime().availableProcessors());
    }

    /**
     * Creates a new flag evaluator with the specified validation mode and pool size.
     *
     * @param validationMode the validation mode to use
     * @param poolSize       the number of WASM instances in the pool
     */
    public FlagEvaluator(ValidationMode validationMode, int poolSize) {
        if (poolSize < 1) {
            throw new IllegalArgumentException("Pool size must be >= 1, got " + poolSize);
        }
        this.poolSize = poolSize;
        this.pool = new ArrayBlockingQueue<>(poolSize);

        for (int i = 0; i < poolSize; i++) {
            WasmInstance inst = createWasmInstance(validationMode);
            pool.add(inst);
        }
    }

    /**
     * Creates a single WASM instance configured with the given validation mode.
     */
    private static WasmInstance createWasmInstance(ValidationMode validationMode) {
        Instance instance = WasmRuntime.createInstance();
        WasmInstance inst = new WasmInstance(instance);
        ExportFunction setValidationMode = instance.export("set_validation_mode");
        setValidationMode.apply(validationMode.getValue());
        return inst;
    }

    /**
     * Updates the flag state across all WASM instances in the pool.
     *
     * <p>The configuration should be a JSON string following the flagd flag schema.
     * All instances are drained from the pool, updated (in parallel for instances
     * beyond the first), then returned with a new generation stamp.
     *
     * @param jsonConfig the flag configuration as JSON
     * @return the update result containing changed flag keys
     * @throws EvaluatorException if the update fails
     */
    public UpdateStateResult updateState(String jsonConfig) throws EvaluatorException {
        updateLock.lock();
        try {
            byte[] configBytes = jsonConfig.getBytes(StandardCharsets.UTF_8);

            // Drain all instances from pool
            List<WasmInstance> instances = new ArrayList<>(poolSize);
            for (int i = 0; i < poolSize; i++) {
                try {
                    instances.add(pool.take());
                } catch (InterruptedException e) {
                    Thread.currentThread().interrupt();
                    // Return any already-drained instances
                    for (WasmInstance inst : instances) {
                        pool.add(inst);
                    }
                    throw new EvaluatorException("Interrupted while draining pool", e);
                }
            }

            try {
                // Update first instance and capture result
                UpdateStateResult result = updateInstance(instances.get(0), configBytes);

                // Update remaining instances in parallel
                if (instances.size() > 1) {
                    CompletableFuture<?>[] futures = new CompletableFuture[instances.size() - 1];
                    for (int i = 1; i < instances.size(); i++) {
                        final WasmInstance inst = instances.get(i);
                        futures[i - 1] = CompletableFuture.runAsync(() -> {
                            try {
                                updateInstance(inst, configBytes);
                            } catch (EvaluatorException e) {
                                throw new RuntimeException(e);
                            }
                        });
                    }
                    CompletableFuture.allOf(futures).join();
                }

                // Increment generation, stamp on cache and all instances
                long gen = generation.incrementAndGet();
                CacheSnapshot newCache = buildCacheSnapshot(gen, result);
                for (WasmInstance inst : instances) {
                    inst.generation = gen;
                }

                // Atomic cache swap
                this.cache = newCache;

                return result;
            } finally {
                // Return all instances to pool
                for (WasmInstance inst : instances) {
                    pool.add(inst);
                }
            }
        } finally {
            updateLock.unlock();
        }
    }

    /**
     * Updates a single WASM instance with the given config bytes.
     */
    private static UpdateStateResult updateInstance(WasmInstance inst, byte[] configBytes) throws EvaluatorException {
        long configPtr = inst.allocFunction.apply(configBytes.length)[0];
        try {
            inst.memory.write((int) configPtr, configBytes);
            long packedResult = inst.updateStateFunction.apply(configPtr, configBytes.length)[0];
            int resultPtr = (int) (packedResult >>> 32);
            int resultLen = (int) (packedResult & 0xFFFFFFFFL);
            String resultJson = inst.memory.readString(resultPtr, resultLen);
            inst.deallocFunction.apply(resultPtr, resultLen);
            return OBJECT_MAPPER.readValue(resultJson, UpdateStateResult.class);
        } catch (Exception e) {
            throw new EvaluatorException("Failed to update state", e);
        } finally {
            inst.deallocFunction.apply(configPtr, configBytes.length);
        }
    }

    /**
     * Builds an immutable CacheSnapshot from an UpdateStateResult.
     */
    private static CacheSnapshot buildCacheSnapshot(long generation, UpdateStateResult result) {
        Map<String, EvaluationResult<Object>> preEval = result.getPreEvaluated();
        if (preEval == null) preEval = Collections.emptyMap();

        Map<String, Set<String>> keySets;
        Map<String, List<String>> reqKeys = result.getRequiredContextKeys();
        if (reqKeys != null) {
            keySets = new HashMap<>(reqKeys.size());
            for (Map.Entry<String, List<String>> entry : reqKeys.entrySet()) {
                keySets.put(entry.getKey(), new HashSet<>(entry.getValue()));
            }
        } else {
            keySets = Collections.emptyMap();
        }

        Map<String, Integer> indices = result.getFlagIndices();
        if (indices == null) indices = Collections.emptyMap();

        return new CacheSnapshot(generation, preEval, keySets, indices);
    }

    /**
     * Evaluates a flag with the given context JSON string.
     *
     * @param <T>         the type of the flag value
     * @param type        the class of the expected flag value type
     * @param flagKey     the key of the flag to evaluate
     * @param contextJson the evaluation context as JSON (use null or "" for empty context)
     * @return the evaluation result containing value, variant, reason, and metadata
     * @throws EvaluatorException if the evaluation fails
     */
    @SuppressWarnings("unchecked")
    public <T> EvaluationResult<T> evaluateFlag(Class<T> type, String flagKey, String contextJson) throws EvaluatorException {
        // Load cache snapshot (lock-free volatile read)
        CacheSnapshot snap = this.cache;

        // Fast path: return cached result for static/disabled flags
        EvaluationResult<Object> cached = snap.preEvaluated.get(flagKey);
        if (cached != null) {
            return (EvaluationResult<T>) (EvaluationResult<?>) cached;
        }

        // Acquire instance from pool
        WasmInstance inst;
        try {
            inst = pool.take();
        } catch (InterruptedException e) {
            Thread.currentThread().interrupt();
            throw new EvaluatorException("Interrupted while acquiring WASM instance", e);
        }
        try {
            // Generation guard: if updateState completed between cache read and pool acquire,
            // reload cache to match the instance's generation
            if (snap.generation != inst.generation) {
                snap = this.cache;
                cached = snap.preEvaluated.get(flagKey);
                if (cached != null) {
                    return (EvaluationResult<T>) (EvaluationResult<?>) cached;
                }
            }

            return evaluateFlagInternal(type, flagKey, contextJson, inst);
        } finally {
            pool.add(inst);
        }
    }

    /**
     * Internal evaluation using flag key string and evaluate_reusable WASM export.
     */
    private <T> EvaluationResult<T> evaluateFlagInternal(Class<T> type, String flagKey, String contextJson, WasmInstance inst) throws EvaluatorException {
        byte[] flagBytes = flagKey.getBytes(StandardCharsets.UTF_8);
        if (flagBytes.length > MAX_FLAG_KEY_SIZE) {
            throw new EvaluatorException("Flag key exceeds maximum size of " + MAX_FLAG_KEY_SIZE + " bytes");
        }

        inst.memory.write((int) inst.flagKeyBufferPtr, flagBytes);

        long contextPtr = 0;
        int contextLen = 0;
        if (contextJson != null && !contextJson.isEmpty()) {
            byte[] contextBytes = contextJson.getBytes(StandardCharsets.UTF_8);
            if (contextBytes.length > MAX_CONTEXT_SIZE) {
                throw new EvaluatorException("Context exceeds maximum size of " + MAX_CONTEXT_SIZE + " bytes");
            }
            inst.memory.write((int) inst.contextBufferPtr, contextBytes);
            contextPtr = inst.contextBufferPtr;
            contextLen = contextBytes.length;
        }

        try {
            long packedResult = inst.evaluateReusableFunction.apply(inst.flagKeyBufferPtr, flagBytes.length, contextPtr, contextLen)[0];
            int resultPtr = (int) (packedResult >>> 32);
            int resultLen = (int) (packedResult & 0xFFFFFFFFL);

            String resultJson = inst.memory.readString(resultPtr, resultLen);
            inst.deallocFunction.apply(resultPtr, resultLen);

            return OBJECT_MAPPER.readValue(resultJson, JAVA_TYPE_MAP.get(type));
        } catch (Exception e) {
            throw new EvaluatorException("Failed to evaluate flag: " + flagKey, e);
        }
    }

    /**
     * Evaluates a flag using the numeric index path (evaluate_by_index WASM export).
     */
    private <T> EvaluationResult<T> evaluateByIndex(Class<T> type, int flagIndex, String contextJson, WasmInstance inst) throws EvaluatorException {
        long contextPtr = 0;
        int contextLen = 0;
        if (contextJson != null && !contextJson.isEmpty()) {
            byte[] contextBytes = contextJson.getBytes(StandardCharsets.UTF_8);
            if (contextBytes.length > MAX_CONTEXT_SIZE) {
                throw new EvaluatorException("Context exceeds maximum size of " + MAX_CONTEXT_SIZE + " bytes");
            }
            inst.memory.write((int) inst.contextBufferPtr, contextBytes);
            contextPtr = inst.contextBufferPtr;
            contextLen = contextBytes.length;
        }

        try {
            long packedResult = inst.evaluateByIndexFunction.apply(flagIndex, contextPtr, contextLen)[0];
            int resultPtr = (int) (packedResult >>> 32);
            int resultLen = (int) (packedResult & 0xFFFFFFFFL);

            String resultJson = inst.memory.readString(resultPtr, resultLen);
            inst.deallocFunction.apply(resultPtr, resultLen);

            return OBJECT_MAPPER.readValue(resultJson, JAVA_TYPE_MAP.get(type));
        } catch (Exception e) {
            throw new EvaluatorException("Failed to evaluate flag by index: " + flagIndex, e);
        }
    }

    /**
     * Evaluates a flag with an EvaluationContext.
     *
     * @param <T>     the type of the flag value
     * @param type    the class of the expected flag value type
     * @param flagKey the key of the flag to evaluate
     * @param context the evaluation context
     * @return the evaluation result containing value, variant, reason, and metadata
     * @throws EvaluatorException if the evaluation or serialization fails
     */
    @SuppressWarnings("unchecked")
    public <T> EvaluationResult<T> evaluateFlag(Class<T> type, String flagKey, EvaluationContext context) throws EvaluatorException {
        try {
            // Load cache snapshot (lock-free volatile read)
            CacheSnapshot snap = this.cache;

            // Fast path: return cached result for static/disabled flags
            EvaluationResult<Object> cached = snap.preEvaluated.get(flagKey);
            if (cached != null) {
                return (EvaluationResult<T>) (EvaluationResult<?>) cached;
            }

            // Fast path: empty context
            if (context == null || context.isEmpty()) {
                return evaluateFlag(type, flagKey, (String) null);
            }

            // Determine context serialization strategy
            Set<String> requiredKeys = snap.requiredContextKeys.get(flagKey);
            String contextJson;
            if (requiredKeys != null) {
                contextJson = EvaluationContextSerializer.serializeFiltered(context, requiredKeys, flagKey);
            } else {
                ByteArrayOutputStream buffer = JSON_BUFFER.get();
                buffer.reset();
                try (JsonGenerator generator = JSON_FACTORY.createGenerator(buffer)) {
                    OBJECT_MAPPER.writeValue(generator, context);
                }
                contextJson = buffer.toString(StandardCharsets.UTF_8.name());
            }

            // Acquire instance from pool
            WasmInstance inst;
            try {
                inst = pool.take();
            } catch (InterruptedException e) {
                Thread.currentThread().interrupt();
                throw new EvaluatorException("Interrupted while acquiring WASM instance", e);
            }
            try {
                // Generation guard
                if (snap.generation != inst.generation) {
                    snap = this.cache;
                    cached = snap.preEvaluated.get(flagKey);
                    if (cached != null) {
                        return (EvaluationResult<T>) (EvaluationResult<?>) cached;
                    }
                    // Re-resolve required keys from new cache
                    requiredKeys = snap.requiredContextKeys.get(flagKey);
                }

                // Check if we can use index-based evaluation
                Integer flagIndex = snap.flagIndices.get(flagKey);
                if (flagIndex != null && inst.evaluateByIndexFunction != null && requiredKeys != null) {
                    return evaluateByIndex(type, flagIndex, contextJson, inst);
                }

                return evaluateFlagInternal(type, flagKey, contextJson, inst);
            } finally {
                pool.add(inst);
            }
        } catch (EvaluatorException e) {
            throw e;
        } catch (Exception e) {
            throw new EvaluatorException("Failed to serialize context", e);
        }
    }

    /**
     * Returns the number of WASM instances in the pool.
     *
     * @return the pool size
     */
    public int getPoolSize() {
        return poolSize;
    }

    @Override
    public void close() {
        // Drain pool non-blocking and deallocate each instance's buffers
        WasmInstance inst;
        while ((inst = pool.poll()) != null) {
            inst.close();
        }
    }

    /**
     * Validation mode determines how validation errors are handled.
     */
    public enum ValidationMode {
        /**
         * Reject invalid flag configurations (strict mode)
         */
        STRICT(0),
        /**
         * Accept invalid flag configurations with warnings (permissive mode)
         */
        PERMISSIVE(1);

        private final int value;

        ValidationMode(int value) {
            this.value = value;
        }

        int getValue() {
            return value;
        }
    }
}
