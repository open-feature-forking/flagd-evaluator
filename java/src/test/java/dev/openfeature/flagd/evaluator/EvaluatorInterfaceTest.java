package dev.openfeature.flagd.evaluator;

import dev.openfeature.contrib.tools.flagd.api.Evaluator;
import dev.openfeature.contrib.tools.flagd.api.FlagStoreException;
import dev.openfeature.sdk.EvaluationContext;
import dev.openfeature.sdk.MutableContext;
import dev.openfeature.sdk.ProviderEvaluation;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;

import java.util.List;
import java.util.Map;

import static org.assertj.core.api.Assertions.assertThat;
import static org.assertj.core.api.Assertions.assertThatThrownBy;

class EvaluatorInterfaceTest {

    private static final String SIMPLE_FLAGS = "{\"flags\":{"
            + "\"bool-flag\":{\"state\":\"ENABLED\",\"variants\":{\"on\":true,\"off\":false},\"defaultVariant\":\"on\"},"
            + "\"string-flag\":{\"state\":\"ENABLED\",\"variants\":{\"v1\":\"hello\",\"v2\":\"world\"},\"defaultVariant\":\"v1\"},"
            + "\"int-flag\":{\"state\":\"ENABLED\",\"variants\":{\"v1\":42,\"v2\":0},\"defaultVariant\":\"v1\"},"
            + "\"double-flag\":{\"state\":\"ENABLED\",\"variants\":{\"v1\":3.14,\"v2\":0.0},\"defaultVariant\":\"v1\"}"
            + "}}";

    private FlagEvaluator evaluator;

    @BeforeEach
    void setUp() throws FlagStoreException {
        evaluator = new FlagEvaluator();
        evaluator.setFlags(SIMPLE_FLAGS);
    }

    @Test
    void implementsEvaluatorInterface() {
        assertThat(evaluator).isInstanceOf(Evaluator.class);
    }

    @Test
    void setFlagsLoadsConfiguration() throws FlagStoreException {
        // Should not throw
        evaluator.setFlags(SIMPLE_FLAGS);
    }

    @Test
    void setFlagsInvalidJsonThrowsFlagStoreException() {
        assertThatThrownBy(() -> evaluator.setFlags("not-valid-json"))
                .isInstanceOf(FlagStoreException.class);
    }

    @Test
    void setFlagsAndGetChangedKeysReturnsChangedFlagsOnFirstLoad() throws FlagStoreException {
        FlagEvaluator fresh = new FlagEvaluator();
        List<String> changed = fresh.setFlagsAndGetChangedKeys(SIMPLE_FLAGS);
        assertThat(changed).isNotNull().isNotEmpty();
    }

    @Test
    void setFlagsAndGetChangedKeysReturnsEmptyOnNoChange() throws FlagStoreException {
        // Load once, then load exact same config again → no changes
        evaluator.setFlagsAndGetChangedKeys(SIMPLE_FLAGS);
        List<String> noChange = evaluator.setFlagsAndGetChangedKeys(SIMPLE_FLAGS);
        assertThat(noChange).isEmpty();
    }

    @Test
    void getFlagSetMetadataReturnsMap() {
        Map<String, Object> metadata = evaluator.getFlagSetMetadata();
        assertThat(metadata).isNotNull();
    }

    @Test
    void resolveBooleanValue() {
        EvaluationContext ctx = new MutableContext();
        ProviderEvaluation<Boolean> result = evaluator.resolveBooleanValue("bool-flag", false, ctx);
        assertThat(result.getValue()).isTrue();
        assertThat(result.getVariant()).isEqualTo("on");
    }

    @Test
    void resolveStringValue() {
        EvaluationContext ctx = new MutableContext();
        ProviderEvaluation<String> result = evaluator.resolveStringValue("string-flag", "default", ctx);
        assertThat(result.getValue()).isEqualTo("hello");
    }

    @Test
    void resolveIntegerValue() {
        EvaluationContext ctx = new MutableContext();
        ProviderEvaluation<Integer> result = evaluator.resolveIntegerValue("int-flag", 0, ctx);
        assertThat(result.getValue()).isEqualTo(42);
    }

    @Test
    void resolveDoubleValue() {
        EvaluationContext ctx = new MutableContext();
        ProviderEvaluation<Double> result = evaluator.resolveDoubleValue("double-flag", 0.0, ctx);
        assertThat(result.getValue()).isEqualTo(3.14);
    }

    @Test
    void resolveFlagNotFoundReturnsDefaultWithError() {
        EvaluationContext ctx = new MutableContext();
        ProviderEvaluation<Boolean> result = evaluator.resolveBooleanValue("nonexistent", false, ctx);
        assertThat(result.getValue()).isEqualTo(false);
        assertThat(result.getErrorCode()).isNotNull();
    }
}
