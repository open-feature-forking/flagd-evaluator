package dev.openfeature.flagd.evaluator;

import dev.openfeature.sdk.EvaluationContext;
import dev.openfeature.sdk.MutableContext;
import dev.openfeature.sdk.ProviderEvaluation;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;

import java.util.HashMap;
import java.util.Map;

import static org.assertj.core.api.Assertions.assertThat;

/**
 * Integration tests for FlagEvaluator.
 */
class FlagEvaluatorTest {

    private FlagEvaluator evaluator;

    @BeforeEach
    void setUp() {
        evaluator = new FlagEvaluator(FlagEvaluator.ValidationMode.PERMISSIVE);
    }

    @Test
    void testSimpleBooleanFlag() throws EvaluatorException {
        String config = "{\n" +
                "                  \"flags\": {\n" +
                "                    \"simple-flag\": {\n" +
                "                      \"state\": \"ENABLED\",\n" +
                "                      \"defaultVariant\": \"on\",\n" +
                "                      \"variants\": {\n" +
                "                        \"on\": true,\n" +
                "                        \"off\": false\n" +
                "                      }\n" +
                "                    }\n" +
                "                  }\n" +
                "                }";

        UpdateStateResult updateResult = evaluator.updateState(config);
        assertThat(updateResult.isSuccess()).isTrue();
        assertThat(updateResult.getChangedFlags()).contains("simple-flag");

        EvaluationResult<Boolean> result = evaluator.evaluateFlag(Boolean.class, "simple-flag", "{}");
        assertThat(result.getValue()).isEqualTo(true);
        assertThat(result.getVariant()).isEqualTo("on");
        assertThat(result.getReason()).isEqualTo("STATIC");
        assertThat(result.isError()).isFalse();
    }

    @Test
    void testStringFlag() throws EvaluatorException {
        String config = " {\n" +
                "                  \"flags\": {\n" +
                "                    \"color-flag\": {\n" +
                "                      \"state\": \"ENABLED\",\n" +
                "                      \"defaultVariant\": \"red\",\n" +
                "                      \"variants\": {\n" +
                "                        \"red\": \"red\",\n" +
                "                        \"blue\": \"blue\",\n" +
                "                        \"green\": \"green\"\n" +
                "                      }\n" +
                "                    }\n" +
                "                  }\n" +
                "                }";

        UpdateStateResult updateResult = evaluator.updateState(config);
        assertThat(updateResult.isSuccess()).isTrue();

        EvaluationResult<String> result = evaluator.evaluateFlag(String.class, "color-flag", "{}");
        assertThat(result.getValue()).isEqualTo("red");
        assertThat(result.getVariant()).isEqualTo("red");
    }

    @Test
    void testTargetingRule() throws EvaluatorException {
        String config = " {\n" +
                "                  \"flags\": {\n" +
                "                    \"user-flag\": {\n" +
                "                      \"state\": \"ENABLED\",\n" +
                "                      \"defaultVariant\": \"default\",\n" +
                "                      \"variants\": {\n" +
                "                        \"default\": false,\n" +
                "                        \"premium\": true\n" +
                "                      },\n" +
                "                      \"targeting\": {\n" +
                "                        \"if\": [\n" +
                "                          {\n" +
                "                            \"==\": [\n" +
                "                              { \"var\": \"email\" },\n" +
                "                              \"premium@example.com\"\n" +
                "                            ]\n" +
                "                          },\n" +
                "                          \"premium\",\n" +
                "                          null\n" +
                "                        ]\n" +
                "                      }\n" +
                "                    }\n" +
                "                  }\n" +
                "                }";

        UpdateStateResult updateResult = evaluator.updateState(config);
        assertThat(updateResult.isSuccess()).isTrue();

        // Test with matching context
        EvaluationContext context = new MutableContext().add("email", "premium@example.com");
        EvaluationResult<Boolean> result = evaluator.evaluateFlag(Boolean.class, "user-flag", context);
        assertThat(result.getValue()).isEqualTo(true);
        assertThat(result.getVariant()).isEqualTo("premium");
        assertThat(result.getReason()).isEqualTo("TARGETING_MATCH");

        // Test with non-matching context
        context = new MutableContext().add("email", "regular@example.com");
        result = evaluator.evaluateFlag(Boolean.class, "user-flag", context);
        assertThat(result.getValue()).isEqualTo(false);
        assertThat(result.getVariant()).isEqualTo("default");
    }

    @Test
    void testFlagNotFound() throws EvaluatorException {
        String config = "{\n" +
                "                  \"flags\": {}\n" +
                "                }";

        evaluator.updateState(config);

        EvaluationResult<Boolean> result = evaluator.evaluateFlag(Boolean.class, "nonexistent-flag", "{}");
        assertThat(result.getReason()).isEqualTo("FLAG_NOT_FOUND");
    }

