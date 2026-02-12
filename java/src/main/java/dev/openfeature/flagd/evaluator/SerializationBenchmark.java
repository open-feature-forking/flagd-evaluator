package dev.openfeature.flagd.evaluator;

import com.fasterxml.jackson.core.JsonFactory;
import com.fasterxml.jackson.core.JsonGenerator;
import com.fasterxml.jackson.core.JsonParser;
import com.fasterxml.jackson.core.JsonToken;
import com.fasterxml.jackson.databind.JavaType;
import dev.openfeature.flagd.evaluator.jackson.EvaluationContextSerializer;
import dev.openfeature.sdk.EvaluationContext;
import dev.openfeature.sdk.ImmutableContext;
import dev.openfeature.sdk.LayeredEvaluationContext;
import dev.openfeature.sdk.Value;
import org.openjdk.jmh.annotations.*;
import org.openjdk.jmh.infra.Blackhole;

import java.io.ByteArrayOutputStream;
import java.nio.charset.StandardCharsets;
import java.util.HashMap;
import java.util.Map;
import java.util.Set;
import java.util.concurrent.TimeUnit;

/**
 * JMH benchmarks isolating serialization overhead in the WASM evaluation pipeline.
 *
 * <p>Breaks down the ~12.5 µs targeting evaluation into:
 * <ul>
 *   <li>S1: Context serialization (EvaluationContext → JSON String)</li>
 *   <li>S2: Result deserialization (JSON String → EvaluationResult)</li>
 *   <li>S3: String→byte[] conversion (intermediate copy)</li>
 *   <li>S4: Full evaluateFlag(EvaluationContext) — production path</li>
 *   <li>S5: evaluateFlag(String) — WASM + result deser, no context ser</li>
 *   <li>S6: Direct byte[] context serialization (skip String intermediary)</li>
 *   <li>S7: Streaming result parser (skip readTree/treeToValue)</li>
 * </ul>
 *
 * <p><b>Running:</b>
 * <pre>
 * ./mvnw clean package -DskipTests
 * java -jar target/benchmarks.jar SerializationBenchmark -prof gc
 * </pre>
 */
@BenchmarkMode({Mode.AverageTime})
@OutputTimeUnit(TimeUnit.MICROSECONDS)
@Fork(1)
@Warmup(iterations = 3, time = 2)
@Measurement(iterations = 5, time = 3)
@Threads(1)
public class SerializationBenchmark {

    private static final String FLAG_CONFIG = "{\n" +
        "  \"flags\": {\n" +
        "    \"targeted-access\": {\n" +
        "      \"state\": \"ENABLED\",\n" +
        "      \"defaultVariant\": \"denied\",\n" +
        "      \"variants\": {\n" +
        "        \"denied\": false,\n" +
        "        \"granted\": true\n" +
        "      },\n" +
        "      \"targeting\": {\n" +
        "        \"if\": [\n" +
        "          {\n" +
        "            \"and\": [\n" +
        "              { \"==\": [{ \"var\": \"role\" }, \"admin\"] },\n" +
        "              { \"in\": [{ \"var\": \"tier\" }, [\"premium\", \"enterprise\"]] }\n" +
        "            ]\n" +
        "          },\n" +
        "          \"granted\",\n" +
        "          null\n" +
        "        ]\n" +
        "      }\n" +
        "    }\n" +
        "  }\n" +
        "}";

    // Typical result JSON from WASM evaluation
    private static final String RESULT_JSON =
        "{\"variant\":\"granted\",\"value\":true,\"reason\":\"TARGETING_MATCH\"}";

    private static final JsonFactory JSON_FACTORY = new JsonFactory();

    @State(Scope.Benchmark)
    public static class SerState {
        FlagEvaluator evaluator;
        EvaluationContext smallContext;
        EvaluationContext largeContext;
        Set<String> requiredKeys;
        String preSerializedContextJson;
        JavaType booleanResultType;

        // ThreadLocal buffer for direct byte[] serialization
        final ThreadLocal<ByteArrayOutputStream> directBuffer =
            ThreadLocal.withInitial(() -> new ByteArrayOutputStream(2048));

