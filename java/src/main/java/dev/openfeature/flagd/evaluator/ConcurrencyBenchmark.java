package dev.openfeature.flagd.evaluator;

import dev.openfeature.sdk.EvaluationContext;
import dev.openfeature.sdk.MutableContext;
import org.openjdk.jmh.annotations.*;
import org.openjdk.jmh.infra.Blackhole;

import java.util.concurrent.TimeUnit;

/**
 * JMH concurrency benchmarks C1-C6 for FlagEvaluator.
 *
 * <p>Measures the impact of {@code synchronized} methods on throughput as thread
 * count increases. All threads share a single {@link FlagEvaluator} instance,
 * matching the production deployment pattern.
 *
 * <p><b>Benchmark matrix:</b>
 * <ul>
 *   <li>C1: Single thread, simple flag (baseline)</li>
 *   <li>C2: 4 threads, simple flag</li>
 *   <li>C3: 8 threads, simple flag</li>
 *   <li>C4: 4 threads, targeting flag with context</li>
 *   <li>C5: 4 threads, mixed workload (static, targeting, disabled flags)</li>
 *   <li>C6: 4 threads read/write contention (3 readers, 1 writer)</li>
 * </ul>
 *
 * <p><b>Running the benchmarks:</b>
 * <pre>
 * ./mvnw clean package
 * java -jar target/benchmarks.jar ConcurrencyBenchmark
 *
 * # Run a specific scenario:
 * java -jar target/benchmarks.jar "ConcurrencyBenchmark.C1_.*"
 *
 * # Run C6 group benchmark:
 * java -jar target/benchmarks.jar "ConcurrencyBenchmark.C6_.*"
 * </pre>
 */
@BenchmarkMode(Mode.Throughput)
@OutputTimeUnit(TimeUnit.SECONDS)
@Fork(1)
@Warmup(iterations = 3, time = 2)
@Measurement(iterations = 5, time = 3)
public class ConcurrencyBenchmark {

    // Simple flag: static boolean, no targeting
    private static final String SIMPLE_FLAG_CONFIG = "{" +
        "\"flags\": {" +
        "  \"simpleFlag\": {" +
        "    \"state\": \"ENABLED\"," +
        "    \"variants\": {\"on\": true, \"off\": false}," +
        "    \"defaultVariant\": \"on\"" +
        "  }" +
        "}" +
        "}";

    // Targeting flag: evaluates context to resolve variant
    private static final String TARGETING_FLAG_CONFIG = "{" +
        "\"flags\": {" +
        "  \"targetingFlag\": {" +
        "    \"state\": \"ENABLED\"," +
        "    \"variants\": {\"blue\": \"blue\", \"red\": \"red\"}," +
        "    \"defaultVariant\": \"red\"," +
        "    \"targeting\": {" +
        "      \"if\": [{\"==\": [{\"var\": \"color\"}, \"blue\"]}, \"blue\", \"red\"]" +
        "    }" +
        "  }" +
        "}" +
        "}";

    // Mixed config: static + targeting + disabled flags
    private static final String MIXED_FLAG_CONFIG = "{" +
        "\"flags\": {" +
        "  \"staticFlag\": {" +
        "    \"state\": \"ENABLED\"," +
        "    \"variants\": {\"on\": true, \"off\": false}," +
        "    \"defaultVariant\": \"on\"" +
        "  }," +
        "  \"targetingFlag\": {" +
        "    \"state\": \"ENABLED\"," +
        "    \"variants\": {\"blue\": \"blue\", \"red\": \"red\"}," +
        "    \"defaultVariant\": \"red\"," +
        "    \"targeting\": {" +
        "      \"if\": [{\"==\": [{\"var\": \"color\"}, \"blue\"]}, \"blue\", \"red\"]" +
        "    }" +
        "  }," +
        "  \"disabledFlag\": {" +
        "    \"state\": \"DISABLED\"," +
        "    \"variants\": {\"on\": true, \"off\": false}," +
        "    \"defaultVariant\": \"off\"" +
        "  }" +
        "}" +
        "}";