    @Test
    void testDisabledFlag() throws EvaluatorException {
        String config = "{\n" +
                "                  \"flags\": {\n" +
                "                    \"disabled-flag\": {\n" +
                "                      \"state\": \"DISABLED\",\n" +
                "                      \"defaultVariant\": \"off\",\n" +
                "                      \"variants\": {\n" +
                "                        \"on\": true,\n" +
                "                        \"off\": false\n" +
                "                      }\n" +
                "                    }\n" +
                "                  }\n" +
                "                }";

        UpdateStateResult updateResult = evaluator.updateState(config);
        assertThat(updateResult.isSuccess()).isTrue();

        EvaluationResult<Boolean> result = evaluator.evaluateFlag(Boolean.class, "disabled-flag", "{}");
        assertThat(result.getValue()).isNull();
        assertThat(result.getReason()).isEqualTo("DISABLED");
    }

    @Test
    void testNumericFlag() throws EvaluatorException {
        String config = "{\n" +
                "                  \"flags\": {\n" +
                "                    \"number-flag\": {\n" +
                "                      \"state\": \"ENABLED\",\n" +
                "                      \"defaultVariant\": \"default\",\n" +
                "                      \"variants\": {\n" +
                "                        \"default\": 42,\n" +
                "                        \"large\": 1000\n" +
                "                      }\n" +
                "                    }\n" +
                "                  }\n" +
                "                }";

        UpdateStateResult updateResult = evaluator.updateState(config);
        assertThat(updateResult.isSuccess()).isTrue();

        EvaluationResult<Integer> result = evaluator.evaluateFlag(Integer.class, "number-flag", "{}");
        assertThat(result.getValue()).isEqualTo(42);
    }

    @Test
    void testContextEnrichment() throws EvaluatorException {
        String config = " {\n" +
                "                  \"flags\": {\n" +
                "                    \"targeting-key-flag\": {\n" +
                "                      \"state\": \"ENABLED\",\n" +
                "                      \"defaultVariant\": \"default\",\n" +
                "                      \"variants\": {\n" +
                "                        \"default\": \"unknown\",\n" +
                "                        \"known\": \"known-user\"\n" +
                "                      },\n" +
                "                      \"targeting\": {\n" +
                "                        \"if\": [\n" +
                "                          {\n" +
                "                            \"!=\": [\n" +
                "                              { \"var\": \"targetingKey\" },\n" +
                "                              \"\"\n" +
                "                            ]\n" +
                "                          },\n" +
                "                          \"known\",\n" +
                "                          null\n" +
                "                        ]\n" +
                "                      }\n" +
                "                    }\n" +
                "                  }\n" +
                "                }";

        UpdateStateResult updateResult = evaluator.updateState(config);
        assertThat(updateResult.isSuccess()).isTrue();

        // Test with targeting key
        EvaluationContext context = new MutableContext("user-123");
        EvaluationResult<String> result = evaluator.evaluateFlag(String.class, "targeting-key-flag", context);
        assertThat(result.getValue()).isEqualTo("known-user");
        assertThat(result.getReason()).isEqualTo("TARGETING_MATCH");
    }

    @Test
    void testUpdateStateWithChangedFlags() throws EvaluatorException {
        // Initial config
        String config1 = "{\n" +
                "                  \"flags\": {\n" +
                "                    \"flag-a\": {\n" +
                "                      \"state\": \"ENABLED\",\n" +
                "                      \"defaultVariant\": \"on\",\n" +
                "                      \"variants\": { \"on\": true }\n" +
                "                    }\n" +
                "                  }\n" +
                "                }";

        UpdateStateResult result1 = evaluator.updateState(config1);
        assertThat(result1.isSuccess()).isTrue();
        assertThat(result1.getChangedFlags()).containsExactly("flag-a");

        // Update with new and modified flags
        String config2 = "{\n" +
                "                  \"flags\": {\n" +
                "                    \"flag-a\": {\n" +
                "                      \"state\": \"DISABLED\",\n" +
                "                      \"defaultVariant\": \"off\",\n" +
                "                      \"variants\": { \"off\": false }\n" +
                "                    },\n" +
                "                    \"flag-b\": {\n" +
                "                      \"state\": \"ENABLED\",\n" +
                "                      \"defaultVariant\": \"on\",\n" +
                "                      \"variants\": { \"on\": true }\n" +
                "                    }\n" +
                "                  }\n" +
                "                }";

        UpdateStateResult result2 = evaluator.updateState(config2);
        assertThat(result2.isSuccess()).isTrue();
        assertThat(result2.getChangedFlags()).containsExactlyInAnyOrder("flag-a", "flag-b");
    }
}
