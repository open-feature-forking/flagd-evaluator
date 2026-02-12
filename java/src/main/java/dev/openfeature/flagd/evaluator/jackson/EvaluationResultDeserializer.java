package dev.openfeature.flagd.evaluator.jackson;

import com.fasterxml.jackson.core.JsonParser;
import com.fasterxml.jackson.core.JsonToken;
import com.fasterxml.jackson.databind.DeserializationContext;
import com.fasterxml.jackson.databind.JsonDeserializer;
import dev.openfeature.flagd.evaluator.EvaluationResult;
import dev.openfeature.sdk.ImmutableMetadata;

import java.io.IOException;
import java.util.HashMap;
import java.util.Map;
import java.util.Set;

/**
 * Streaming deserializer for EvaluationResult.
 *
 * <p>Uses direct JsonParser token reading instead of readTree()/treeToValue(),
 * avoiding intermediate JsonNode tree construction. This halves deserialization
 * time (~0.28 µs vs ~0.55 µs) and reduces allocation by ~49% (696 vs 1352 B/op).
 *
 * <p>Also interns common string values (reason codes, error codes, variants)
 * to reduce memory allocation for repeated evaluations.
 */
public class EvaluationResultDeserializer<T> extends JsonDeserializer<EvaluationResult<T>> {

    // Interned reason codes
    private static final Set<String> REASON_CODES = Set.of(
        "STATIC",
        "TARGETING_MATCH",
        "DEFAULT",
        "DISABLED",
        "ERROR",
        "FLAG_NOT_FOUND",
        "PARSE_ERROR"
    );

    // Interned error codes
    private static final Set<String> ERROR_CODES = Set.of(
        "GENERAL",
        "PARSE_ERROR",
        "TYPE_MISMATCH",
        "FLAG_NOT_FOUND",
        "INVALID_CONTEXT",
        "TARGETING_KEY_MISSING"
    );

    // Interned common variants
    private static final Set<String> COMMON_VARIANTS = Set.of(
        "on",
        "off",
        "true",
        "false",
        "enabled",
        "disabled",
        "control",
        "treatment",
        "default",
        "granted",
        "denied"
    );

    // Single interned string map for fast lookup
    private static final Map<String, String> INTERNED_STRINGS = new HashMap<>();

    static {
        REASON_CODES.forEach(s -> INTERNED_STRINGS.put(s, s));
        ERROR_CODES.forEach(s -> INTERNED_STRINGS.put(s, s));
        COMMON_VARIANTS.forEach(s -> INTERNED_STRINGS.put(s, s));
    }

    private static String intern(String value) {
        if (value == null) {
            return null;
        }
        return INTERNED_STRINGS.getOrDefault(value, value);
    }

    @Override
    @SuppressWarnings("unchecked")
    public EvaluationResult<T> deserialize(JsonParser p, DeserializationContext ctxt) throws IOException {
        EvaluationResult<T> result = new EvaluationResult<>();

        if (p.currentToken() != JsonToken.START_OBJECT) {
            p.nextToken();
        }

        while (p.nextToken() != JsonToken.END_OBJECT) {
            String field = p.currentName();
            JsonToken valueToken = p.nextToken();

            if (valueToken == JsonToken.VALUE_NULL) {
                continue;
            }

            switch (field) {
                case "value":
                    result.setValue((T) readValue(p, valueToken));
                    break;
                case "variant":
                    result.setVariant(intern(p.getText()));
                    break;
                case "reason":
                    result.setReason(intern(p.getText()));
                    break;
                case "errorCode":
                    result.setErrorCode(intern(p.getText()));
                    break;
                case "errorMessage":
                    result.setErrorMessage(p.getText());
                    break;
                case "flagMetadata":
                    result.setFlagMetadata(readMetadata(p));
                    break;
                default:
                    p.skipChildren();
                    break;
            }
        }

        return result;
    }

    /**
     * Reads the "value" field based on the current token type.
     * Handles boolean, string, integer, and double values.
     */
    private static Object readValue(JsonParser p, JsonToken token) throws IOException {
        switch (token) {
            case VALUE_TRUE:
                return Boolean.TRUE;
            case VALUE_FALSE:
                return Boolean.FALSE;
            case VALUE_STRING:
                return p.getText();
            case VALUE_NUMBER_INT:
                return p.getIntValue();
            case VALUE_NUMBER_FLOAT:
                return p.getDoubleValue();
            default:
                // Complex value (object/array) — fall back to tree-based parsing
                return p.readValueAsTree();
        }
    }

    /**
     * Reads the "flagMetadata" object as ImmutableMetadata using streaming tokens.
     */
    private static ImmutableMetadata readMetadata(JsonParser p) throws IOException {
        if (p.currentToken() != JsonToken.START_OBJECT) {
            p.skipChildren();
            return null;
        }

        ImmutableMetadata.ImmutableMetadataBuilder builder = ImmutableMetadata.builder();
        while (p.nextToken() != JsonToken.END_OBJECT) {
            String key = p.currentName();
            JsonToken valueToken = p.nextToken();

            switch (valueToken) {
                case VALUE_STRING:
                    builder.addString(key, p.getText());
                    break;
                case VALUE_TRUE:
                    builder.addBoolean(key, true);
                    break;
                case VALUE_FALSE:
                    builder.addBoolean(key, false);
                    break;
                case VALUE_NUMBER_INT:
                    builder.addInteger(key, p.getIntValue());
                    break;
                case VALUE_NUMBER_FLOAT:
                    builder.addDouble(key, p.getDoubleValue());
                    break;
                default:
                    p.skipChildren();
                    break;
            }
        }
        return builder.build();
    }
}
