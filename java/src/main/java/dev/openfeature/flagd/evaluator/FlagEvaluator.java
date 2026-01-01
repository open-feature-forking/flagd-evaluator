package dev.openfeature.flagd.evaluator;

import com.dylibso.chicory.runtime.ExportFunction;
import com.dylibso.chicory.runtime.Instance;
import com.dylibso.chicory.runtime.Memory;
import com.fasterxml.jackson.core.JsonFactory;
import com.fasterxml.jackson.core.JsonGenerator;
import com.fasterxml.jackson.databind.JavaType;
import com.fasterxml.jackson.databind.ObjectMapper;
import com.fasterxml.jackson.databind.module.SimpleModule;
import com.google.protobuf.InvalidProtocolBufferException;
import dev.openfeature.flagd.evaluator.jackson.EvaluationContextSerializer;
import dev.openfeature.flagd.evaluator.jackson.EvaluationResultDeserializer;
import dev.openfeature.flagd.evaluator.jackson.ImmutableMetadataDeserializer;
import dev.openfeature.flagd.evaluator.proto.EvaluationProto;
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
    private final ExportFunction evaluateBinaryFunction;
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
        this.evaluateBinaryFunction = wasmInstance.export("evaluate_binary");
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

        // Optimization: pass null pointer (0) for empty context to skip allocation
        // The WASM module handles null context as empty object
        boolean hasContext = contextJson != null && !contextJson.isEmpty() && !contextJson.equals("{}");
        long contextPtr = 0;
        int contextLen = 0;
        if (hasContext) {
            byte[] contextBytes = contextJson.getBytes(StandardCharsets.UTF_8);
            contextPtr = allocFunction.apply(contextBytes.length)[0];
            contextLen = contextBytes.length;
            memory.write((int) contextPtr, contextBytes);
        }

        try {
            memory.write((int) flagPtr, flagBytes);

            long packedResult = evaluateFunction.apply(flagPtr, flagBytes.length, contextPtr, contextLen)[0];
            int resultPtr = (int) (packedResult >>> 32);
            int resultLen = (int) (packedResult & 0xFFFFFFFFL);

            String resultJson = memory.readString(resultPtr, resultLen);

            return OBJECT_MAPPER.readValue(resultJson, JAVA_TYPE_MAP.get(type));
        } catch (Exception e) {
            throw new EvaluatorException("Failed to evaluate flag: " + flagKey, e);
        }
        // Note: input buffers (flagPtr, contextPtr) are freed by WASM internally
        // Only need to free the result buffer, but that's also managed by WASM
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
            // Fast path: empty context passed as null string (WASM handles this efficiently)
            if (context == null || context.isEmpty()) {
                return evaluateFlag(type, flagKey, (String) null);
            }
            // Use ThreadLocal buffer for streaming serialization
            ByteArrayOutputStream buffer = JSON_BUFFER.get();
            buffer.reset();
            try (JsonGenerator generator = JSON_FACTORY.createGenerator(buffer)) {
                OBJECT_MAPPER.writeValue(generator, context);
            }
            return evaluateFlag(type, flagKey, buffer.toString(StandardCharsets.UTF_8.name()));
        } catch (Exception e) {
            throw new EvaluatorException("Failed to serialize context", e);
        }
    }

    /**
     * Evaluates a flag using the binary protobuf protocol for better performance.
     *
     * <p>This method uses protobuf for the WASM response instead of JSON, which is
     * faster to parse and produces less garbage.
     *
     * @param <T>         the type of the flag value
     * @param type        the class of the expected flag value type
     * @param flagKey     the key of the flag to evaluate
     * @param contextJson the evaluation context as JSON (use null or "" for empty context)
     * @return the evaluation result containing value, variant, reason, and metadata
     * @throws EvaluatorException if the evaluation fails
     */
    public synchronized <T> EvaluationResult<T> evaluateFlagBinary(Class<T> type, String flagKey, String contextJson) throws EvaluatorException {
        byte[] flagBytes = flagKey.getBytes(StandardCharsets.UTF_8);
        long flagPtr = allocFunction.apply(flagBytes.length)[0];

        // Optimization: pass null pointer (0) for empty context to skip allocation
        boolean hasContext = contextJson != null && !contextJson.isEmpty() && !contextJson.equals("{}");
        long contextPtr = 0;
        int contextLen = 0;
        if (hasContext) {
            byte[] contextBytes = contextJson.getBytes(StandardCharsets.UTF_8);
            contextPtr = allocFunction.apply(contextBytes.length)[0];
            contextLen = contextBytes.length;
            memory.write((int) contextPtr, contextBytes);
        }

        try {
            memory.write((int) flagPtr, flagBytes);

            long packedResult = evaluateBinaryFunction.apply(flagPtr, flagBytes.length, contextPtr, contextLen)[0];
            int resultPtr = (int) (packedResult >>> 32);
            int resultLen = (int) (packedResult & 0xFFFFFFFFL);

            // Read raw bytes from memory (not string)
            byte[] protoBytes = memory.readBytes(resultPtr, resultLen);

            // Parse protobuf and convert to EvaluationResult
            return parseProtoResult(type, protoBytes);
        } catch (Exception e) {
            throw new EvaluatorException("Failed to evaluate flag: " + flagKey, e);
        }
        // Note: input buffers are freed by WASM internally
    }

    /**
     * Evaluates a flag with EvaluationContext using the binary protobuf protocol.
     *
     * @param <T>     the type of the flag value
     * @param type    the class of the expected flag value type
     * @param flagKey the key of the flag to evaluate
     * @param context the evaluation context
     * @return the evaluation result
     * @throws EvaluatorException if the evaluation fails
     */
    public <T> EvaluationResult<T> evaluateFlagBinary(Class<T> type, String flagKey, EvaluationContext context) throws EvaluatorException {
        try {
            if (context == null || context.isEmpty()) {
                return evaluateFlagBinary(type, flagKey, (String) null);
            }
            ByteArrayOutputStream buffer = JSON_BUFFER.get();
            buffer.reset();
            try (JsonGenerator generator = JSON_FACTORY.createGenerator(buffer)) {
                OBJECT_MAPPER.writeValue(generator, context);
            }
            return evaluateFlagBinary(type, flagKey, buffer.toString(StandardCharsets.UTF_8.name()));
        } catch (Exception e) {
            throw new EvaluatorException("Failed to serialize context", e);
        }
    }

    /**
     * Parses a protobuf EvaluationResult and converts it to the Java type.
     */
    @SuppressWarnings("unchecked")
    private <T> EvaluationResult<T> parseProtoResult(Class<T> type, byte[] protoBytes) throws InvalidProtocolBufferException {
        EvaluationProto.EvaluationResult protoResult = EvaluationProto.EvaluationResult.parseFrom(protoBytes);

        // Convert protobuf value to Java value
        Object value = null;
        if (protoResult.hasValue()) {
            EvaluationProto.Value protoValue = protoResult.getValue();
            switch (protoValue.getKindCase()) {
                case BOOL_VALUE:
                    value = protoValue.getBoolValue();
                    break;
                case STRING_VALUE:
                    value = protoValue.getStringValue();
                    break;
                case INT_VALUE:
                    if (type == Double.class) {
                        value = (double) protoValue.getIntValue();
                    } else {
                        value = (int) protoValue.getIntValue();
                    }
                    break;
                case DOUBLE_VALUE:
                    if (type == Integer.class) {
                        value = (int) protoValue.getDoubleValue();
                    } else {
                        value = protoValue.getDoubleValue();
                    }
                    break;
                case JSON_VALUE:
                    // For complex types, parse the JSON value
                    try {
                        if (type == Value.class) {
                            value = OBJECT_MAPPER.readValue(protoValue.getJsonValue(), Value.class);
                        } else {
                            value = OBJECT_MAPPER.readValue(protoValue.getJsonValue(), type);
                        }
                    } catch (Exception e) {
                        value = protoValue.getJsonValue();
                    }
                    break;
                case KIND_NOT_SET:
                default:
                    value = null;
                    break;
            }
        }

        // Convert reason
        String reason = convertReason(protoResult.getReason());

        // Convert error code
        String errorCode = null;
        if (protoResult.getErrorCode() != EvaluationProto.ErrorCode.ERROR_CODE_UNSPECIFIED) {
            errorCode = convertErrorCode(protoResult.getErrorCode());
        }

        // Parse metadata if present
        ImmutableMetadata metadata = null;
        if (!protoResult.getMetadataJson().isEmpty()) {
            try {
                metadata = OBJECT_MAPPER.readValue(protoResult.getMetadataJson(), ImmutableMetadata.class);
            } catch (Exception e) {
                // Ignore metadata parsing errors
            }
        }

        EvaluationResult<T> result = new EvaluationResult<>();
        result.setValue((T) value);
        result.setVariant(protoResult.getVariant().isEmpty() ? null : protoResult.getVariant());
        result.setReason(reason);
        result.setErrorCode(errorCode);
        result.setErrorMessage(protoResult.getErrorMessage().isEmpty() ? null : protoResult.getErrorMessage());
        result.setFlagMetadata(metadata);
        return result;
    }

    private String convertReason(EvaluationProto.Reason reason) {
        switch (reason) {
            case REASON_STATIC: return "STATIC";
            case REASON_DEFAULT: return "DEFAULT";
            case REASON_TARGETING_MATCH: return "TARGETING_MATCH";
            case REASON_DISABLED: return "DISABLED";
            case REASON_ERROR: return "ERROR";
            case REASON_FLAG_NOT_FOUND: return "FLAG_NOT_FOUND";
            case REASON_FALLBACK: return "FALLBACK";
            default: return "UNKNOWN";
        }
    }

    private String convertErrorCode(EvaluationProto.ErrorCode errorCode) {
        switch (errorCode) {
            case ERROR_CODE_FLAG_NOT_FOUND: return "FLAG_NOT_FOUND";
            case ERROR_CODE_PARSE_ERROR: return "PARSE_ERROR";
            case ERROR_CODE_TYPE_MISMATCH: return "TYPE_MISMATCH";
            case ERROR_CODE_GENERAL: return "GENERAL";
            default: return null;
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
