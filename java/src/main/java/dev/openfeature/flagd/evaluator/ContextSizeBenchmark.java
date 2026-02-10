package dev.openfeature.flagd.evaluator;

import org.openjdk.jmh.annotations.*;
import org.openjdk.jmh.infra.Blackhole;

import java.util.concurrent.TimeUnit;

/**
 * JMH benchmarks measuring the impact of context size on evaluation performance.
 *
 * <p>These benchmarks vary context size (empty, small, large) across different
 * flag complexities (simple, simple targeting, complex targeting) to isolate
 * serialization overhead from evaluation logic overhead.
 *
 * <p><b>Running the benchmarks:</b>
 * <pre>
 * ./mvnw clean package
 * java -jar target/benchmarks.jar ContextSizeBenchmark
 *
 * # Run a specific scenario:
 * java -jar target/benchmarks.jar ContextSizeBenchmark.E1_simpleFlag_emptyContext
 * </pre>
 */
@BenchmarkMode({Mode.Throughput, Mode.AverageTime})
@OutputTimeUnit(TimeUnit.MICROSECONDS)
@Fork(1)
@Warmup(iterations = 3, time = 2)
@Measurement(iterations = 5, time = 3)
public class ContextSizeBenchmark {

    // Simple flag: no targeting rules
    private static final String SIMPLE_FLAG_CONFIG = "{\n" +
        "  \"flags\": {\n" +
        "    \"simple-bool\": {\n" +
        "      \"state\": \"ENABLED\",\n" +
        "      \"defaultVariant\": \"on\",\n" +
        "      \"variants\": {\n" +
        "        \"on\": true,\n" +
        "        \"off\": false\n" +
        "      }\n" +
        "    },\n" +
        "    \"simple-targeting\": {\n" +
        "      \"state\": \"ENABLED\",\n" +
        "      \"defaultVariant\": \"denied\",\n" +
        "      \"variants\": {\n" +
        "        \"denied\": false,\n" +
        "        \"granted\": true\n" +
        "      },\n" +
        "      \"targeting\": {\n" +
        "        \"if\": [\n" +
        "          { \"==\": [{ \"var\": \"role\" }, \"admin\"] },\n" +
        "          \"granted\",\n" +
        "          null\n" +
        "        ]\n" +
        "      }\n" +
        "    },\n" +
        "    \"complex-targeting\": {\n" +
        "      \"state\": \"ENABLED\",\n" +
        "      \"defaultVariant\": \"basic\",\n" +
        "      \"variants\": {\n" +
        "        \"premium\": \"premium-tier\",\n" +
        "        \"standard\": \"standard-tier\",\n" +
        "        \"basic\": \"basic-tier\"\n" +
        "      },\n" +
        "      \"targeting\": {\n" +
        "        \"if\": [\n" +
        "          { \"and\": [\n" +
        "            { \"==\": [{ \"var\": \"tier\" }, \"premium\"] },\n" +
        "            { \">\": [{ \"var\": \"score\" }, 90] }\n" +
        "          ]},\n" +
        "          \"premium\",\n" +
        "          { \"if\": [\n" +
        "            { \"or\": [\n" +
        "              { \"==\": [{ \"var\": \"tier\" }, \"standard\"] },\n" +
        "              { \">\": [{ \"var\": \"score\" }, 50] }\n" +
        "            ]},\n" +
        "            \"standard\",\n" +
        "            \"basic\"\n" +
        "          ]}\n" +
        "        ]\n" +
        "      }\n" +
        "    }\n" +
        "  }\n" +
        "}";

    // Empty context
    private static final String EMPTY_CONTEXT = "{}";

    // Small context (5 attributes)
    private static final String SMALL_CONTEXT = "{" +
        "\"targetingKey\": \"user-123\", " +
        "\"tier\": \"premium\", " +
        "\"role\": \"admin\", " +
        "\"region\": \"us-east\", " +
        "\"score\": 85" +
        "}";

    // Large context is built dynamically in setup (100+ attributes)

    @State(Scope.Benchmark)
    public static class EvaluatorState {
        FlagEvaluator evaluator;
        String largeContextJson;

