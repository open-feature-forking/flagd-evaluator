package dev.openfeature.flagd.evaluator;

import dev.openfeature.flagd.evaluator.comparison.MinimalInProcessResolver;
import dev.openfeature.sdk.EvaluationContext;
import dev.openfeature.sdk.MutableContext;
import dev.openfeature.sdk.ProviderEvaluation;
import org.openjdk.jmh.annotations.*;
import org.openjdk.jmh.infra.Blackhole;

import java.util.Random;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.atomic.AtomicLong;

/**
 * JMH benchmarks for concurrent FlagEvaluator access.
 *
 * <p>In production, a single {@link FlagEvaluator} instance is shared across
 * request-handling threads. These benchmarks measure throughput and latency
 * under concurrent load to validate thread-safety and identify contention.
 *
 * <p><b>Running the benchmarks:</b>
 * <pre>
 * ./mvnw clean package
 * java -jar target/benchmarks.jar ConcurrentFlagEvaluatorBenchmark
 *
 * # Run a specific scenario:
 * java -jar target/benchmarks.jar ConcurrentFlagEvaluatorBenchmark.concurrentSimpleFlag
 *
 * # Run with specific thread count:
 * java -jar target/benchmarks.jar ConcurrentFlagEvaluatorBenchmark -t 4
 * </pre>
 */
@BenchmarkMode({Mode.Throughput, Mode.AverageTime})
@OutputTimeUnit(TimeUnit.MICROSECONDS)
@Fork(1)
@Warmup(iterations = 3, time = 2)
@Measurement(iterations = 5, time = 3)
public class ConcurrentFlagEvaluatorBenchmark {

    // Flag configuration with multiple flag types for realistic workload
    private static final String FLAG_CONFIG = "{\n" +
        "  \"flags\": {\n" +
        "    \"simple-bool\": {\n" +
        "      \"state\": \"ENABLED\",\n" +
        "      \"defaultVariant\": \"on\",\n" +
        "      \"variants\": {\n" +
        "        \"on\": true,\n" +
        "        \"off\": false\n" +
        "      }\n" +
        "    },\n" +
        "    \"targeted-access\": {\n" +
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
        "              { \"==\": [{ \"var\": \"role\" }, \"admin\"] },\n" +
        "              { \"in\": [{ \"var\": \"tier\" }, [\"premium\", \"enterprise\"]] }\n" +
        "            ]\n" +
        "          },\n" +
        "          \"granted\",\n" +
        "          null\n" +
        "        ]\n" +
        "      }\n" +
        "    },\n" +
        "    \"disabled-feature\": {\n" +
        "      \"state\": \"DISABLED\",\n" +
        "      \"defaultVariant\": \"off\",\n" +
        "      \"variants\": {\n" +
        "        \"on\": true,\n" +
        "        \"off\": false\n" +
        "      }\n" +
        "    },\n" +
        "    \"header-color\": {\n" +
        "      \"state\": \"ENABLED\",\n" +
        "      \"defaultVariant\": \"red\",\n" +
        "      \"variants\": {\n" +
        "        \"red\": \"#CC0000\",\n" +
        "        \"blue\": \"#0000CC\",\n" +
        "        \"green\": \"#00CC00\"\n" +
        "      },\n" +
        "      \"targeting\": {\n" +
        "        \"fractional\": [\n" +
        "          { \"var\": \"targetingKey\" },\n" +
        "          [\"red\", 50],\n" +
        "          [\"blue\", 25],\n" +
        "          [\"green\", 25]\n" +
        "        ]\n" +
        "      }\n" +
        "    }\n" +
        "  }\n" +
        "}";

    // Flag keys for mixed-flag scenario
    private static final String[] FLAG_KEYS = {
        "simple-bool", "targeted-access", "disabled-feature", "header-color"
    };

    // ========================================================================
    // Shared State (one FlagEvaluator per benchmark run)
    // ========================================================================

    @State(Scope.Benchmark)
    public static class SharedEvaluatorState {
        FlagEvaluator evaluator;

