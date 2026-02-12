package dev.openfeature.flagd.evaluator;

import org.openjdk.jmh.annotations.*;
import org.openjdk.jmh.infra.Blackhole;

import java.util.concurrent.TimeUnit;

/**
 * JMH benchmarks for custom operator evaluation performance (O1-O6).
 *
 * <p>These benchmarks measure the cost of evaluating flags that use custom
 * operators: fractional bucketing, semantic version comparison, and string
 * prefix/suffix matching. Each benchmark isolates a single operator to
 * provide clear per-operator performance data.
 *
 * <p><b>Running the benchmarks:</b>
 * <pre>
 * ./mvnw clean package
 * java -jar target/benchmarks.jar OperatorBenchmark
 *
 * # Run a specific operator:
 * java -jar target/benchmarks.jar OperatorBenchmark.O1_fractional2
 * </pre>
 */
@BenchmarkMode({Mode.Throughput, Mode.AverageTime})
@OutputTimeUnit(TimeUnit.MICROSECONDS)
@Fork(1)
@Warmup(iterations = 3, time = 2)
@Measurement(iterations = 5, time = 3)
public class OperatorBenchmark {

    // O1: Fractional with 2 buckets
    private static final String FRACTIONAL_2_CONFIG = "{\n" +
        "  \"flags\": {\n" +
        "    \"frac2-flag\": {\n" +
        "      \"state\": \"ENABLED\",\n" +
        "      \"defaultVariant\": \"control\",\n" +
        "      \"variants\": {\n" +
        "        \"control\": \"control\",\n" +
        "        \"treatment\": \"treatment\"\n" +
        "      },\n" +
        "      \"targeting\": {\n" +
        "        \"fractional\": [\n" +
        "          [{ \"var\": \"targetingKey\" }],\n" +
        "          [\"control\", 50],\n" +
        "          [\"treatment\", 50]\n" +
        "        ]\n" +
        "      }\n" +
        "    }\n" +
        "  }\n" +
        "}";

    // O2: Fractional with 8 buckets
    private static final String FRACTIONAL_8_CONFIG = "{\n" +
        "  \"flags\": {\n" +
        "    \"frac8-flag\": {\n" +
        "      \"state\": \"ENABLED\",\n" +
        "      \"defaultVariant\": \"v1\",\n" +
        "      \"variants\": {\n" +
        "        \"v1\": \"v1\", \"v2\": \"v2\", \"v3\": \"v3\", \"v4\": \"v4\",\n" +
        "        \"v5\": \"v5\", \"v6\": \"v6\", \"v7\": \"v7\", \"v8\": \"v8\"\n" +
        "      },\n" +
        "      \"targeting\": {\n" +
        "        \"fractional\": [\n" +
        "          [{ \"var\": \"targetingKey\" }],\n" +
        "          [\"v1\", 12], [\"v2\", 13], [\"v3\", 12], [\"v4\", 13],\n" +
        "          [\"v5\", 12], [\"v6\", 13], [\"v7\", 12], [\"v8\", 13]\n" +
        "        ]\n" +
        "      }\n" +
        "    }\n" +
        "  }\n" +
        "}";

    // O3: Semver equality comparison
    private static final String SEMVER_EQ_CONFIG = "{\n" +
        "  \"flags\": {\n" +
        "    \"semver-eq-flag\": {\n" +
        "      \"state\": \"ENABLED\",\n" +
        "      \"defaultVariant\": \"off\",\n" +
        "      \"variants\": {\n" +
        "        \"on\": true,\n" +
        "        \"off\": false\n" +
        "      },\n" +
        "      \"targeting\": {\n" +
        "        \"if\": [\n" +
        "          { \"sem_ver\": [{ \"var\": \"version\" }, \"=\", \"1.2.3\"] },\n" +
        "          \"on\", \"off\"\n" +
        "        ]\n" +
        "      }\n" +
        "    }\n" +
        "  }\n" +
        "}";

    // O4: Semver range (caret)
    private static final String SEMVER_RANGE_CONFIG = "{\n" +
        "  \"flags\": {\n" +
        "    \"semver-range-flag\": {\n" +
        "      \"state\": \"ENABLED\",\n" +
        "      \"defaultVariant\": \"off\",\n" +
        "      \"variants\": {\n" +
        "        \"on\": true,\n" +
        "        \"off\": false\n" +
        "      },\n" +
        "      \"targeting\": {\n" +
        "        \"if\": [\n" +
        "          { \"sem_ver\": [{ \"var\": \"version\" }, \"^\", \"1.2.0\"] },\n" +
        "          \"on\", \"off\"\n" +
        "        ]\n" +
        "      }\n" +
        "    }\n" +
        "  }\n" +
        "}";

