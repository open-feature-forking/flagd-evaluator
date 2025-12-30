package dev.openfeature.flagd.evaluator.jackson;

import com.fasterxml.jackson.core.JsonGenerator;
import com.fasterxml.jackson.databind.JsonSerializer;
import com.fasterxml.jackson.databind.SerializerProvider;
import dev.openfeature.sdk.EvaluationContext;
import dev.openfeature.sdk.LayeredEvaluationContext;
import dev.openfeature.sdk.Value;

import java.io.IOException;

/**
 * Custom serializer for EvaluationContext (including LayeredEvaluationContext and MutableContext).
 *
 * <p>This serializer iterates through the context's keys and writes each key-value pair
 * to the JSON output. It works with any implementation of EvaluationContext.
 */
public class LayeredEvalContextSerializer extends JsonSerializer<EvaluationContext> {

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
     */
    private Object extractRawValue(Value value) {
        if (value.isBoolean()) {
            return value.asBoolean();
        } else if (value.isNumber()) {
            // Try integer first, then double
            try {
                return value.asInteger();
            } catch (Exception e) {
                return value.asDouble();
            }
        } else if (value.isString()) {
            return value.asString();
        } else if (value.isList()) {
            return value.asList();
        } else if (value.isStructure()) {
            return value.asStructure();
        } else {
            // Fallback: return null or the value itself
            return null;
        }
    }
}
