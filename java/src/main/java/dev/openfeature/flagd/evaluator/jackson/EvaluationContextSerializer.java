package dev.openfeature.flagd.evaluator.jackson;

import com.fasterxml.jackson.core.JsonGenerator;
import com.fasterxml.jackson.databind.JsonSerializer;
import com.fasterxml.jackson.databind.SerializerProvider;
import dev.openfeature.sdk.EvaluationContext;
import dev.openfeature.sdk.Structure;
import dev.openfeature.sdk.Value;

import java.io.ByteArrayOutputStream;
import java.io.IOException;
import java.util.ArrayList;
import java.util.HashMap;
import java.util.List;
import java.util.Map;
import java.util.Set;

/**
 * Custom serializer for EvaluationContext (including LayeredEvaluationContext and MutableContext).
 *
 * <p>This serializer iterates through the context's keys and writes each key-value pair
 * to the JSON output. It works with any implementation of EvaluationContext and properly
 * handles nested structures, lists, and all OpenFeature Value types.
 */
public class EvaluationContextSerializer extends JsonSerializer<EvaluationContext> {

    @Override
    public void serialize(EvaluationContext ctx, JsonGenerator gen, SerializerProvider serializers)
            throws IOException {
        gen.writeStartObject();

        // Use the keySet and getValue to stream the entries
        for (String key : ctx.keySet()) {
            Value value = ctx.getValue(key);
            // Extract the raw value from the Value wrapper and serialize it
            gen.writeFieldName(key);
            Object rawValue = extractRawValue(value);
            serializers.defaultSerializeValue(rawValue, gen);
        }

        gen.writeEndObject();
    }

    // ThreadLocal buffer for filtered serialization
    private static final ThreadLocal<ByteArrayOutputStream> FILTERED_BUFFER =
        ThreadLocal.withInitial(() -> new ByteArrayOutputStream(2048));

    private static final com.fasterxml.jackson.core.JsonFactory SHARED_JSON_FACTORY =
        new com.fasterxml.jackson.core.JsonFactory();

    /**
     * Serializes a filtered subset of the evaluation context with $flagd enrichment.
     *
     * <p>Only includes the specified required keys from the context, plus the
     * {@code $flagd} enrichment object and {@code targetingKey}. This dramatically
     * reduces the serialized size for large contexts where the targeting rule only
     * references a few fields.
     *
     * @param ctx          the evaluation context to filter and serialize
     * @param requiredKeys the set of context keys that the targeting rule references
     * @param flagKey      the flag key (for $flagd.flagKey enrichment)
     * @return the filtered, enriched JSON string
     * @throws IOException if serialization fails
     */
    public static String serializeFiltered(
            EvaluationContext ctx,
            Set<String> requiredKeys,
            String flagKey) throws IOException {
        ByteArrayOutputStream buffer = FILTERED_BUFFER.get();
        buffer.reset();

        try (JsonGenerator gen = SHARED_JSON_FACTORY.createGenerator(buffer)) {
            gen.writeStartObject();

            // Write only the required keys from the context
            for (String key : requiredKeys) {
                // Skip $flagd — we add it ourselves below
                if (key.startsWith("$flagd")) {
                    continue;
                }
                Value value = ctx.getValue(key);
                if (value != null) {
                    gen.writeFieldName(key);
                    writeValue(gen, value);
                } else if ("targetingKey".equals(key)) {
                    // targetingKey defaults to empty string if not in context
                    String tk = ctx.getTargetingKey();
                    gen.writeStringField("targetingKey", tk != null ? tk : "");
                }
            }

            // Ensure targetingKey is always present
            if (!requiredKeys.contains("targetingKey")) {
                String tk = ctx.getTargetingKey();
                gen.writeStringField("targetingKey", tk != null ? tk : "");
            }

            // Add $flagd enrichment
            gen.writeObjectFieldStart("$flagd");
            gen.writeStringField("flagKey", flagKey);
            gen.writeNumberField("timestamp", System.currentTimeMillis() / 1000);
            gen.writeEndObject();

            gen.writeEndObject();
        }

        return buffer.toString("UTF-8");
    }

    /**
     * Writes an OpenFeature Value to a JsonGenerator.
     */
    private static void writeValue(JsonGenerator gen, Value value) throws IOException {
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
            Structure structure = value.asStructure();
            for (String key : structure.keySet()) {
                gen.writeFieldName(key);
                writeValue(gen, structure.getValue(key));
            }
            gen.writeEndObject();
        } else {
            gen.writeNull();
        }
    }

    /**
     * Extracts the raw Java object from an OpenFeature Value wrapper.
     * Recursively handles nested structures and lists.
     */
    private Object extractRawValue(Value value) {
        if (value == null) {
            return null;
        }

        if (value.isBoolean()) {
            return value.asBoolean();
        } else if (value.isNumber()) {
            // Return as double - Jackson will serialize appropriately (42.0 -> 42, 3.14 -> 3.14)
            return value.asDouble();
        } else if (value.isString()) {
            return value.asString();
        } else if (value.isList()) {
            // Recursively extract values from list
            List<Value> valueList = value.asList();
            List<Object> result = new ArrayList<>(valueList.size());
            for (Value item : valueList) {
                result.add(extractRawValue(item));
            }
            return result;
        } else if (value.isStructure()) {
            // Recursively extract values from structure
            Structure structure = value.asStructure();
            Map<String, Object> result = new HashMap<>();
            for (String key : structure.keySet()) {
                Value nestedValue = structure.getValue(key);
                result.put(key, extractRawValue(nestedValue));
            }
            return result;
        } else {
            // Fallback: return null
            return null;
        }
    }
}