    // O5: starts_with string matching
    private static final String STARTS_WITH_CONFIG = "{\n" +
        "  \"flags\": {\n" +
        "    \"starts-flag\": {\n" +
        "      \"state\": \"ENABLED\",\n" +
        "      \"defaultVariant\": \"off\",\n" +
        "      \"variants\": {\n" +
        "        \"on\": true,\n" +
        "        \"off\": false\n" +
        "      },\n" +
        "      \"targeting\": {\n" +
        "        \"if\": [\n" +
        "          { \"starts_with\": [{ \"var\": \"email\" }, \"admin\"] },\n" +
        "          \"on\", \"off\"\n" +
        "        ]\n" +
        "      }\n" +
        "    }\n" +
        "  }\n" +
        "}";

    // O6: ends_with string matching
    private static final String ENDS_WITH_CONFIG = "{\n" +
        "  \"flags\": {\n" +
        "    \"ends-flag\": {\n" +
        "      \"state\": \"ENABLED\",\n" +
        "      \"defaultVariant\": \"off\",\n" +
        "      \"variants\": {\n" +
        "        \"on\": true,\n" +
        "        \"off\": false\n" +
        "      },\n" +
        "      \"targeting\": {\n" +
        "        \"if\": [\n" +
        "          { \"ends_with\": [{ \"var\": \"email\" }, \"@example.com\"] },\n" +
        "          \"on\", \"off\"\n" +
        "        ]\n" +
        "      }\n" +
        "    }\n" +
        "  }\n" +
        "}";

    // Context JSON strings
    private static final String FRACTIONAL_CONTEXT = "{\"targetingKey\": \"user-123\"}";
    private static final String SEMVER_EQ_CONTEXT = "{\"version\": \"1.2.3\"}";
    private static final String SEMVER_RANGE_CONTEXT = "{\"version\": \"1.5.0\"}";
    private static final String STARTS_WITH_CONTEXT = "{\"email\": \"admin@example.com\"}";
    private static final String ENDS_WITH_CONTEXT = "{\"email\": \"user@example.com\"}";

    // ========================================================================
    // State: one evaluator per operator to avoid config interference
    // ========================================================================

    @State(Scope.Benchmark)
    public static class Fractional2State {
        FlagEvaluator evaluator;

        @Setup(Level.Trial)
        public void setup() {
            try {
                evaluator = new FlagEvaluator(FlagEvaluator.ValidationMode.PERMISSIVE);
                UpdateStateResult result = evaluator.updateState(FRACTIONAL_2_CONFIG);
                if (!result.isSuccess()) {
                    throw new RuntimeException("Failed to update state: " + result.getError());
                }
            } catch (Exception e) {
                throw new RuntimeException("Failed to setup Fractional2State", e);
            }
        }

        @TearDown(Level.Trial)
        public void tearDown() {
            if (evaluator != null) {
                evaluator.close();
            }
        }
    }

    @State(Scope.Benchmark)
    public static class Fractional8State {
        FlagEvaluator evaluator;

        @Setup(Level.Trial)
        public void setup() {
            try {
                evaluator = new FlagEvaluator(FlagEvaluator.ValidationMode.PERMISSIVE);
                UpdateStateResult result = evaluator.updateState(FRACTIONAL_8_CONFIG);
                if (!result.isSuccess()) {
                    throw new RuntimeException("Failed to update state: " + result.getError());
                }
            } catch (Exception e) {
                throw new RuntimeException("Failed to setup Fractional8State", e);
            }
        }

        @TearDown(Level.Trial)
        public void tearDown() {
            if (evaluator != null) {
                evaluator.close();
            }
        }
    }

    @State(Scope.Benchmark)
    public static class SemverEqState {
        FlagEvaluator evaluator;

        @Setup(Level.Trial)
        public void setup() {
            try {
                evaluator = new FlagEvaluator(FlagEvaluator.ValidationMode.PERMISSIVE);
                UpdateStateResult result = evaluator.updateState(SEMVER_EQ_CONFIG);
                if (!result.isSuccess()) {
                    throw new RuntimeException("Failed to update state: " + result.getError());
                }
            } catch (Exception e) {
                throw new RuntimeException("Failed to setup SemverEqState", e);
            }
        }

        @TearDown(Level.Trial)
        public void tearDown() {
            if (evaluator != null) {
                evaluator.close();
            }
        }
    }

    @State(Scope.Benchmark)
    public static class SemverRangeState {
        FlagEvaluator evaluator;

