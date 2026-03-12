package dev.openfeature.flagd.evaluator;

import dev.openfeature.sdk.EvaluationContext;
import dev.openfeature.sdk.MutableContext;
import org.junit.jupiter.api.Test;
import static org.assertj.core.api.Assertions.*;

class YamlLoadingTest {

    private static final String SIMPLE_YAML =
        "flags:\n" +
        "  bool-flag:\n" +
        "    state: ENABLED\n" +
        "    variants:\n" +
        "      'on': true\n" +
        "      'off': false\n" +
        "    defaultVariant: 'on'\n" +
        "  string-flag:\n" +
        "    state: ENABLED\n" +
        "    variants:\n" +
        "      v1: hello\n" +
        "      v2: world\n" +
        "    defaultVariant: v1\n";

    private static final String SIMPLE_JSON =
        "{\"flags\":{" +
        "\"bool-flag\":{\"state\":\"ENABLED\",\"variants\":{\"on\":true,\"off\":false},\"defaultVariant\":\"on\"}," +
        "\"string-flag\":{\"state\":\"ENABLED\",\"variants\":{\"v1\":\"hello\",\"v2\":\"world\"},\"defaultVariant\":\"v1\"}" +
        "}}";

    @Test
    void updateStateLoadsBooleanFlag() throws Exception {
        FlagEvaluator evaluator = new FlagEvaluator();
        evaluator.updateState(SIMPLE_YAML);

        EvaluationContext ctx = new MutableContext("user-1");
        EvaluationResult<Boolean> result = evaluator.evaluateFlag(Boolean.class, "bool-flag", ctx);
        assertThat(result.getValue()).isTrue();
    }

    @Test
    void updateStateLoadsStringFlag() throws Exception {
        FlagEvaluator evaluator = new FlagEvaluator();
        evaluator.updateState(SIMPLE_YAML);

        EvaluationContext ctx = new MutableContext("user-1");
        EvaluationResult<String> result = evaluator.evaluateFlag(String.class, "string-flag", ctx);
        assertThat(result.getValue()).isEqualTo("hello");
    }

    @Test
    void updateStateInvalidYamlThrowsEvaluatorException() {
        FlagEvaluator evaluator = new FlagEvaluator();
        assertThatThrownBy(() -> evaluator.updateState("flags:\n  bad: [unclosed"))
            .isInstanceOf(EvaluatorException.class);
    }

    @Test
    void updateStateYamlAndJsonProduceSameResultsParity() throws Exception {
        FlagEvaluator yamlEvaluator = new FlagEvaluator();
        yamlEvaluator.updateState(SIMPLE_YAML);

        FlagEvaluator jsonEvaluator = new FlagEvaluator();
        jsonEvaluator.updateState(SIMPLE_JSON);

        EvaluationContext ctx = new MutableContext("user-1");
        EvaluationResult<Boolean> yamlResult = yamlEvaluator.evaluateFlag(Boolean.class, "bool-flag", ctx);
        EvaluationResult<Boolean> jsonResult = jsonEvaluator.evaluateFlag(Boolean.class, "bool-flag", ctx);
        assertThat(yamlResult.getValue()).isEqualTo(jsonResult.getValue());
        assertThat(yamlResult.getVariant()).isEqualTo(jsonResult.getVariant());

        EvaluationResult<String> yamlStrResult = yamlEvaluator.evaluateFlag(String.class, "string-flag", ctx);
        EvaluationResult<String> jsonStrResult = jsonEvaluator.evaluateFlag(String.class, "string-flag", ctx);
        assertThat(yamlStrResult.getValue()).isEqualTo(jsonStrResult.getValue());
        assertThat(yamlStrResult.getVariant()).isEqualTo(jsonStrResult.getVariant());
    }

    // Auto-detection tests

    @Test
    void updateStateAutoDetectsYaml() throws Exception {
        FlagEvaluator evaluator = new FlagEvaluator();
        evaluator.updateState(SIMPLE_YAML);

        EvaluationContext ctx = new MutableContext("user-1");
        EvaluationResult<Boolean> result = evaluator.evaluateFlag(Boolean.class, "bool-flag", ctx);
        assertThat(result.getValue()).isTrue();
    }

    @Test
    void updateStateAutoDetectsJson() throws Exception {
        FlagEvaluator evaluator = new FlagEvaluator();
        evaluator.updateState(SIMPLE_JSON);

        EvaluationContext ctx = new MutableContext("user-1");
        EvaluationResult<Boolean> result = evaluator.evaluateFlag(Boolean.class, "bool-flag", ctx);
        assertThat(result.getValue()).isTrue();
    }

    @Test
    void updateStateYamlAndJsonProduceSameResults() throws Exception {
        FlagEvaluator yamlEvaluator = new FlagEvaluator();
        yamlEvaluator.updateState(SIMPLE_YAML);

        FlagEvaluator jsonEvaluator = new FlagEvaluator();
        jsonEvaluator.updateState(SIMPLE_JSON);

        EvaluationContext ctx = new MutableContext("user-1");

        EvaluationResult<Boolean> yamlBool = yamlEvaluator.evaluateFlag(Boolean.class, "bool-flag", ctx);
        EvaluationResult<Boolean> jsonBool = jsonEvaluator.evaluateFlag(Boolean.class, "bool-flag", ctx);
        assertThat(yamlBool.getValue()).isEqualTo(jsonBool.getValue());
        assertThat(yamlBool.getVariant()).isEqualTo(jsonBool.getVariant());

        EvaluationResult<String> yamlStr = yamlEvaluator.evaluateFlag(String.class, "string-flag", ctx);
        EvaluationResult<String> jsonStr = jsonEvaluator.evaluateFlag(String.class, "string-flag", ctx);
        assertThat(yamlStr.getValue()).isEqualTo(jsonStr.getValue());
        assertThat(yamlStr.getVariant()).isEqualTo(jsonStr.getVariant());
    }
}
