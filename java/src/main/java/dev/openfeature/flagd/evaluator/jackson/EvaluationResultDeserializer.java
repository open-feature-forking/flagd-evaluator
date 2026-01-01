package dev.openfeature.flagd.evaluator.jackson;

import com.fasterxml.jackson.core.JsonParser;
import com.fasterxml.jackson.databind.DeserializationContext;
import com.fasterxml.jackson.databind.JsonDeserializer;
import com.fasterxml.jackson.databind.JsonNode;
import dev.openfeature.flagd.evaluator.EvaluationResult;
import dev.openfeature.sdk.ImmutableMetadata;

import java.io.IOException;
import java.util.HashMap;
import java.util.Map;
import java.util.Set;

/**
 * Custom deserializer for EvaluationResult that interns common string values.
 *
 * <p>This reduces memory allocation by reusing String instances for common
 * values like reason codes, error codes, and common variants.
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
        "default"
    );

    // Single interned string map for fast lookup
    private static final Map<String, String> INTERNED_STRINGS = new HashMap<>();

    static {
        // Pre-populate interned strings map
        REASON_CODES.forEach(s -> INTERNED_STRINGS.put(s, s));
        ERROR_CODES.forEach(s -> INTERNED_STRINGS.put(s, s));
        COMMON_VARIANTS.forEach(s -> INTERNED_STRINGS.put(s, s));
    }

    /**
     * Interns a string if it matches a known common value.
     *
     * @param value the string to potentially intern
     * @return the interned string if found, otherwise the original string
     */
    private static String intern(String value) {
        if (value == null) {
            return null;
        }
        return INTERNED_STRINGS.getOrDefault(value, value);
    }

    @Override
    public EvaluationResult<T> deserialize(JsonParser p, DeserializationContext ctxt) throws IOException {
        JsonNode node = p.getCodec().readTree(p);

        EvaluationResult<T> result = new EvaluationResult<>();

        // Deserialize value (type-specific, cannot intern)
        if (node.has("value") && !node.get("value").isNull()) {
            JsonNode valueNode = node.get("value");
            @SuppressWarnings("unchecked")
            T value = (T) p.getCodec().treeToValue(valueNode, Object.class);
            result.setValue(value);
        }

        // Deserialize and intern variant
        if (node.has("variant") && !node.get("variant").isNull()) {
            result.setVariant(intern(node.get("variant").asText()));
        }

        // Deserialize and intern reason
        if (node.has("reason") && !node.get("reason").isNull()) {
            result.setReason(intern(node.get("reason").asText()));
        }

        // Deserialize and intern error code
        if (node.has("errorCode") && !node.get("errorCode").isNull()) {
            result.setErrorCode(intern(node.get("errorCode").asText()));
        }

        // Deserialize error message (don't intern - usually unique)
        if (node.has("errorMessage") && !node.get("errorMessage").isNull()) {
            result.setErrorMessage(node.get("errorMessage").asText());
        }

        // Deserialize flag metadata as ImmutableMetadata
        if (node.has("flagMetadata") && !node.get("flagMetadata").isNull()) {
            ImmutableMetadata metadata =
                p.getCodec().treeToValue(node.get("flagMetadata"), ImmutableMetadata.class);
            result.setFlagMetadata(metadata);
        }

        return result;
    }
}