        @Setup(Level.Trial)
        public void setup() {
            try {
                evaluator = new FlagEvaluator(FlagEvaluator.ValidationMode.PERMISSIVE);
                UpdateStateResult result = evaluator.updateState(SIMPLE_FLAG_CONFIG);
                if (!result.isSuccess()) {
                    throw new RuntimeException("Failed to update flag state: " + result.getError());
                }

                // Build large context: small context attrs + attr_0 through attr_99
                StringBuilder sb = new StringBuilder(4096);
                sb.append("{");
                sb.append("\"targetingKey\": \"user-123\", ");
                sb.append("\"tier\": \"premium\", ");
                sb.append("\"role\": \"admin\", ");
                sb.append("\"region\": \"us-east\", ");
                sb.append("\"score\": 85");
                for (int i = 0; i < 100; i++) {
                    sb.append(", ");
                    // Mixed types: strings, ints, booleans, floats
                    switch (i % 4) {
                        case 0:
                            sb.append("\"attr_").append(i).append("\": \"value-").append(i).append("\"");
                            break;
                        case 1:
                            sb.append("\"attr_").append(i).append("\": ").append(i * 7);
                            break;
                        case 2:
                            sb.append("\"attr_").append(i).append("\": ").append(i % 2 == 0 ? "true" : "false");
                            break;
                        case 3:
                            sb.append("\"attr_").append(i).append("\": ").append(i * 1.5);
                            break;
                    }
                }
                sb.append("}");
                largeContextJson = sb.toString();
            } catch (Exception e) {
                throw new RuntimeException("Failed to setup evaluator state", e);
            }
        }

        @TearDown(Level.Trial)
        public void tearDown() {
            if (evaluator != null) {
                evaluator.close();
            }
        }
    }

    // ========================================================================
    // E1: Simple flag, empty context (baseline)
    // ========================================================================

    @Benchmark
    public void E1_simpleFlag_emptyContext(EvaluatorState state, Blackhole bh) {
        try {
            EvaluationResult<Boolean> result = state.evaluator.evaluateFlag(
                Boolean.class, "simple-bool", EMPTY_CONTEXT);
            bh.consume(result.getValue());
            bh.consume(result.getVariant());
        } catch (Exception e) {
            throw new RuntimeException("Benchmark failed", e);
        }
    }

    // ========================================================================
    // E2: Simple flag, small context
    // ========================================================================

    @Benchmark
    public void E2_simpleFlag_smallContext(EvaluatorState state, Blackhole bh) {
        try {
            EvaluationResult<Boolean> result = state.evaluator.evaluateFlag(
                Boolean.class, "simple-bool", SMALL_CONTEXT);
            bh.consume(result.getValue());
            bh.consume(result.getVariant());
        } catch (Exception e) {
            throw new RuntimeException("Benchmark failed", e);
        }
    }

    // ========================================================================
    // E3: Simple flag, large context
    // ========================================================================

    @Benchmark
    public void E3_simpleFlag_largeContext(EvaluatorState state, Blackhole bh) {
        try {
            EvaluationResult<Boolean> result = state.evaluator.evaluateFlag(
                Boolean.class, "simple-bool", state.largeContextJson);
            bh.consume(result.getValue());
            bh.consume(result.getVariant());
        } catch (Exception e) {
            throw new RuntimeException("Benchmark failed", e);
        }
    }

    // ========================================================================
    // E4: Simple targeting, small context
    // ========================================================================

    @Benchmark
    public void E4_simpleTargeting_smallContext(EvaluatorState state, Blackhole bh) {
        try {
            EvaluationResult<Boolean> result = state.evaluator.evaluateFlag(
                Boolean.class, "simple-targeting", SMALL_CONTEXT);
            bh.consume(result.getValue());
            bh.consume(result.getVariant());
        } catch (Exception e) {
            throw new RuntimeException("Benchmark failed", e);
        }
    }

    // ========================================================================
    // E5: Simple targeting, large context
    // ========================================================================

    @Benchmark
    public void E5_simpleTargeting_largeContext(EvaluatorState state, Blackhole bh) {
        try {
            EvaluationResult<Boolean> result = state.evaluator.evaluateFlag(
                Boolean.class, "simple-targeting", state.largeContextJson);
            bh.consume(result.getValue());
            bh.consume(result.getVariant());
        } catch (Exception e) {
            throw new RuntimeException("Benchmark failed", e);
        }
    }

    // ========================================================================
    // E6: Complex targeting, small context
    // ========================================================================

    @Benchmark
    public void E6_complexTargeting_smallContext(EvaluatorState state, Blackhole bh) {
        try {
            EvaluationResult<String> result = state.evaluator.evaluateFlag(
                String.class, "complex-targeting", SMALL_CONTEXT);
            bh.consume(result.getValue());
            bh.consume(result.getVariant());
        } catch (Exception e) {
            throw new RuntimeException("Benchmark failed", e);
        }
    }

    // ========================================================================
    // E7: Complex targeting, large context
    // ========================================================================

    @Benchmark
    public void E7_complexTargeting_largeContext(EvaluatorState state, Blackhole bh) {
        try {
            EvaluationResult<String> result = state.evaluator.evaluateFlag(
                String.class, "complex-targeting", state.largeContextJson);
            bh.consume(result.getValue());
            bh.consume(result.getVariant());
        } catch (Exception e) {
            throw new RuntimeException("Benchmark failed", e);
        }
    }

    /**
     * Main method to run benchmarks standalone.
     */
    public static void main(String[] args) throws Exception {
        org.openjdk.jmh.Main.main(args);
    }
}
