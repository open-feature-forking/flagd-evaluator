package dev.openfeature.flagd.evaluator;

import org.openjdk.jmh.annotations.*;
import org.openjdk.jmh.infra.Blackhole;

import java.util.concurrent.TimeUnit;

/**
 * JMH concurrency benchmarks (C7-C10) for FlagEvaluator.
 *
 * <p>Measures the performance of flag evaluation under high thread contention.
 * Since {@link FlagEvaluator} methods are {@code synchronized} and the WASM
 * module is single-threaded, all evaluations serialize through a single lock.
 * These benchmarks quantify the throughput ceiling under that contention.
 *
 * <p>Scenarios:
 * <ul>
 *   <li><b>C7</b> - 16 threads evaluating a simple static boolean flag</li>
 *   <li><b>C8</b> - 16 threads evaluating a flag with targeting rules</li>
 *   <li><b>C9</b> - 16 threads with mixed workload (static + targeting + disabled)</li>
 *   <li><b>C10</b> - 15 reader threads + 1 writer thread (read/write contention)</li>
 * </ul>
 *
 * <p><b>Running:</b>
 * <pre>
 * ./mvnw clean package -DskipTests
 * java -jar target/benchmarks.jar "Concurrency" -wi 1 -i 1 -f 1   # smoke test
 * java -jar target/benchmarks.jar "Concurrency"                     # full run
 * </pre>
 */
@BenchmarkMode(Mode.Throughput)
@OutputTimeUnit(TimeUnit.SECONDS)
@Fork(value = 1, warmups = 1)
@Warmup(iterations = 3, time = 2)
@Measurement(iterations = 5, time = 3)
public class ConcurrencyJmhBenchmark {

    // Flag configuration matching the Rust concurrency benchmarks (C7-C10).
    // Contains: boolFlag (static), targetedFlag (with targeting), disabledFlag.
    private static final String BENCH_CONFIG = "{\n" +
        "  \"flags\": {\n" +
        "    \"boolFlag\": {\n" +
        "      \"state\": \"ENABLED\",\n" +
        "      \"variants\": { \"on\": true, \"off\": false },\n" +
        "      \"defaultVariant\": \"on\"\n" +
        "    },\n" +
        "    \"targetedFlag\": {\n" +
        "      \"state\": \"ENABLED\",\n" +
        "      \"variants\": { \"admin\": \"admin-value\", \"user\": \"user-value\" },\n" +
        "      \"defaultVariant\": \"user\",\n" +
        "      \"targeting\": {\n" +
        "        \"if\": [\n" +
        "          {\"==\": [{\"var\": \"role\"}, \"admin\"]},\n" +
        "          \"admin\",\n" +
        "          \"user\"\n" +
        "        ]\n" +
        "      }\n" +
        "    },\n" +
        "    \"disabledFlag\": {\n" +
        "      \"state\": \"DISABLED\",\n" +
        "      \"variants\": { \"on\": true, \"off\": false },\n" +
        "      \"defaultVariant\": \"on\"\n" +
        "    }\n" +
        "  }\n" +
        "}";

    // Alternative config for C10 writer thread — same structure, different default.
    private static final String BENCH_CONFIG_ALT = "{\n" +
        "  \"flags\": {\n" +
        "    \"boolFlag\": {\n" +
        "      \"state\": \"ENABLED\",\n" +
        "      \"variants\": { \"on\": true, \"off\": false },\n" +
        "      \"defaultVariant\": \"off\"\n" +
        "    },\n" +
        "    \"targetedFlag\": {\n" +
        "      \"state\": \"ENABLED\",\n" +
        "      \"variants\": { \"admin\": \"admin-value\", \"user\": \"user-value\" },\n" +
        "      \"defaultVariant\": \"user\",\n" +
        "      \"targeting\": {\n" +
        "        \"if\": [\n" +
        "          {\"==\": [{\"var\": \"role\"}, \"admin\"]},\n" +
        "          \"admin\",\n" +
        "          \"user\"\n" +
        "        ]\n" +
        "      }\n" +
        "    },\n" +
        "    \"disabledFlag\": {\n" +
        "      \"state\": \"DISABLED\",\n" +
        "      \"variants\": { \"on\": true, \"off\": false },\n" +
        "      \"defaultVariant\": \"on\"\n" +
        "    }\n" +
        "  }\n" +
        "}";

    // ========================================================================
    // Shared state: single FlagEvaluator instance shared across all threads
    // ========================================================================

    @State(Scope.Benchmark)
    public static class SharedEvaluator {
        FlagEvaluator evaluator;

        @Setup(Level.Trial)
        public void setup() {
            try {
                evaluator = new FlagEvaluator(FlagEvaluator.ValidationMode.PERMISSIVE);
                UpdateStateResult result = evaluator.updateState(BENCH_CONFIG);
                if (!result.isSuccess()) {
                    throw new RuntimeException("Failed to load flag config: " + result.getError());
                }
            } catch (Exception e) {
                throw new RuntimeException("Failed to setup benchmark", e);
            }
        }

        @TearDown(Level.Trial)
        public void tearDown() {
            if (evaluator != null) {
                evaluator.close();
            }
        }
    }

