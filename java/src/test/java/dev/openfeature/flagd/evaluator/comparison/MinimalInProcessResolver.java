package dev.openfeature.flagd.evaluator.comparison;

import com.fasterxml.jackson.databind.ObjectMapper;
import dev.openfeature.sdk.*;
import dev.openfeature.sdk.exceptions.GeneralError;
import dev.openfeature.sdk.exceptions.ParseError;
import dev.openfeature.sdk.exceptions.TypeMismatchError;

import java.util.HashMap;
import java.util.Map;

/**
 * Minimal in-process resolver extracted from the old java-sdk-contrib implementation.
 *
 * <p>This resolver contains ONLY the core evaluation logic without any of the infrastructure:
 * - No FlagStore/Storage layer
 * - No QueueSource/Sync connectors
 * - No event handling threads
 * - No state management
 *
 * <p>Used for comparison testing to validate that the new WASM-based evaluator
 * produces the same results as the old JsonLogic-based approach.
 *
 * <p>Source: dev.openfeature.contrib.providers.flagd.resolver.process.InProcessResolver
 */
public class MinimalInProcessResolver {

    private static final String EMPTY_TARGETING_STRING = "{}";
    private static final ObjectMapper MAPPER = new ObjectMapper();

    private final Map<String, FeatureFlag> flags = new HashMap<>();
    private final SimpleJsonLogicOperator operator = new SimpleJsonLogicOperator();

    /**
     * Manually add a flag to the resolver (replaces FlagStore).
     */
    public void setFlag(String key, FeatureFlag flag) {
        flags.put(key, flag);
    }

    /**
     * Load flags from JSON configuration (similar to updateState).
     */
    public void loadFlags(String jsonConfig) throws Exception {
        @SuppressWarnings("unchecked")
        Map<String, Object> config = MAPPER.readValue(jsonConfig, Map.class);

        @SuppressWarnings("unchecked")
        Map<String, Object> flagsMap = (Map<String, Object>) config.get("flags");

        if (flagsMap != null) {
            for (Map.Entry<String, Object> entry : flagsMap.entrySet()) {
                String flagKey = entry.getKey();
                @SuppressWarnings("unchecked")
                Map<String, Object> flagData = (Map<String, Object>) entry.getValue();

                @SuppressWarnings("unchecked")
                Map<String, Object> variants = (Map<String, Object>) flagData.get("variants");

                // Handle targeting - if not present or null, use empty object
                Object targeting = flagData.get("targeting");
                String targetingJson = (targeting == null) ? null : MAPPER.writeValueAsString(targeting);

                FeatureFlag flag = new FeatureFlag(
                    (String) flagData.get("state"),
                    (String) flagData.get("defaultVariant"),
                    variants,
                    targetingJson
                );

                setFlag(flagKey, flag);
            }
        }
    }

    /**
     * Resolve a boolean flag.
     */
    public ProviderEvaluation<Boolean> booleanEvaluation(String key, Boolean defaultValue, EvaluationContext ctx) {
        return resolve(Boolean.class, key, ctx);
    }

    /**
     * Resolve a string flag.
     */
    public ProviderEvaluation<String> stringEvaluation(String key, String defaultValue, EvaluationContext ctx) {
        return resolve(String.class, key, ctx);
    }

    /**
     * Resolve a double flag.
     */
    public ProviderEvaluation<Double> doubleEvaluation(String key, Double defaultValue, EvaluationContext ctx) {
        return resolve(Double.class, key, ctx);
    }

    /**
     * Resolve an integer flag.
     */
    public ProviderEvaluation<Integer> integerEvaluation(String key, Integer defaultValue, EvaluationContext ctx) {
        return resolve(Integer.class, key, ctx);
    }

