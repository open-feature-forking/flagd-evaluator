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
import dev.openfeature.sdk.*;

import java.io.ByteArrayOutputStream;
import java.nio.charset.StandardCharsets;
import java.util.HashMap;
import java.util.Map;

/**
 * Thread-safe flag evaluator using the flagd-evaluator WASM module.
 *
 * <p>This class provides a type-safe API for evaluating feature flags using the
 * bundled WASM module. Each instance maintains its own WASM instance and can be
 * used concurrently from multiple threads.
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
 * String context = "{\"targetingKey\": \"user-123\"}";
 * EvaluationResult<Boolean> result = evaluator.evaluateFlag(Boolean.class, "my-flag", context);
 * System.out.println("Value: " + result.getValue());
 * System.out.println("Variant: " + result.getVariant());
 * }</pre>
 *
 * <p><b>Thread Safety:</b> This class is thread-safe. Multiple threads can call
 * evaluation methods concurrently.
 */
public class FlagEvaluator implements AutoCloseable {

    static final ObjectMapper OBJECT_MAPPER = new ObjectMapper();
    private static final JsonFactory JSON_FACTORY = new JsonFactory();
    private static final Map<Class, JavaType> JAVA_TYPE_MAP = new HashMap<>();
    private static final EvaluationContextSerializer CONTEXT_SERIALIZER = new EvaluationContextSerializer();

    // ThreadLocal buffers for reducing allocations
    private static final ThreadLocal<ByteArrayOutputStream> JSON_BUFFER =
        ThreadLocal.withInitial(() -> new ByteArrayOutputStream(8192));

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

    private final Instance wasmInstance;
    private final ExportFunction updateStateFunction;
    private final ExportFunction evaluateFunction;
    private final ExportFunction allocFunction;
    private final ExportFunction deallocFunction;
    private final Memory memory;

    /**
     * Creates a new flag evaluator with strict validation mode.
     *
     * <p>In strict mode, invalid flag configurations will be rejected.
     */
    public FlagEvaluator() {
        this(ValidationMode.STRICT);
    }

    /**
     * Creates a new flag evaluator with the specified validation mode.
     *
     * @param validationMode the validation mode to use
     */
    public FlagEvaluator(ValidationMode validationMode) {
        this.wasmInstance = WasmRuntime.createInstance();
        this.updateStateFunction = wasmInstance.export("update_state");
        this.evaluateFunction = wasmInstance.export("evaluate");
        this.allocFunction = wasmInstance.export("alloc");
        this.deallocFunction = wasmInstance.export("dealloc");
        this.memory = wasmInstance.memory();

        // Set validation mode
        ExportFunction setValidationMode = wasmInstance.export("set_validation_mode");
        setValidationMode.apply(validationMode.getValue());
    }

    /**
     * Updates the flag state with a new configuration.
     *
     * <p>The configuration should be a JSON string following the flagd flag schema:
     * <pre>{@code
     * {
     *   "flags": {
     *     "my-flag": {
     *       "state": "ENABLED",
     *       "defaultVariant": "on",
     *       "variants": {
     *         "on": true,
     *         "off": false
     *       }
     *     }
     *   }
     * }
     * }</pre>
     *
     * @param jsonConfig the flag configuration as JSON
     * @return the update result containing changed flag keys
     * @throws EvaluatorException if the update fails
     */
    public synchronized UpdateStateResult updateState(String jsonConfig) throws EvaluatorException {
        // Use explicit UTF-8 encoding for better performance
        byte[] configBytes = jsonConfig.getBytes(StandardCharsets.UTF_8);
        long configPtr = allocFunction.apply(configBytes.length)[0];

        try {
            memory.write((int) configPtr, configBytes);

            long packedResult = updateStateFunction.apply(configPtr, configBytes.length)[0];
            int resultPtr = (int) (packedResult >>> 32);
            int resultLen = (int) (packedResult & 0xFFFFFFFFL);

            String resultJson = memory.readString(resultPtr, resultLen);

            return OBJECT_MAPPER.readValue(resultJson, UpdateStateResult.class);
        } catch (Exception e) {
            throw new EvaluatorException("Failed to update state", e);
        } finally {
            deallocFunction.apply(configPtr, configBytes.length);
        }
    }

