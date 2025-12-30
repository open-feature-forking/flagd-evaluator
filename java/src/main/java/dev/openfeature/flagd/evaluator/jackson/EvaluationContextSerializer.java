package dev.openfeature.flagd.evaluator.jackson;

import com.fasterxml.jackson.core.JsonGenerator;
import com.fasterxml.jackson.databind.JsonSerializer;
import com.fasterxml.jackson.databind.SerializerProvider;
import dev.openfeature.sdk.EvaluationContext;
import dev.openfeature.sdk.Structure;
import dev.openfeature.sdk.Value;

import java.io.IOException;
import java.util.ArrayList;
import java.util.HashMap;
import java.util.List;
import java.util.Map;

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