    // Alternate config for C6 write contention (different default variant)
    private static final String MIXED_FLAG_CONFIG_ALT = "{" +
        "\"flags\": {" +
        "  \"staticFlag\": {" +
        "    \"state\": \"ENABLED\"," +
        "    \"variants\": {\"on\": true, \"off\": false}," +
        "    \"defaultVariant\": \"off\"" +
        "  }," +
        "  \"targetingFlag\": {" +
        "    \"state\": \"ENABLED\"," +
        "    \"variants\": {\"blue\": \"blue\", \"red\": \"red\"}," +
        "    \"defaultVariant\": \"blue\"," +
        "    \"targeting\": {" +
        "      \"if\": [{\"==\": [{\"var\": \"color\"}, \"blue\"]}, \"blue\", \"red\"]" +
        "    }" +
        "  }," +
        "  \"disabledFlag\": {" +
        "    \"state\": \"DISABLED\"," +
        "    \"variants\": {\"on\": true, \"off\": false}," +
        "    \"defaultVariant\": \"off\"" +
        "  }" +
        "}" +
        "}";

    // Flag keys for C5 mixed workload rotation
    private static final String[] MIXED_FLAG_KEYS = {"staticFlag", "targetingFlag", "disabledFlag"};

    // ========================================================================
    // Shared State: single FlagEvaluator shared across all threads
    // ========================================================================

    @State(Scope.Benchmark)
    public static class SimpleState {
        FlagEvaluator evaluator;