    /**
     * Resolve an object flag.
     */
    public ProviderEvaluation<Value> objectEvaluation(String key, Value defaultValue, EvaluationContext ctx) {
        final ProviderEvaluation<Object> evaluation = resolve(Object.class, key, ctx);

        return ProviderEvaluation.<Value>builder()
                .value(Value.objectToValue(evaluation.getValue()))
                .variant(evaluation.getVariant())
                .reason(evaluation.getReason())
                .errorCode(evaluation.getErrorCode())
                .errorMessage(evaluation.getErrorMessage())
                .flagMetadata(evaluation.getFlagMetadata())
                .build();
    }

    /**
     * Core resolution logic extracted from InProcessResolver (lines 175-255).
     * Uses simple HashMap instead of FlagStore.
     */
    private <T> ProviderEvaluation<T> resolve(Class<T> type, String key, EvaluationContext ctx) {
        final FeatureFlag flag = flags.get(key);

        // missing flag
        if (flag == null) {
            return ProviderEvaluation.<T>builder()
                    .errorMessage("flag: " + key + " not found")
                    .errorCode(ErrorCode.FLAG_NOT_FOUND)
                    .flagMetadata(ImmutableMetadata.builder().build())
                    .build();
        }

        // state check
        if ("DISABLED".equals(flag.getState())) {
            return ProviderEvaluation.<T>builder()
                    .errorMessage("flag: " + key + " is disabled")
                    .errorCode(ErrorCode.FLAG_NOT_FOUND)
                    .flagMetadata(ImmutableMetadata.builder().build())
                    .build();
        }

        final String resolvedVariant;
        final String reason;

        if (EMPTY_TARGETING_STRING.equals(flag.getTargeting())) {
            resolvedVariant = flag.getDefaultVariant();
            reason = Reason.STATIC.toString();
        } else {
            try {
                final Object jsonResolved = operator.apply(key, flag.getTargeting(), ctx);
                if (jsonResolved == null) {
                    resolvedVariant = flag.getDefaultVariant();
                    reason = Reason.DEFAULT.toString();
                } else {
                    resolvedVariant = jsonResolved.toString(); // convert to string to support shorthand
                    reason = Reason.TARGETING_MATCH.toString();
                }
            } catch (Exception e) {
                String message = String.format("error evaluating targeting rule for flag %s", key);
                throw new ParseError(message);
            }
        }

        // check variant existence
        Object value = flag.getVariants().get(resolvedVariant);
        if (value == null) {
            String message = String.format("variant %s not found in flag with key %s", resolvedVariant, key);
            throw new GeneralError(message);
        }

        if (value instanceof Integer && type == Double.class) {
            // if this is an integer and we are trying to resolve a double, convert
            value = ((Integer) value).doubleValue();
        } else if (value instanceof Double && type == Integer.class) {
            // if this is a double and we are trying to resolve an integer, convert
            value = ((Double) value).intValue();
        }

        if (!type.isAssignableFrom(value.getClass())) {
            String message = "returning default variant for flagKey: %s, type not valid";
            throw new TypeMismatchError(message);
        }

        return ProviderEvaluation.<T>builder()
                .value((T) value)
                .variant(resolvedVariant)
                .reason(reason)
                .flagMetadata(ImmutableMetadata.builder().build())
                .build();
    }

    /**
     * Simple feature flag model (extracted from old implementation).
     */
    public static class FeatureFlag {
        private final String state;
        private final String defaultVariant;
        private final Map<String, Object> variants;
        private final String targeting;

        public FeatureFlag(String state, String defaultVariant,
                          Map<String, Object> variants, String targeting) {
            this.state = state;
            this.defaultVariant = defaultVariant;
            this.variants = variants;
            this.targeting = targeting;
        }

        public String getState() {
            return state;
        }

        public String getDefaultVariant() {
            return defaultVariant;
        }

        public Map<String, Object> getVariants() {
            return variants;
        }

        public String getTargeting() {
            return targeting == null ? EMPTY_TARGETING_STRING : targeting;
        }
    }
}
