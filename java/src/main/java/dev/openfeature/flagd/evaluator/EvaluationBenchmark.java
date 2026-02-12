package dev.openfeature.flagd.evaluator;

import org.openjdk.jmh.annotations.*;
import org.openjdk.jmh.infra.Blackhole;

import java.util.concurrent.TimeUnit;

/**
 * JMH benchmarks for evaluation gap scenarios E3, E6, E7, E10, E11.
 *
 * <p>These benchmarks complement {@link ContextSizeBenchmark} by covering
 * large context (1000+ attributes), disabled flags, and missing flags.
 *
 * <p><b>Running the benchmarks:</b>
 * <pre>
 * ./mvnw clean package
 * java -jar target/benchmarks.jar EvaluationBenchmark
 *
 * # Run a specific scenario:
 * java -jar target/benchmarks.jar EvaluationBenchmark.E10_disabledFlag
 * </pre>
 */
@BenchmarkMode({Mode.Throughput, Mode.AverageTime})
@OutputTimeUnit(TimeUnit.MICROSECONDS)
@Fork(1)
@Warmup(iterations = 3, time = 2)
@Measurement(iterations = 5, time = 3)
public class EvaluationBenchmark {

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
        "    }\n" +
        "  }\n" +
        "}";

    // Complex targeting flag with nested if/and/or conditions
    private static final String COMPLEX_TARGETING_CONFIG = "{\n" +
        "  \"flags\": {\n" +
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

    // Disabled flag
    private static final String DISABLED_FLAG_CONFIG = "{\n" +
        "  \"flags\": {\n" +
        "    \"disabled-feature\": {\n" +
        "      \"state\": \"DISABLED\",\n" +
        "      \"defaultVariant\": \"off\",\n" +
        "      \"variants\": {\n" +
        "        \"on\": true,\n" +
        "        \"off\": false\n" +
        "      }\n" +
        "    }\n" +
        "  }\n" +
        "}";

    // Empty config for missing flag scenario
    private static final String EMPTY_FLAGS_CONFIG = "{\"flags\": {}}";

    // Small context (5 attributes)
    private static final String SMALL_CONTEXT = "{" +
        "\"targetingKey\": \"user-123\", " +
        "\"tier\": \"premium\", " +
        "\"role\": \"admin\", " +
        "\"region\": \"us-east\", " +
        "\"score\": 85" +
        "}";

    // Empty context
    private static final String EMPTY_CONTEXT = "{}";

    /**
     * Generates a large context JSON with 1000+ attributes.
     * Includes the standard small context attributes plus attr_0 through attr_999.
     */
    private static String generateLargeContext() {
        StringBuilder sb = new StringBuilder(32768);
        sb.append("{");
        sb.append("\"targetingKey\": \"user-123\", ");
        sb.append("\"tier\": \"premium\", ");
        sb.append("\"role\": \"admin\", ");
        sb.append("\"region\": \"us-east\", ");
        sb.append("\"score\": 85");
        for (int i = 0; i < 1000; i++) {
            sb.append(", ");
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
        return sb.toString();
    }

    // ========================================================================
    // E3: Simple flag, large context (1000+ attributes)
    // ========================================================================

    @State(Scope.Benchmark)
    public static class SimplelargeState {
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
                largeContextJson = generateLargeContext();
            } catch (Exception e) {
                throw new RuntimeException("Failed to setup state", e);
            }
        }

