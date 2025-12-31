package dev.openfeature.flagd.evaluator.comparison;

import dev.openfeature.flagd.evaluator.EvaluationResult;
import dev.openfeature.flagd.evaluator.FlagEvaluator;
import dev.openfeature.sdk.EvaluationContext;
import dev.openfeature.sdk.MutableContext;
import dev.openfeature.sdk.ProviderEvaluation;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;

import static org.assertj.core.api.Assertions.assertThat;

/**
 * Comparison tests between old InProcessResolver pattern and new WASM-based FlagEvaluator.
 *
 * <p>This comparison validates that both implementations produce identical results:
 * - Old: Java-based JsonLogic evaluation (io.github.jamsesso:json-logic-java)
 * - New: WASM-based evaluation (Rust-compiled flagd-evaluator)
 *
 * <p>These tests validate that both implementations produce identical results for:
 * - Simple static flags (no targeting)
 * - Complex targeting rules with JSON Logic operators
 * - Disabled flags
 * - Missing flags
 * - Type conversions
 * - Different flag types (boolean, string, integer, double)
 *
 * <p>This demonstrates that the new simplified API (FlagEvaluator) can replace
 * the old complex resolver pattern (InProcessResolver + FlagStore + QueueSource + Events)
 * while providing identical evaluation results.
 */
class ResolverComparisonTest {

    private MinimalInProcessResolver oldResolver;
    private FlagEvaluator newEvaluator;

    @BeforeEach
    void setUp() throws Exception {
        oldResolver = new MinimalInProcessResolver();
        newEvaluator = new FlagEvaluator(FlagEvaluator.ValidationMode.PERMISSIVE);
    }

    @Test
    void testSimpleBooleanFlag_BothProduceSameResult() throws Exception {
        // Given: A simple boolean flag with no targeting
        String config = "{\n" +
            "  \"flags\": {\n" +
            "    \"simple-flag\": {\n" +
            "      \"state\": \"ENABLED\",\n" +
            "      \"defaultVariant\": \"on\",\n" +
            "      \"variants\": {\n" +
            "        \"on\": true,\n" +
            "        \"off\": false\n" +
            "      }\n" +
            "    }\n" +
            "  }\n" +
            "}";

        // Load into both resolvers
        oldResolver.loadFlags(config);
        newEvaluator.updateState(config);

        // When: Evaluating with empty context
        EvaluationContext ctx = new MutableContext();

        ProviderEvaluation<Boolean> oldResult = oldResolver.booleanEvaluation("simple-flag", false, ctx);
        EvaluationResult<Boolean> newResult = newEvaluator.evaluateFlag(Boolean.class, "simple-flag", ctx);

        // Then: Both produce identical results
        assertThat(newResult.getValue()).isEqualTo(oldResult.getValue());
        assertThat(newResult.getVariant()).isEqualTo(oldResult.getVariant());
        assertThat(newResult.getReason()).isEqualTo(oldResult.getReason());
    }

    @Test
    void testDisabledFlag_BothProduceSameError() throws Exception {
        // Given: A disabled flag
        String config = "{\n" +
            "  \"flags\": {\n" +
            "    \"disabled-flag\": {\n" +
            "      \"state\": \"DISABLED\",\n" +
            "      \"defaultVariant\": \"on\",\n" +
            "      \"variants\": {\n" +
            "        \"on\": true,\n" +
            "        \"off\": false\n" +
            "      }\n" +
            "    }\n" +
            "  }\n" +
            "}";

        oldResolver.loadFlags(config);
        newEvaluator.updateState(config);

        // When: Evaluating disabled flag
        EvaluationContext ctx = new MutableContext();

        ProviderEvaluation<Boolean> oldResult = oldResolver.booleanEvaluation("disabled-flag", false, ctx);
        EvaluationResult<Boolean> newResult = newEvaluator.evaluateFlag(Boolean.class, "disabled-flag", ctx);

        // Then: Both return errors
        assertThat(newResult.isError()).isTrue();
        assertThat(oldResult.getErrorCode()).isNotNull();
        assertThat(newResult.getErrorCode()).isEqualTo(oldResult.getErrorCode().toString());
    }

    @Test
    void testMissingFlag_BothProduceSameError() throws Exception {
        // Given: Empty flag configuration
        String config = "{\n" +
            "  \"flags\": {}\n" +
            "}";

        oldResolver.loadFlags(config);
        newEvaluator.updateState(config);

        // When: Evaluating non-existent flag
        EvaluationContext ctx = new MutableContext();

        ProviderEvaluation<Boolean> oldResult = oldResolver.booleanEvaluation("missing-flag", false, ctx);
        EvaluationResult<Boolean> newResult = newEvaluator.evaluateFlag(Boolean.class, "missing-flag", ctx);

        // Then: Both return FLAG_NOT_FOUND
        assertThat(newResult.isError()).isTrue();
        assertThat(oldResult.getErrorCode()).isNotNull();
        assertThat(newResult.getErrorCode()).isEqualTo(oldResult.getErrorCode().toString());
    }