        @Setup(Level.Trial)
        public void setup() {
            try {
                evaluator = new FlagEvaluator(FlagEvaluator.ValidationMode.PERMISSIVE);
                UpdateStateResult result = evaluator.updateState(SIMPLE_FLAG_CONFIG);
                if (!result.isSuccess()) {
                    throw new RuntimeException("Failed to load simple flag config: " + result.getError());
                }
            } catch (Exception e) {
                throw new RuntimeException("Failed to setup SimpleState", e);
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
    public static class TargetingState {
        FlagEvaluator evaluator;

        @Setup(Level.Trial)
        public void setup() {
            try {
                evaluator = new FlagEvaluator(FlagEvaluator.ValidationMode.PERMISSIVE);
                UpdateStateResult result = evaluator.updateState(TARGETING_FLAG_CONFIG);
                if (!result.isSuccess()) {
                    throw new RuntimeException("Failed to load targeting flag config: " + result.getError());
                }
            } catch (Exception e) {
                throw new RuntimeException("Failed to setup TargetingState", e);
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
    public static class MixedState {
        FlagEvaluator evaluator;

        @Setup(Level.Trial)
        public void setup() {
            try {
                evaluator = new FlagEvaluator(FlagEvaluator.ValidationMode.PERMISSIVE);
                UpdateStateResult result = evaluator.updateState(MIXED_FLAG_CONFIG);
                if (!result.isSuccess()) {
                    throw new RuntimeException("Failed to load mixed flag config: " + result.getError());
                }
            } catch (Exception e) {
                throw new RuntimeException("Failed to setup MixedState", e);
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
    public static class ReadWriteState {
        FlagEvaluator evaluator;

        @Setup(Level.Trial)
        public void setup() {
            try {
                evaluator = new FlagEvaluator(FlagEvaluator.ValidationMode.PERMISSIVE);
                UpdateStateResult result = evaluator.updateState(MIXED_FLAG_CONFIG);
                if (!result.isSuccess()) {
                    throw new RuntimeException("Failed to load config for read/write state: " + result.getError());
                }
            } catch (Exception e) {
                throw new RuntimeException("Failed to setup ReadWriteState", e);
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
    // Thread-local State: per-thread context and rotation counter
    // ========================================================================

    @State(Scope.Thread)
    public static class ThreadContext {
        EvaluationContext targetingContext;
        String simpleContextJson;
        int counter;

        @Setup(Level.Trial)
        public void setup() {
            targetingContext = new MutableContext()
                .add("color", "blue")
                .add("targetingKey", "user-123");

            simpleContextJson = "{\"targetingKey\": \"user-123\"}";
            counter = 0;
        }
    }

    // ========================================================================
    // C1: Single thread, simple flag (baseline)
    // ========================================================================

    @Benchmark
    @Threads(1)
    public void C1_singleThread_simpleFlag(SimpleState state, ThreadContext ctx, Blackhole bh) {
        try {
            EvaluationResult<Boolean> result = state.evaluator.evaluateFlag(
                Boolean.class, "simpleFlag", ctx.simpleContextJson);
            bh.consume(result.getValue());
            bh.consume(result.getVariant());
        } catch (Exception e) {
            throw new RuntimeException("C1 benchmark failed", e);
        }
    }

    // ========================================================================
    // C2: 4 threads, simple flag
    // ========================================================================

    @Benchmark
    @Threads(4)
    public void C2_4threads_simpleFlag(SimpleState state, ThreadContext ctx, Blackhole bh) {
        try {
            EvaluationResult<Boolean> result = state.evaluator.evaluateFlag(
                Boolean.class, "simpleFlag", ctx.simpleContextJson);
            bh.consume(result.getValue());
            bh.consume(result.getVariant());
        } catch (Exception e) {
            throw new RuntimeException("C2 benchmark failed", e);
        }
    }

    // ========================================================================
    // C3: 8 threads, simple flag
    // ========================================================================

    @Benchmark
    @Threads(8)
    public void C3_8threads_simpleFlag(SimpleState state, ThreadContext ctx, Blackhole bh) {
        try {
            EvaluationResult<Boolean> result = state.evaluator.evaluateFlag(
                Boolean.class, "simpleFlag", ctx.simpleContextJson);
            bh.consume(result.getValue());
            bh.consume(result.getVariant());
        } catch (Exception e) {
            throw new RuntimeException("C3 benchmark failed", e);
        }
    }

    // ========================================================================
    // C4: 4 threads, targeting flag with context
    // ========================================================================

    @Benchmark
    @Threads(4)
    public void C4_4threads_targetingFlag(TargetingState state, ThreadContext ctx, Blackhole bh) {
        try {
            EvaluationResult<String> result = state.evaluator.evaluateFlag(
                String.class, "targetingFlag", ctx.targetingContext);
            bh.consume(result.getValue());
            bh.consume(result.getVariant());
        } catch (Exception e) {
            throw new RuntimeException("C4 benchmark failed", e);
        }
    }

    // ========================================================================
    // C5: 4 threads, mixed workload (rotate through static, targeting, disabled)
    // ========================================================================

    @Benchmark
    @Threads(4)
    public void C5_4threads_mixedWorkload(MixedState state, ThreadContext ctx, Blackhole bh) {
        try {
            String flagKey = MIXED_FLAG_KEYS[ctx.counter % MIXED_FLAG_KEYS.length];
            ctx.counter++;

            if ("targetingFlag".equals(flagKey)) {
                EvaluationResult<String> result = state.evaluator.evaluateFlag(
                    String.class, flagKey, ctx.targetingContext);
                bh.consume(result.getValue());
                bh.consume(result.getVariant());
            } else {
                EvaluationResult<Boolean> result = state.evaluator.evaluateFlag(
                    Boolean.class, flagKey, ctx.simpleContextJson);
                bh.consume(result.getValue());
                bh.consume(result.getVariant());
            }
        } catch (Exception e) {
            throw new RuntimeException("C5 benchmark failed", e);
        }
    }

    // ========================================================================
    // C6: 4 threads read/write contention (3 readers, 1 writer)
    // Uses @Group/@GroupThreads for asymmetric workload.
    // ========================================================================

    @Benchmark
    @Group("C6_readWriteContention")
    @GroupThreads(3)
    public void C6_read(ReadWriteState state, ThreadContext ctx, Blackhole bh) {
        try {
            EvaluationResult<Boolean> result = state.evaluator.evaluateFlag(
                Boolean.class, "staticFlag", ctx.simpleContextJson);
            bh.consume(result.getValue());
            bh.consume(result.getVariant());
        } catch (Exception e) {
            throw new RuntimeException("C6 read benchmark failed", e);
        }
    }

    @Benchmark
    @Group("C6_readWriteContention")
    @GroupThreads(1)
    public void C6_write(ReadWriteState state, ThreadContext ctx, Blackhole bh) {
        try {
            // Alternate between two configs to force actual state changes
            String config = (ctx.counter % 2 == 0)
                ? MIXED_FLAG_CONFIG : MIXED_FLAG_CONFIG_ALT;
            ctx.counter++;
            UpdateStateResult result = state.evaluator.updateState(config);
            bh.consume(result.isSuccess());
        } catch (Exception e) {
            throw new RuntimeException("C6 write benchmark failed", e);
        }
    }

    /**
     * Main method to run benchmarks standalone.
     */
    public static void main(String[] args) throws Exception {
        org.openjdk.jmh.Main.main(args);
    }
}