        @Setup(Level.Trial)
        public void setup() {
            try {
                evaluator = new FlagEvaluator(FlagEvaluator.ValidationMode.PERMISSIVE);
                evaluator.updateState(FLAG_CONFIG);

                // API-level context
                Map<String, Value> apiAttrs = new HashMap<>();
                apiAttrs.put("environment", new Value("production"));
                apiAttrs.put("service", new Value("checkout"));
                ImmutableContext apiCtx = new ImmutableContext(apiAttrs);

                // Small invocation context (4 attrs, matching targeting rule)
                Map<String, Value> smallAttrs = new HashMap<>();
                smallAttrs.put("tier", new Value("premium"));
                smallAttrs.put("role", new Value("admin"));
                smallAttrs.put("region", new Value("us-east"));
                smallAttrs.put("score", new Value(85));
                smallContext = new LayeredEvaluationContext(apiCtx, ImmutableContext.EMPTY,
                    new ImmutableContext("user-123", smallAttrs), ImmutableContext.EMPTY);

                // Large invocation context (100+ attrs)
                Map<String, Value> largeAttrs = new HashMap<>(smallAttrs);
                for (int i = 0; i < 100; i++) {
                    switch (i % 4) {
                        case 0: largeAttrs.put("attr_" + i, new Value("value-" + i)); break;
                        case 1: largeAttrs.put("attr_" + i, new Value(i * 7)); break;
                        case 2: largeAttrs.put("attr_" + i, new Value(i % 2 == 0)); break;
                        case 3: largeAttrs.put("attr_" + i, new Value(i * 1.5)); break;
                    }
                }
                largeContext = new LayeredEvaluationContext(apiCtx, ImmutableContext.EMPTY,
                    new ImmutableContext("user-123", largeAttrs), ImmutableContext.EMPTY);

                // Required keys for targeted-access flag (extracted at updateState)
                requiredKeys = Set.of("role", "tier", "targetingKey");

                // Pre-serialized context for S5
                preSerializedContextJson = EvaluationContextSerializer.serializeFiltered(
                    smallContext, requiredKeys, "targeted-access");

                booleanResultType = FlagEvaluator.OBJECT_MAPPER.getTypeFactory()
                    .constructParametricType(EvaluationResult.class, Boolean.class);
            } catch (Exception e) {
                throw new RuntimeException("Setup failed", e);
            }
        }

        @TearDown(Level.Trial)
        public void tearDown() {
            if (evaluator != null) {
                evaluator.close();
            }
        }
    }

    // ========================================================================
    // S1: Context serialization — EvaluationContext → JSON String
    // ========================================================================

    @Benchmark
    public void S1_serializeContext_small(SerState state, Blackhole bh) throws Exception {
        String json = EvaluationContextSerializer.serializeFiltered(
            state.smallContext, state.requiredKeys, "targeted-access");
        bh.consume(json);
    }

    @Benchmark
    public void S1_serializeContext_large(SerState state, Blackhole bh) throws Exception {
        String json = EvaluationContextSerializer.serializeFiltered(
            state.largeContext, state.requiredKeys, "targeted-access");
        bh.consume(json);
    }

    // ========================================================================
    // S2: Result deserialization — JSON String → EvaluationResult (Jackson readValue)
    // ========================================================================

    @Benchmark
    public void S2_deserializeResult_jackson(SerState state, Blackhole bh) throws Exception {
        EvaluationResult<Boolean> result = FlagEvaluator.OBJECT_MAPPER.readValue(
            RESULT_JSON, state.booleanResultType);
        bh.consume(result);
    }

    // ========================================================================
    // S3: String → byte[] conversion (the intermediate copy)
    // ========================================================================

    @Benchmark
    public void S3_stringToBytes(SerState state, Blackhole bh) {
        byte[] bytes = state.preSerializedContextJson.getBytes(StandardCharsets.UTF_8);
        bh.consume(bytes);
    }

    // ========================================================================
    // S4: Full production path — evaluateFlag(EvaluationContext)
    // ========================================================================

    @Benchmark
    public void S4_full_evaluateFlag_small(SerState state, Blackhole bh) throws Exception {
        EvaluationResult<Boolean> result = state.evaluator.evaluateFlag(
            Boolean.class, "targeted-access", state.smallContext);
        bh.consume(result.getValue());
        bh.consume(result.getVariant());
    }

    @Benchmark
    public void S4_full_evaluateFlag_large(SerState state, Blackhole bh) throws Exception {
        EvaluationResult<Boolean> result = state.evaluator.evaluateFlag(
            Boolean.class, "targeted-access", state.largeContext);
        bh.consume(result.getValue());
        bh.consume(result.getVariant());
    }

    // ========================================================================
    // S5: WASM + result deser only — evaluateFlag(String) with pre-serialized JSON
    // ========================================================================

    @Benchmark
    public void S5_wasmPlusDeser_preSerializedJson(SerState state, Blackhole bh) throws Exception {
        EvaluationResult<Boolean> result = state.evaluator.evaluateFlag(
            Boolean.class, "targeted-access", state.preSerializedContextJson);
        bh.consume(result.getValue());
        bh.consume(result.getVariant());
    }

    // ========================================================================
    // S6: Direct byte[] context serialization (skip String intermediary)
    // ========================================================================

    @Benchmark
    public void S6_serializeContextToBytes_small(SerState state, Blackhole bh) throws Exception {
        ByteArrayOutputStream buffer = state.directBuffer.get();
        buffer.reset();
        serializeFilteredToBytes(buffer, state.smallContext, state.requiredKeys, "targeted-access");
        byte[] bytes = buffer.toByteArray();
        bh.consume(bytes);
    }

    @Benchmark
    public void S6_serializeContextToBytes_large(SerState state, Blackhole bh) throws Exception {
        ByteArrayOutputStream buffer = state.directBuffer.get();
        buffer.reset();
        serializeFilteredToBytes(buffer, state.largeContext, state.requiredKeys, "targeted-access");
        byte[] bytes = buffer.toByteArray();
        bh.consume(bytes);
    }

