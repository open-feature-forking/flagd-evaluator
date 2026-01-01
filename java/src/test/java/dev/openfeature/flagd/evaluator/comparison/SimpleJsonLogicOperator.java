package dev.openfeature.flagd.evaluator.comparison;

import com.fasterxml.jackson.databind.ObjectMapper;
import com.fasterxml.jackson.databind.module.SimpleModule;
import dev.openfeature.flagd.evaluator.jackson.EvaluationContextSerializer;
import dev.openfeature.sdk.EvaluationContext;
import io.github.jamsesso.jsonlogic.JsonLogic;

/**
 * JSON Logic operator wrapper using the actual json-logic-java library.
 *
 * <p>This uses the same JsonLogic library that the old InProcessResolver used,
 * allowing us to do a true performance comparison between:
 * - Old: Java-based JsonLogic evaluation
 * - New: WASM-based evaluation
 */
class SimpleJsonLogicOperator {

    private static final ObjectMapper OBJECT_MAPPER = new ObjectMapper();
    private final JsonLogic jsonLogic;

    static {
        SimpleModule module = new SimpleModule();
        module.addSerializer(EvaluationContext.class, new EvaluationContextSerializer());
        OBJECT_MAPPER.registerModule(module);
    }

    SimpleJsonLogicOperator() {
        this.jsonLogic = new JsonLogic();
    }

    /**
     * Apply targeting rule using the JsonLogic library.
     *
     * @param flagKey The flag key (not used by JsonLogic, but kept for API compatibility)
     * @param targetingRule JSON Logic rule as string
     * @param ctx Evaluation context
     * @return The result of evaluating the rule, or null if rule is empty/null
     */
    Object apply(String flagKey, String targetingRule, EvaluationContext ctx) throws Exception {
        if (targetingRule == null || targetingRule.equals("null") || targetingRule.equals("{}")) {
            return null;
        }

        // Serialize context to Map (JsonLogic expects a Map, not a JSON string!)
        String contextJson = OBJECT_MAPPER.writeValueAsString(ctx);
        @SuppressWarnings("unchecked")
        java.util.Map<String, Object> contextMap = OBJECT_MAPPER.readValue(contextJson, java.util.Map.class);

        // Evaluate using JsonLogic (rule as string, data as Map)
        Object result = jsonLogic.apply(targetingRule, contextMap);

        return result;
    }
}