    /**
     * Evaluates a flag with the given context using type-safe evaluation.
     *
     * <p>Returns an {@link EvaluationResult} with the resolved value, variant,
     * reason, error information, and metadata.
     *
     * <p>The context should be a JSON string with evaluation context properties:
     * <pre>{@code
     * {
     *   "targetingKey": "user-123",
     *   "email": "user@example.com",
     *   "age": 25
     * }
     * }</pre>
     *
     * <p><b>Supported types:</b>
     * <ul>
     *   <li>{@code Boolean.class} - For boolean flags</li>
     *   <li>{@code String.class} - For string flags</li>
     *   <li>{@code Integer.class} - For integer flags</li>
     *   <li>{@code Double.class} - For double/number flags</li>
     *   <li>{@code Value.class} - For structured/object flags</li>
     * </ul>
     *
     * @param <T>         the type of the flag value
     * @param type        the class of the expected flag value type
     * @param flagKey     the key of the flag to evaluate
     * @param contextJson the evaluation context as JSON (use "{}" for empty context)
     * @return the evaluation result containing value, variant, reason, and metadata
     * @throws EvaluatorException if the evaluation fails
     */
    public synchronized <T> EvaluationResult<T> evaluateFlag(Class<T> type, String flagKey, String contextJson) throws EvaluatorException {
        // Use explicit UTF-8 encoding for better performance
        byte[] flagBytes = flagKey.getBytes(StandardCharsets.UTF_8);
        long flagPtr = allocFunction.apply(flagBytes.length)[0];

        byte[] contextBytes = contextJson.getBytes(StandardCharsets.UTF_8);
        long contextPtr = allocFunction.apply(contextBytes.length)[0];

        try {
            memory.write((int) flagPtr, flagBytes);
            memory.write((int) contextPtr, contextBytes);

            long packedResult = evaluateFunction.apply(flagPtr, flagBytes.length, contextPtr, contextBytes.length)[0];
            int resultPtr = (int) (packedResult >>> 32);
            int resultLen = (int) (packedResult & 0xFFFFFFFFL);

            String resultJson = memory.readString(resultPtr, resultLen);

            return OBJECT_MAPPER.readValue(resultJson, JAVA_TYPE_MAP.get(type));
        } catch (Exception e) {
            throw new EvaluatorException("Failed to evaluate flag: " + flagKey, e);
        } finally {
            deallocFunction.apply(flagPtr, flagBytes.length);
            deallocFunction.apply(contextPtr, contextBytes.length);
        }
    }

    /**
     * Evaluates a flag with a Map-based context using type-safe evaluation.
     *
     * <p>This is a convenience method that converts the context map to JSON before evaluation.
     *
     * @param <T>     the type of the flag value
     * @param type    the class of the expected flag value type
     * @param flagKey the key of the flag to evaluate
     * @param context the evaluation context as a Map
     * @return the evaluation result containing value, variant, reason, and metadata
     * @throws EvaluatorException if the evaluation or serialization fails
     */
    public <T> EvaluationResult<T> evaluateFlag(Class<T> type, String flagKey, EvaluationContext context) throws EvaluatorException {
        try {
            String contextJson;
            if (context == null || context.isEmpty()) {
                contextJson = "{}";
            } else {
                // Use ThreadLocal buffer for streaming serialization
                ByteArrayOutputStream buffer = JSON_BUFFER.get();
                buffer.reset();
                try (JsonGenerator generator = JSON_FACTORY.createGenerator(buffer)) {
                    OBJECT_MAPPER.writeValue(generator, context);
                }
                contextJson = buffer.toString(StandardCharsets.UTF_8.name());
            }
            return evaluateFlag(type, flagKey, contextJson);
        } catch (Exception e) {
            throw new EvaluatorException("Failed to serialize context", e);
        }
    }

    @Override
    public void close() {
        // WASM instances don't need explicit cleanup in Chicory
        // This method is here to support try-with-resources
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
