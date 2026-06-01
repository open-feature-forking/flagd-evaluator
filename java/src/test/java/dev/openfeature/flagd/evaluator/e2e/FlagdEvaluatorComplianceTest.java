package dev.openfeature.flagd.evaluator.e2e;

import dev.openfeature.contrib.tools.flagd.api.Evaluator;
import dev.openfeature.contrib.tools.flagd.api.testkit.AbstractEvaluatorTest;
import dev.openfeature.flagd.evaluator.FlagEvaluator;
import org.junit.platform.suite.api.ExcludeTags;

/**
 * Compliance test suite running the bundled {@code flagd-api-testkit} Gherkin
 * scenarios against the WASM-backed {@link FlagEvaluator}, which already implements
 * the flagd-api {@link Evaluator} interface directly — no adapter required.
 *
 * <p>All Cucumber runner configuration is provided by {@link AbstractEvaluatorTest}.
 * This class is discovered via Java SPI — see
 * {@code src/test/resources/META-INF/services/dev.openfeature.contrib.tools.flagd.api.testkit.EvaluatorFactory}.
 *
 * <p>{@code fractional-v1} is excluded because the core implements the high-precision
 * v2 bucketing algorithm; the v1 examples assert the legacy float-based results.
 */
@ExcludeTags("fractional-v1")
public class FlagdEvaluatorComplianceTest extends AbstractEvaluatorTest {

    @Override
    public Evaluator create(String flagsJson) throws Exception {
        FlagEvaluator evaluator = new FlagEvaluator();
        evaluator.setFlags(flagsJson);
        return evaluator;
    }
}