    // Per-thread counter for varying context across threads in targeting benchmarks
    @State(Scope.Thread)
    public static class ThreadState {
        int invocationCount;
    }

    // ========================================================================
    // C7: 16 threads evaluating a simple static boolean flag
    // ========================================================================

    /**
     * C7: 16 threads concurrently evaluating a simple (static) boolean flag.
     *
     * <p>Tests throughput saturation — at 16 threads the synchronized lock
     * contention dominates, revealing the scalability ceiling.
     */
    @Benchmark
    @Threads(16)
    public void c7_simple_16t(SharedEvaluator state, Blackhole bh) {
        try {
            EvaluationResult<Boolean> result = state.evaluator.evaluateFlag(
                Boolean.class, "boolFlag", (String) null);
            bh.consume(result.getValue());
        } catch (Exception e) {
            throw new RuntimeException("C7 benchmark failed", e);
        }
    }

    // ========================================================================
    // C8: 16 threads evaluating a flag with targeting rules
    // ========================================================================

    /**
     * C8: 16 threads concurrently evaluating a flag with targeting rules.
     *
     * <p>Combines heavy synchronized contention with per-evaluation rule
     * processing, measuring how targeting overhead compounds under high
     * parallelism.
     */
    @Benchmark
    @Threads(16)
    public void c8_targeting_16t(SharedEvaluator state, ThreadState ts, Blackhole bh) {
        try {
            // Alternate between admin and viewer roles across invocations
            String role = (ts.invocationCount++ % 2 == 0) ? "admin" : "viewer";
            String context = "{\"role\": \"" + role + "\"}";

            EvaluationResult<String> result = state.evaluator.evaluateFlag(
                String.class, "targetedFlag", context);
            bh.consume(result.getValue());
        } catch (Exception e) {
            throw new RuntimeException("C8 benchmark failed", e);
        }
    }

    // ========================================================================
    // C9: 16 threads with mixed workload (static + targeting + disabled)
    // ========================================================================

    /**
     * C9: 16 threads with mixed workload (static, targeting, and disabled flags).
     *
     * <p>Simulates a realistic high-load production scenario where threads
     * evaluate different flag types concurrently. The workload cycles through
     * static, targeting, and disabled flags.
     */
    @Benchmark
    @Threads(16)
    public void c9_mixed_16t(SharedEvaluator state, ThreadState ts, Blackhole bh) {
        try {
            int pick = ts.invocationCount++ % 4;
            switch (pick) {
                case 0:
                    // Static boolean flag
                    bh.consume(state.evaluator.evaluateFlag(
                        Boolean.class, "boolFlag", (String) null).getValue());
                    break;
                case 1:
                    // Targeting flag (admin)
                    bh.consume(state.evaluator.evaluateFlag(
                        String.class, "targetedFlag", "{\"role\": \"admin\"}").getValue());
                    break;
                case 2:
                    // Disabled flag
                    bh.consume(state.evaluator.evaluateFlag(
                        Boolean.class, "disabledFlag", (String) null).getValue());
                    break;
                case 3:
                    // Targeting flag (viewer)
                    bh.consume(state.evaluator.evaluateFlag(
                        String.class, "targetedFlag", "{\"role\": \"viewer\"}").getValue());
                    break;
            }
        } catch (Exception e) {
            throw new RuntimeException("C9 benchmark failed", e);
        }
    }

    // ========================================================================
    // C10: 15 reader threads + 1 writer thread (read/write contention)
    // ========================================================================

    /**
     * C10: Read/write contention at 16 threads — 15 evaluating while 1 updates state.
     *
     * <p>The writer thread alternates between two configurations, simulating
     * periodic config refreshes under heavy parallel evaluation load. This
     * measures contention between readers and a writer at high thread counts.
     *
     * <p>Uses JMH {@code @Group} to coordinate 15 reader threads and 1 writer thread.
     */
    @Benchmark
    @Group("c10_read_write_16t")
    @GroupThreads(15)
    public void c10_readers(SharedEvaluator state, Blackhole bh) {
        try {
            EvaluationResult<Boolean> result = state.evaluator.evaluateFlag(
                Boolean.class, "boolFlag", (String) null);
            bh.consume(result.getValue());
        } catch (Exception e) {
            throw new RuntimeException("C10 reader failed", e);
        }
    }

    @Benchmark
    @Group("c10_read_write_16t")
    @GroupThreads(1)
    public void c10_writer(SharedEvaluator state, ThreadState ts, Blackhole bh) {
        try {
            // Alternate between two configs to force actual state changes
            String config = (ts.invocationCount++ % 2 == 0) ? BENCH_CONFIG_ALT : BENCH_CONFIG;
            UpdateStateResult result = state.evaluator.updateState(config);
            bh.consume(result.isSuccess());
        } catch (Exception e) {
            throw new RuntimeException("C10 writer failed", e);
        }
    }

    /**
     * Main method to run benchmarks standalone.
     */
    public static void main(String[] args) throws Exception {
        org.openjdk.jmh.Main.main(args);
    }
}