    @Test
    void testStringFlag_BothProduceSameResult() throws Exception {
        // Given: A string flag
        String config = "{\n" +
            "  \"flags\": {\n" +
            "    \"color-flag\": {\n" +
            "      \"state\": \"ENABLED\",\n" +
            "      \"defaultVariant\": \"red\",\n" +
            "      \"variants\": {\n" +
            "        \"red\": \"red\",\n" +
            "        \"blue\": \"blue\",\n" +
            "        \"green\": \"green\"\n" +
            "      }\n" +
            "    }\n" +
            "  }\n" +
            "}";

        oldResolver.loadFlags(config);
        newEvaluator.updateState(config);

        // When: Evaluating string flag
        EvaluationContext ctx = new MutableContext();

        ProviderEvaluation<String> oldResult = oldResolver.stringEvaluation("color-flag", "red", ctx);
        EvaluationResult<String> newResult = newEvaluator.evaluateFlag(String.class, "color-flag", ctx);

        // Then: Both produce same result
        assertThat(newResult.getValue()).isEqualTo("red").isEqualTo(oldResult.getValue());
        assertThat(newResult.getVariant()).isEqualTo("red").isEqualTo(oldResult.getVariant());
    }

    @Test
    void testIntegerFlag_BothProduceSameResult() throws Exception {
        // Given: An integer flag
        String config = "{\n" +
            "  \"flags\": {\n" +
            "    \"max-items\": {\n" +
            "      \"state\": \"ENABLED\",\n" +
            "      \"defaultVariant\": \"default\",\n" +
            "      \"variants\": {\n" +
            "        \"default\": 10,\n" +
            "        \"premium\": 100\n" +
            "      }\n" +
            "    }\n" +
            "  }\n" +
            "}";

        oldResolver.loadFlags(config);
        newEvaluator.updateState(config);

        // When: Evaluating integer flag
        EvaluationContext ctx = new MutableContext();

        ProviderEvaluation<Integer> oldResult = oldResolver.integerEvaluation("max-items", 10, ctx);
        EvaluationResult<Integer> newResult = newEvaluator.evaluateFlag(Integer.class, "max-items", ctx);

        // Then: Both produce same result
        assertThat(newResult.getValue()).isEqualTo(10).isEqualTo(oldResult.getValue());
        assertThat(newResult.getVariant()).isEqualTo("default").isEqualTo(oldResult.getVariant());
    }

    @Test
    void testDoubleFlag_BothProduceSameResult() throws Exception {
        // Given: A double flag
        String config = "{\n" +
            "  \"flags\": {\n" +
            "    \"pi-value\": {\n" +
            "      \"state\": \"ENABLED\",\n" +
            "      \"defaultVariant\": \"precise\",\n" +
            "      \"variants\": {\n" +
            "        \"precise\": 3.14159,\n" +
            "        \"rough\": 3.14\n" +
            "      }\n" +
            "    }\n" +
            "  }\n" +
            "}";

        oldResolver.loadFlags(config);
        newEvaluator.updateState(config);

        // When: Evaluating double flag
        EvaluationContext ctx = new MutableContext();

        ProviderEvaluation<Double> oldResult = oldResolver.doubleEvaluation("pi-value", 3.14, ctx);
        EvaluationResult<Double> newResult = newEvaluator.evaluateFlag(Double.class, "pi-value", ctx);

        // Then: Both produce same result
        assertThat(newResult.getValue()).isEqualTo(3.14159).isEqualTo(oldResult.getValue());
        assertThat(newResult.getVariant()).isEqualTo("precise").isEqualTo(oldResult.getVariant());
    }

    @Test
    void testComplexTargetingRule_BothProduceSameResult() throws Exception {
        // Given: Complex targeting with multiple conditions
        String config = "{\n" +
            "  \"flags\": {\n" +
            "    \"feature-access\": {\n" +
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
            "              {\n" +
            "                \"==\": [\n" +
            "                  { \"var\": \"role\" },\n" +
            "                  \"admin\"\n" +
            "                ]\n" +
            "              },\n" +
            "              {\n" +
            "                \"in\": [\n" +
            "                  { \"var\": \"tier\" },\n" +
            "                  [\"premium\", \"enterprise\"]\n" +
            "                ]\n" +
            "              }\n" +
            "            ]\n" +
            "          },\n" +
            "          \"granted\",\n" +
            "          null\n" +
            "        ]\n" +
            "      }\n" +
            "    }\n" +
            "  }\n" +
            "}";

        oldResolver.loadFlags(config);
        newEvaluator.updateState(config);

        // When: Evaluating with matching context
        EvaluationContext matchingCtx = new MutableContext()
            .add("role", "admin")
            .add("tier", "premium");

        ProviderEvaluation<Boolean> oldMatch = oldResolver.booleanEvaluation("feature-access", false, matchingCtx);
        EvaluationResult<Boolean> newMatch = newEvaluator.evaluateFlag(Boolean.class, "feature-access", matchingCtx);

        // Then: Both grant access
        assertThat(newMatch.getValue()).isEqualTo(true).isEqualTo(oldMatch.getValue());
        assertThat(newMatch.getVariant()).isEqualTo("granted").isEqualTo(oldMatch.getVariant());

        // When: Evaluating with partial match (admin but wrong tier)
        EvaluationContext partialCtx = new MutableContext()
            .add("role", "admin")
            .add("tier", "basic");

        ProviderEvaluation<Boolean> oldPartial = oldResolver.booleanEvaluation("feature-access", false, partialCtx);
        EvaluationResult<Boolean> newPartial = newEvaluator.evaluateFlag(Boolean.class, "feature-access", partialCtx);

        // Then: Both deny access
        assertThat(newPartial.getValue()).isEqualTo(false).isEqualTo(oldPartial.getValue());
        assertThat(newPartial.getVariant()).isEqualTo("denied").isEqualTo(oldPartial.getVariant());
    }
}