    // ========================================================================
    // S7: Streaming result parser (skip readTree/treeToValue)
    // ========================================================================

    @Benchmark
    public void S7_deserializeResult_streaming(SerState state, Blackhole bh) throws Exception {
        EvaluationResult<Boolean> result = deserializeResultStreaming(RESULT_JSON);
        bh.consume(result);
    }

    // ========================================================================
    // Alternative implementations
    // ========================================================================

    /**
     * Writes filtered context directly to a byte stream, skipping String intermediary.
     * Same logic as EvaluationContextSerializer.serializeFiltered but outputs bytes directly.
     */
    private static void serializeFilteredToBytes(
            ByteArrayOutputStream buffer,
            EvaluationContext ctx,
            Set<String> requiredKeys,
            String flagKey) throws Exception {
        try (JsonGenerator gen = JSON_FACTORY.createGenerator(buffer)) {
            gen.writeStartObject();

            for (String key : requiredKeys) {
                if (key.startsWith("$flagd")) continue;
                Value value = ctx.getValue(key);
                if (value != null) {
                    gen.writeFieldName(key);
                    writeValue(gen, value);
                } else if ("targetingKey".equals(key)) {
                    String tk = ctx.getTargetingKey();
                    gen.writeStringField("targetingKey", tk != null ? tk : "");
                }
            }

            if (!requiredKeys.contains("targetingKey")) {
                String tk = ctx.getTargetingKey();
                gen.writeStringField("targetingKey", tk != null ? tk : "");
            }

            gen.writeObjectFieldStart("$flagd");
            gen.writeStringField("flagKey", flagKey);
            gen.writeNumberField("timestamp", System.currentTimeMillis() / 1000);
            gen.writeEndObject();

            gen.writeEndObject();
        }
    }

    /**
     * Streaming result parser — reads JSON tokens directly without building a tree.
     * Avoids readTree() + treeToValue() overhead.
     */
    @SuppressWarnings("unchecked")
    private static <T> EvaluationResult<T> deserializeResultStreaming(String json) throws Exception {
        EvaluationResult<T> result = new EvaluationResult<>();
        try (JsonParser p = JSON_FACTORY.createParser(json)) {
            if (p.nextToken() != JsonToken.START_OBJECT) {
                throw new IllegalStateException("Expected START_OBJECT");
            }
            while (p.nextToken() != JsonToken.END_OBJECT) {
                String field = p.currentName();
                p.nextToken(); // move to value
                switch (field) {
                    case "value":
                        if (p.currentToken() == JsonToken.VALUE_TRUE) {
                            result.setValue((T) Boolean.TRUE);
                        } else if (p.currentToken() == JsonToken.VALUE_FALSE) {
                            result.setValue((T) Boolean.FALSE);
                        } else if (p.currentToken() == JsonToken.VALUE_STRING) {
                            result.setValue((T) p.getText());
                        } else if (p.currentToken() == JsonToken.VALUE_NUMBER_INT) {
                            result.setValue((T) Integer.valueOf(p.getIntValue()));
                        } else if (p.currentToken() == JsonToken.VALUE_NUMBER_FLOAT) {
                            result.setValue((T) Double.valueOf(p.getDoubleValue()));
                        }
                        break;
                    case "variant":
                        result.setVariant(p.getText());
                        break;
                    case "reason":
                        result.setReason(p.getText());
                        break;
                    case "errorCode":
                        result.setErrorCode(p.getText());
                        break;
                    case "errorMessage":
                        result.setErrorMessage(p.getText());
                        break;
                    case "flagMetadata":
                        p.skipChildren(); // skip for now
                        break;
                    default:
                        p.skipChildren();
                        break;
                }
            }
        }
        return result;
    }

    private static void writeValue(JsonGenerator gen, Value value) throws Exception {
        if (value == null || value.isNull()) {
            gen.writeNull();
        } else if (value.isBoolean()) {
            gen.writeBoolean(value.asBoolean());
        } else if (value.isNumber()) {
            double d = value.asDouble();
            if (d == Math.floor(d) && !Double.isInfinite(d)) {
                gen.writeNumber((long) d);
            } else {
                gen.writeNumber(d);
            }
        } else if (value.isString()) {
            gen.writeString(value.asString());
        } else if (value.isList()) {
            gen.writeStartArray();
            for (Value item : value.asList()) {
                writeValue(gen, item);
            }
            gen.writeEndArray();
        } else if (value.isStructure()) {
            gen.writeStartObject();
            dev.openfeature.sdk.Structure structure = value.asStructure();
            for (String key : structure.keySet()) {
                gen.writeFieldName(key);
                writeValue(gen, structure.getValue(key));
            }
            gen.writeEndObject();
        } else {
            gen.writeNull();
        }
    }

    public static void main(String[] args) throws Exception {
        org.openjdk.jmh.Main.main(args);
    }
}