        @Setup(Level.Trial)
        public void setup() {
            try {
                evaluator = new FlagEvaluator(FlagEvaluator.ValidationMode.PERMISSIVE);
                UpdateStateResult result = evaluator.updateState(FLAG_CONFIG);
                if (!result.isSuccess()) {
                    throw new RuntimeException("Failed to update flag state: " + result.getError());
                }
            } catch (Exception e) {
                throw new RuntimeException("Failed to setup shared evaluator", e);
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
    public static class SharedComparisonState {
        FlagEvaluator newEvaluator;
        MinimalInProcessResolver oldResolver;

        @Setup(Level.Trial)
        public void setup() {
            try {
                newEvaluator = new FlagEvaluator(FlagEvaluator.ValidationMode.PERMISSIVE);
                newEvaluator.updateState(FLAG_CONFIG);

                oldResolver = new MinimalInProcessResolver();
                oldResolver.loadFlags(FLAG_CONFIG);
            } catch (Exception e) {
                throw new RuntimeException("Failed to setup comparison state", e);
            }
        }

        @TearDown(Level.Trial)
        public void tearDown() {
            if (newEvaluator != null) {
                newEvaluator.close();
            }
        }
    }

    // State for read/write contention benchmark
    @State(Scope.Benchmark)
    public static class ReadWriteState {
        FlagEvaluator evaluator;
        AtomicLong invocationCount = new AtomicLong(0);

        // Alternate configs to simulate state updates
        static final String CONFIG_A = FLAG_CONFIG;
        static final String CONFIG_B = "{\n" +
            "  \"flags\": {\n" +
            "    \"simple-bool\": {\n" +
            "      \"state\": \"ENABLED\",\n" +
            "      \"defaultVariant\": \"off\",\n" +
            "      \"variants\": {\n" +
            "        \"on\": true,\n" +
            "        \"off\": false\n" +
            "      }\n" +
            "    },\n" +
            "    \"targeted-access\": {\n" +
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
            "    }\n" +
            "  }\n" +
            "}";

        @Setup(Level.Trial)
        public void setup() {
            try {
                evaluator = new FlagEvaluator(FlagEvaluator.ValidationMode.PERMISSIVE);
                evaluator.updateState(CONFIG_A);
                invocationCount.set(0);
            } catch (Exception e) {
                throw new RuntimeException("Failed to setup read/write state", e);
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
    // Thread-local State (per-thread context generation)
    // ========================================================================

    @State(Scope.Thread)
    public static class ThreadContext {
        Random random;
        EvaluationContext matchingContext;
        EvaluationContext nonMatchingContext;
        String simpleContextJson;

        @Setup(Level.Trial)
        public void setup() {
            // Deterministic seed per thread (Thread.currentThread().getId() varies)
            random = new Random(42 + Thread.currentThread().getId());

            matchingContext = new MutableContext()
                .add("role", "admin")
                .add("tier", "premium")
                .add("targetingKey", "user-" + random.nextInt(10000));

            nonMatchingContext = new MutableContext()
                .add("role", "viewer")
                .add("tier", "free")
                .add("targetingKey", "user-" + random.nextInt(10000));

            simpleContextJson = "{\"targetingKey\": \"user-" + random.nextInt(10000) + "\"}";
        }
    }

    // ========================================================================
    // Concurrent Evaluation Benchmarks (1, 2, 4, 8 threads)
    // ========================================================================

    /**
     * All threads evaluate a simple boolean flag with no targeting rules.
     * Measures baseline concurrent throughput with minimal evaluation logic.
     */
    @Benchmark
    @Threads(1)
    public void concurrentSimpleFlag_1t(SharedEvaluatorState state, ThreadContext ctx, Blackhole bh) {
        evaluateSimpleFlag(state, ctx, bh);
    }

    @Benchmark
    @Threads(2)
    public void concurrentSimpleFlag_2t(SharedEvaluatorState state, ThreadContext ctx, Blackhole bh) {
        evaluateSimpleFlag(state, ctx, bh);
    }

    @Benchmark
    @Threads(4)
    public void concurrentSimpleFlag_4t(SharedEvaluatorState state, ThreadContext ctx, Blackhole bh) {
        evaluateSimpleFlag(state, ctx, bh);
    }

    @Benchmark
    @Threads(8)
    public void concurrentSimpleFlag_8t(SharedEvaluatorState state, ThreadContext ctx, Blackhole bh) {
        evaluateSimpleFlag(state, ctx, bh);
    }

    private void evaluateSimpleFlag(SharedEvaluatorState state, ThreadContext ctx, Blackhole bh) {
        try {
            EvaluationResult<Boolean> result = state.evaluator.evaluateFlag(
                Boolean.class, "simple-bool", ctx.simpleContextJson);
            bh.consume(result.getValue());
            bh.consume(result.getVariant());
        } catch (Exception e) {
            throw new RuntimeException("Benchmark failed", e);
        }
    }

    /**
     * All threads evaluate a flag with targeting rules that match.
     * Measures concurrent throughput when targeting logic is exercised.
     */
    @Benchmark
    @Threads(1)
    public void concurrentTargetingMatch_1t(SharedEvaluatorState state, ThreadContext ctx, Blackhole bh) {
        evaluateTargetingMatch(state, ctx, bh);
    }

    @Benchmark
    @Threads(2)
    public void concurrentTargetingMatch_2t(SharedEvaluatorState state, ThreadContext ctx, Blackhole bh) {
        evaluateTargetingMatch(state, ctx, bh);
    }

    @Benchmark
    @Threads(4)
    public void concurrentTargetingMatch_4t(SharedEvaluatorState state, ThreadContext ctx, Blackhole bh) {
        evaluateTargetingMatch(state, ctx, bh);
    }

    @Benchmark
    @Threads(8)
    public void concurrentTargetingMatch_8t(SharedEvaluatorState state, ThreadContext ctx, Blackhole bh) {
        evaluateTargetingMatch(state, ctx, bh);
    }

    private void evaluateTargetingMatch(SharedEvaluatorState state, ThreadContext ctx, Blackhole bh) {
        try {
            EvaluationResult<Boolean> result = state.evaluator.evaluateFlag(
                Boolean.class, "targeted-access", ctx.matchingContext);
            bh.consume(result.getValue());
            bh.consume(result.getVariant());
        } catch (Exception e) {
            throw new RuntimeException("Benchmark failed", e);
        }
    }

    /**
     * All threads evaluate a flag with targeting rules that don't match (default path).
     * Measures concurrent throughput on the fallback/default code path.
     */
    @Benchmark
    @Threads(1)
    public void concurrentTargetingNoMatch_1t(SharedEvaluatorState state, ThreadContext ctx, Blackhole bh) {
        evaluateTargetingNoMatch(state, ctx, bh);
    }

    @Benchmark
    @Threads(2)
    public void concurrentTargetingNoMatch_2t(SharedEvaluatorState state, ThreadContext ctx, Blackhole bh) {
        evaluateTargetingNoMatch(state, ctx, bh);
    }

    @Benchmark
    @Threads(4)
    public void concurrentTargetingNoMatch_4t(SharedEvaluatorState state, ThreadContext ctx, Blackhole bh) {
        evaluateTargetingNoMatch(state, ctx, bh);
    }

    @Benchmark
    @Threads(8)
    public void concurrentTargetingNoMatch_8t(SharedEvaluatorState state, ThreadContext ctx, Blackhole bh) {
        evaluateTargetingNoMatch(state, ctx, bh);
    }

    private void evaluateTargetingNoMatch(SharedEvaluatorState state, ThreadContext ctx, Blackhole bh) {
        try {
            EvaluationResult<Boolean> result = state.evaluator.evaluateFlag(
                Boolean.class, "targeted-access", ctx.nonMatchingContext);
            bh.consume(result.getValue());
            bh.consume(result.getVariant());
        } catch (Exception e) {
            throw new RuntimeException("Benchmark failed", e);
        }
    }

    /**
     * Threads randomly pick different flags to evaluate, simulating a realistic
     * production workload where different requests evaluate different flags.
     */
    @Benchmark
    @Threads(1)
    public void concurrentMixedFlags_1t(SharedEvaluatorState state, ThreadContext ctx, Blackhole bh) {
        evaluateMixedFlags(state, ctx, bh);
    }

    @Benchmark
    @Threads(2)
    public void concurrentMixedFlags_2t(SharedEvaluatorState state, ThreadContext ctx, Blackhole bh) {
        evaluateMixedFlags(state, ctx, bh);
    }

    @Benchmark
    @Threads(4)
    public void concurrentMixedFlags_4t(SharedEvaluatorState state, ThreadContext ctx, Blackhole bh) {
        evaluateMixedFlags(state, ctx, bh);
    }

    @Benchmark
    @Threads(8)
    public void concurrentMixedFlags_8t(SharedEvaluatorState state, ThreadContext ctx, Blackhole bh) {
        evaluateMixedFlags(state, ctx, bh);
    }

    private void evaluateMixedFlags(SharedEvaluatorState state, ThreadContext ctx, Blackhole bh) {
        try {
            String flagKey = FLAG_KEYS[ctx.random.nextInt(FLAG_KEYS.length)];
            EvaluationContext context = ctx.random.nextBoolean()
                ? ctx.matchingContext : ctx.nonMatchingContext;

            // Use String type for header-color, Boolean for everything else
            if ("header-color".equals(flagKey)) {
                EvaluationResult<String> result = state.evaluator.evaluateFlag(
                    String.class, flagKey, context);
                bh.consume(result.getValue());
                bh.consume(result.getVariant());
            } else {
                EvaluationResult<Boolean> result = state.evaluator.evaluateFlag(
                    Boolean.class, flagKey, context);
                bh.consume(result.getValue());
                bh.consume(result.getVariant());
            }
        } catch (Exception e) {
            throw new RuntimeException("Benchmark failed", e);
        }
    }

    /**
     * Simulates read/write contention: most invocations evaluate flags, but
     * every 1000th invocation triggers a state update. This models the
     * production pattern where config syncs happen while evaluations proceed.
     */
    @Benchmark
    @Threads(4)
    public void concurrentWithStateUpdate(ReadWriteState state, ThreadContext ctx, Blackhole bh) {
        try {
            long count = state.invocationCount.incrementAndGet();

            if (count % 1000 == 0) {
                // Occasional state update (simulates config sync)
                String config = (count % 2000 == 0)
                    ? ReadWriteState.CONFIG_A : ReadWriteState.CONFIG_B;
                UpdateStateResult updateResult = state.evaluator.updateState(config);
                bh.consume(updateResult.isSuccess());
            } else {
                // Normal evaluation (vast majority of calls)
                EvaluationResult<Boolean> result = state.evaluator.evaluateFlag(
                    Boolean.class, "simple-bool", ctx.simpleContextJson);
                bh.consume(result.getValue());
                bh.consume(result.getVariant());
            }
        } catch (Exception e) {
            throw new RuntimeException("Benchmark failed", e);
        }
    }

    // ========================================================================
    // Old vs New Comparison Under Concurrency
    // ========================================================================

    /**
     * Old JsonLogic resolver: simple flag evaluation under concurrent load.
     */
    @Benchmark
    @Threads(4)
    public void oldResolver_ConcurrentSimple(SharedComparisonState state, ThreadContext ctx, Blackhole bh) {
        EvaluationContext context = new MutableContext()
            .add("targetingKey", "user-" + ctx.random.nextInt(10000));
        ProviderEvaluation<Boolean> result = state.oldResolver.booleanEvaluation(
            "simple-bool", false, context);
        bh.consume(result);
    }

    /**
     * New WASM evaluator: simple flag evaluation under concurrent load.
     */
    @Benchmark
    @Threads(4)
    public void newEvaluator_ConcurrentSimple(SharedComparisonState state, ThreadContext ctx, Blackhole bh) {
        try {
            EvaluationResult<Boolean> result = state.newEvaluator.evaluateFlag(
                Boolean.class, "simple-bool", ctx.simpleContextJson);
            bh.consume(result.getValue());
            bh.consume(result.getVariant());
        } catch (Exception e) {
            throw new RuntimeException("Benchmark failed", e);
        }
    }

    /**
     * Old JsonLogic resolver: targeting evaluation under concurrent load.
     */
    @Benchmark
    @Threads(4)
    public void oldResolver_ConcurrentTargeting(SharedComparisonState state, ThreadContext ctx, Blackhole bh) {
        ProviderEvaluation<Boolean> result = state.oldResolver.booleanEvaluation(
            "targeted-access", false, ctx.matchingContext);
        bh.consume(result);
    }

    /**
     * New WASM evaluator: targeting evaluation under concurrent load.
     */
    @Benchmark
    @Threads(4)
    public void newEvaluator_ConcurrentTargeting(SharedComparisonState state, ThreadContext ctx, Blackhole bh) {
        try {
            EvaluationResult<Boolean> result = state.newEvaluator.evaluateFlag(
                Boolean.class, "targeted-access", ctx.matchingContext);
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