        @Setup(Level.Trial)
        public void setup() {
            try {
                evaluator = new FlagEvaluator(FlagEvaluator.ValidationMode.PERMISSIVE);
                UpdateStateResult result = evaluator.updateState(SEMVER_RANGE_CONFIG);
                if (!result.isSuccess()) {
                    throw new RuntimeException("Failed to update state: " + result.getError());
                }
            } catch (Exception e) {
                throw new RuntimeException("Failed to setup SemverRangeState", e);
            }
        }

        @TearDown(Level.Trial)
        public void tearDown() {
            if (evaluator != null) {
                evaluator.close();
            }
        }
    }

    @State(Scope.Benchmark)
    public static class StartsWithState {
        FlagEvaluator evaluator;

        @Setup(Level.Trial)
        public void setup() {
            try {
                evaluator = new FlagEvaluator(FlagEvaluator.ValidationMode.PERMISSIVE);
                UpdateStateResult result = evaluator.updateState(STARTS_WITH_CONFIG);
                if (!result.isSuccess()) {
                    throw new RuntimeException("Failed to update state: " + result.getError());
                }
            } catch (Exception e) {
                throw new RuntimeException("Failed to setup StartsWithState", e);
            }
        }

        @TearDown(Level.Trial)
        public void tearDown() {
            if (evaluator != null) {
                evaluator.close();
            }
        }
    }

    @State(Scope.Benchmark)
    public static class EndsWithState {
        FlagEvaluator evaluator;

        @Setup(Level.Trial)
        public void setup() {
            try {
                evaluator = new FlagEvaluator(FlagEvaluator.ValidationMode.PERMISSIVE);
                UpdateStateResult result = evaluator.updateState(ENDS_WITH_CONFIG);
                if (!result.isSuccess()) {
                    throw new RuntimeException("Failed to update state: " + result.getError());
                }
            } catch (Exception e) {
                throw new RuntimeException("Failed to setup EndsWithState", e);
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
    // O1: Fractional operator with 2 buckets
    // ========================================================================

    @Benchmark
    public void O1_fractional2(Fractional2State state, Blackhole bh) {
        try {
            EvaluationResult<String> result = state.evaluator.evaluateFlag(
                String.class, "frac2-flag", FRACTIONAL_CONTEXT);
            bh.consume(result.getValue());
            bh.consume(result.getVariant());
        } catch (Exception e) {
            throw new RuntimeException("Benchmark failed", e);
        }
    }

    // ========================================================================
    // O2: Fractional operator with 8 buckets
    // ========================================================================

    @Benchmark
    public void O2_fractional8(Fractional8State state, Blackhole bh) {
        try {
            EvaluationResult<String> result = state.evaluator.evaluateFlag(
                String.class, "frac8-flag", FRACTIONAL_CONTEXT);
            bh.consume(result.getValue());
            bh.consume(result.getVariant());
        } catch (Exception e) {
            throw new RuntimeException("Benchmark failed", e);
        }
    }

    // ========================================================================
    // O3: Semver equality comparison
    // ========================================================================

    @Benchmark
    public void O3_semverEquality(SemverEqState state, Blackhole bh) {
        try {
            EvaluationResult<Boolean> result = state.evaluator.evaluateFlag(
                Boolean.class, "semver-eq-flag", SEMVER_EQ_CONTEXT);
            bh.consume(result.getValue());
            bh.consume(result.getVariant());
        } catch (Exception e) {
            throw new RuntimeException("Benchmark failed", e);
        }
    }

    // ========================================================================
    // O4: Semver range (caret)
    // ========================================================================

    @Benchmark
    public void O4_semverRange(SemverRangeState state, Blackhole bh) {
        try {
            EvaluationResult<Boolean> result = state.evaluator.evaluateFlag(
                Boolean.class, "semver-range-flag", SEMVER_RANGE_CONTEXT);
            bh.consume(result.getValue());
            bh.consume(result.getVariant());
        } catch (Exception e) {
            throw new RuntimeException("Benchmark failed", e);
        }
    }

    // ========================================================================
    // O5: starts_with string matching
    // ========================================================================

    @Benchmark
    public void O5_startsWith(StartsWithState state, Blackhole bh) {
        try {
            EvaluationResult<Boolean> result = state.evaluator.evaluateFlag(
                Boolean.class, "starts-flag", STARTS_WITH_CONTEXT);
            bh.consume(result.getValue());
            bh.consume(result.getVariant());
        } catch (Exception e) {
            throw new RuntimeException("Benchmark failed", e);
        }
    }

    // ========================================================================
    // O6: ends_with string matching
    // ========================================================================

    @Benchmark
    public void O6_endsWith(EndsWithState state, Blackhole bh) {
        try {
            EvaluationResult<Boolean> result = state.evaluator.evaluateFlag(
                Boolean.class, "ends-flag", ENDS_WITH_CONTEXT);
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