        @TearDown(Level.Trial)
        public void tearDown() {
            if (evaluator != null) {
                evaluator.close();
            }
        }
    }

    @Benchmark
    public void E3_simpleFlag_largeContext(SimplelargeState state, Blackhole bh) {
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
    // E6: Complex targeting, small context (5 attributes)
    // ========================================================================

    @State(Scope.Benchmark)
    public static class ComplexSmallState {
        FlagEvaluator evaluator;

        @Setup(Level.Trial)
        public void setup() {
            try {
                evaluator = new FlagEvaluator(FlagEvaluator.ValidationMode.PERMISSIVE);
                UpdateStateResult result = evaluator.updateState(COMPLEX_TARGETING_CONFIG);
                if (!result.isSuccess()) {
                    throw new RuntimeException("Failed to update flag state: " + result.getError());
                }
            } catch (Exception e) {
                throw new RuntimeException("Failed to setup state", e);
            }
        }

        @TearDown(Level.Trial)
        public void tearDown() {
            if (evaluator != null) {
                evaluator.close();
            }
        }
    }

    @Benchmark
    public void E6_complexTargeting_smallContext(ComplexSmallState state, Blackhole bh) {
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
    // E7: Complex targeting, large context (1000+ attributes)
    // ========================================================================

    @State(Scope.Benchmark)
    public static class ComplexLargeState {
        FlagEvaluator evaluator;
        String largeContextJson;

        @Setup(Level.Trial)
        public void setup() {
            try {
                evaluator = new FlagEvaluator(FlagEvaluator.ValidationMode.PERMISSIVE);
                UpdateStateResult result = evaluator.updateState(COMPLEX_TARGETING_CONFIG);
                if (!result.isSuccess()) {
                    throw new RuntimeException("Failed to update flag state: " + result.getError());
                }
                largeContextJson = generateLargeContext();
            } catch (Exception e) {
                throw new RuntimeException("Failed to setup state", e);
            }
        }

        @TearDown(Level.Trial)
        public void tearDown() {
            if (evaluator != null) {
                evaluator.close();
            }
        }
    }

    @Benchmark
    public void E7_complexTargeting_largeContext(ComplexLargeState state, Blackhole bh) {
        try {
            EvaluationResult<String> result = state.evaluator.evaluateFlag(
                String.class, "complex-targeting", state.largeContextJson);
            bh.consume(result.getValue());
            bh.consume(result.getVariant());
        } catch (Exception e) {
            throw new RuntimeException("Benchmark failed", e);
        }
    }

    // ========================================================================
    // E10: Disabled flag evaluation
    // ========================================================================

    @State(Scope.Benchmark)
    public static class DisabledFlagState {
        FlagEvaluator evaluator;

        @Setup(Level.Trial)
        public void setup() {
            try {
                evaluator = new FlagEvaluator(FlagEvaluator.ValidationMode.PERMISSIVE);
                UpdateStateResult result = evaluator.updateState(DISABLED_FLAG_CONFIG);
                if (!result.isSuccess()) {
                    throw new RuntimeException("Failed to update flag state: " + result.getError());
                }
            } catch (Exception e) {
                throw new RuntimeException("Failed to setup state", e);
            }
        }

        @TearDown(Level.Trial)
        public void tearDown() {
            if (evaluator != null) {
                evaluator.close();
            }
        }
    }

    @Benchmark
    public void E10_disabledFlag(DisabledFlagState state, Blackhole bh) {
        try {
            EvaluationResult<Boolean> result = state.evaluator.evaluateFlag(
                Boolean.class, "disabled-feature", EMPTY_CONTEXT);
            bh.consume(result.getValue());
            bh.consume(result.getVariant());
            bh.consume(result.getReason());
        } catch (Exception e) {
            throw new RuntimeException("Benchmark failed", e);
        }
    }

    // ========================================================================
    // E11: Missing/nonexistent flag evaluation
    // ========================================================================

    @State(Scope.Benchmark)
    public static class MissingFlagState {
        FlagEvaluator evaluator;

        @Setup(Level.Trial)
        public void setup() {
            try {
                evaluator = new FlagEvaluator(FlagEvaluator.ValidationMode.PERMISSIVE);
                UpdateStateResult result = evaluator.updateState(EMPTY_FLAGS_CONFIG);
                if (!result.isSuccess()) {
                    throw new RuntimeException("Failed to update flag state: " + result.getError());
                }
            } catch (Exception e) {
                throw new RuntimeException("Failed to setup state", e);
            }
        }

        @TearDown(Level.Trial)
        public void tearDown() {
            if (evaluator != null) {
                evaluator.close();
            }
        }
    }

    @Benchmark
    public void E11_missingFlag(MissingFlagState state, Blackhole bh) {
        try {
            EvaluationResult<Boolean> result = state.evaluator.evaluateFlag(
                Boolean.class, "nonexistent-flag", EMPTY_CONTEXT);
            bh.consume(result.getValue());
            bh.consume(result.getVariant());
            bh.consume(result.getReason());
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
