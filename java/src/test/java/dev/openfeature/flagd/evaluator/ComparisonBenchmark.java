package dev.openfeature.flagd.evaluator;

import dev.openfeature.flagd.evaluator.comparison.MinimalInProcessResolver;
import dev.openfeature.sdk.EvaluationContext;
import dev.openfeature.sdk.MutableContext;
import dev.openfeature.sdk.ProviderEvaluation;
import org.openjdk.jmh.annotations.*;
import org.openjdk.jmh.infra.Blackhole;

import java.util.Random;
import java.util.concurrent.TimeUnit;

/**
 * JMH benchmarks comparing old JsonLogic resolver vs new WASM evaluator.
 *
 * <p>Covers single-threaded comparison (X1, X2), context size sweep (X3),
 * and concurrent comparison (X4).
 *
 * <p><b>Running the benchmarks:</b>
 * <pre>
 * ./mvnw clean package
 * java -jar target/benchmarks.jar ComparisonBenchmark
 *
 * # Single-threaded only:
 * java -jar target/benchmarks.jar "ComparisonBenchmark.X[12]_.*"
 *
 * # Concurrent only:
 * java -jar target/benchmarks.jar "ComparisonBenchmark.X4_.*"
 * </pre>
 */
@BenchmarkMode({Mode.Throughput, Mode.AverageTime})
@OutputTimeUnit(TimeUnit.MICROSECONDS)
@Fork(1)
@Warmup(iterations = 3, time = 2)
@Measurement(iterations = 5, time = 3)
public class ComparisonBenchmark {

    // Flag configuration with simple and targeting flags
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
        "    }\n" +
        "  }\n" +
        "}";

    // Context JSON strings for the new evaluator
    private static final String EMPTY_CONTEXT_JSON = "{}";

    private static final String SMALL_CONTEXT_JSON = "{" +
        "\"targetingKey\": \"user-123\", " +
        "\"tier\": \"premium\", " +
        "\"role\": \"admin\", " +
        "\"region\": \"us-east\", " +
        "\"score\": 85" +
        "}";

    @State(Scope.Benchmark)
    public static class ComparisonState {
        FlagEvaluator newEvaluator;
        MinimalInProcessResolver oldResolver;

        // EvaluationContext objects for the old resolver
        EvaluationContext emptyContext;
        EvaluationContext smallContext;
        EvaluationContext largeContext;

        // JSON strings for the new evaluator
        String largeContextJson;

        @Setup(Level.Trial)
        public void setup() {
            try {
                // Setup new WASM evaluator
                newEvaluator = new FlagEvaluator(FlagEvaluator.ValidationMode.PERMISSIVE);
                newEvaluator.updateState(FLAG_CONFIG);

                // Setup old resolver
                oldResolver = new MinimalInProcessResolver();
                oldResolver.loadFlags(FLAG_CONFIG);

                // Build contexts for old resolver
                emptyContext = new MutableContext();

                smallContext = new MutableContext()
                    .add("targetingKey", "user-123")
                    .add("tier", "premium")
                    .add("role", "admin")
                    .add("region", "us-east")
                    .add("score", 85);

                MutableContext large = new MutableContext()
                    .add("targetingKey", "user-123")
                    .add("tier", "premium")
                    .add("role", "admin")
                    .add("region", "us-east")
                    .add("score", 85);
                for (int i = 0; i < 100; i++) {
                    switch (i % 4) {
                        case 0:
                            large.add("attr_" + i, "value-" + i);
                            break;
                        case 1:
                            large.add("attr_" + i, i * 7);
                            break;
                        case 2:
                            large.add("attr_" + i, i % 2 == 0);
                            break;
                        case 3:
                            large.add("attr_" + i, i * 1.5);
                            break;
                    }
                }
                largeContext = large;

                // Build large context JSON for the new evaluator
                StringBuilder sb = new StringBuilder(4096);
                sb.append("{");
                sb.append("\"targetingKey\": \"user-123\", ");
                sb.append("\"tier\": \"premium\", ");
                sb.append("\"role\": \"admin\", ");
                sb.append("\"region\": \"us-east\", ");
                sb.append("\"score\": 85");
                for (int i = 0; i < 100; i++) {
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
                largeContextJson = sb.toString();
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

    // ========================================================================
    // X1: Old vs New - Simple flag evaluation (single-threaded)
    // ========================================================================

    @Benchmark
    @Threads(1)
    public void X1_old_simple(ComparisonState state, Blackhole bh) {
        ProviderEvaluation<Boolean> result = state.oldResolver.booleanEvaluation(
            "simple-bool", false, state.emptyContext);
        bh.consume(result);
    }

    @Benchmark
    @Threads(1)
    public void X1_new_simple(ComparisonState state, Blackhole bh) {
        try {
            EvaluationResult<Boolean> result = state.newEvaluator.evaluateFlag(
                Boolean.class, "simple-bool", EMPTY_CONTEXT_JSON);
            bh.consume(result.getValue());
            bh.consume(result.getVariant());
        } catch (Exception e) {
            throw new RuntimeException("Benchmark failed", e);
        }
    }

    // ========================================================================
    // X2: Old vs New - Targeting evaluation (single-threaded)
    // ========================================================================

    @Benchmark
    @Threads(1)
    public void X2_old_targeting(ComparisonState state, Blackhole bh) {
        ProviderEvaluation<Boolean> result = state.oldResolver.booleanEvaluation(
            "targeted-access", false, state.smallContext);
        bh.consume(result);
    }

    @Benchmark
    @Threads(1)
    public void X2_new_targeting(ComparisonState state, Blackhole bh) {
        try {
            EvaluationResult<Boolean> result = state.newEvaluator.evaluateFlag(
                Boolean.class, "targeted-access", SMALL_CONTEXT_JSON);
            bh.consume(result.getValue());
            bh.consume(result.getVariant());
        } catch (Exception e) {
            throw new RuntimeException("Benchmark failed", e);
        }
    }

    // ========================================================================
    // X3: Context size sweep - Old resolver (empty, small, large)
    // ========================================================================

    @Benchmark
    @Threads(1)
    public void X3_old_emptyContext(ComparisonState state, Blackhole bh) {
        ProviderEvaluation<Boolean> result = state.oldResolver.booleanEvaluation(
            "targeted-access", false, state.emptyContext);
        bh.consume(result);
    }

    @Benchmark
    @Threads(1)
    public void X3_old_smallContext(ComparisonState state, Blackhole bh) {
        ProviderEvaluation<Boolean> result = state.oldResolver.booleanEvaluation(
            "targeted-access", false, state.smallContext);
        bh.consume(result);
    }

    @Benchmark
    @Threads(1)
    public void X3_old_largeContext(ComparisonState state, Blackhole bh) {
        ProviderEvaluation<Boolean> result = state.oldResolver.booleanEvaluation(
            "targeted-access", false, state.largeContext);
        bh.consume(result);
    }

    // ========================================================================
    // X3: Context size sweep - New evaluator (empty, small, large)
    // ========================================================================

    @Benchmark
    @Threads(1)
    public void X3_new_emptyContext(ComparisonState state, Blackhole bh) {
        try {
            EvaluationResult<Boolean> result = state.newEvaluator.evaluateFlag(
                Boolean.class, "targeted-access", EMPTY_CONTEXT_JSON);
            bh.consume(result.getValue());
            bh.consume(result.getVariant());
        } catch (Exception e) {
            throw new RuntimeException("Benchmark failed", e);
        }
    }

    @Benchmark
    @Threads(1)
    public void X3_new_smallContext(ComparisonState state, Blackhole bh) {
        try {
            EvaluationResult<Boolean> result = state.newEvaluator.evaluateFlag(
                Boolean.class, "targeted-access", SMALL_CONTEXT_JSON);
            bh.consume(result.getValue());
            bh.consume(result.getVariant());
        } catch (Exception e) {
            throw new RuntimeException("Benchmark failed", e);
        }
    }

    @Benchmark
    @Threads(1)
    public void X3_new_largeContext(ComparisonState state, Blackhole bh) {
        try {
            EvaluationResult<Boolean> result = state.newEvaluator.evaluateFlag(
                Boolean.class, "targeted-access", state.largeContextJson);
            bh.consume(result.getValue());
            bh.consume(result.getVariant());
        } catch (Exception e) {
            throw new RuntimeException("Benchmark failed", e);
        }
    }

    // ========================================================================
    // Thread-local context for concurrent benchmarks
    // ========================================================================

    @State(Scope.Thread)
    public static class ThreadContext {
        Random random;
        String simpleContextJson;
        EvaluationContext matchingContext;

        @Setup(Level.Trial)
        public void setup() {
            random = new Random(42 + Thread.currentThread().getId());
            simpleContextJson = "{\"targetingKey\": \"user-" + random.nextInt(10000) + "\"}";
            matchingContext = new MutableContext()
                .add("role", "admin")
                .add("tier", "premium")
                .add("targetingKey", "user-" + random.nextInt(10000));
        }
    }

    // ========================================================================
    // X4: Old vs New under concurrency (4 threads)
    // ========================================================================

    @Benchmark
    @Threads(4)
    public void X4_old_concurrentSimple(ComparisonState state, ThreadContext ctx, Blackhole bh) {
        ProviderEvaluation<Boolean> result = state.oldResolver.booleanEvaluation(
            "simple-bool", false, state.emptyContext);
        bh.consume(result);
    }

    @Benchmark
    @Threads(4)
    public void X4_new_concurrentSimple(ComparisonState state, ThreadContext ctx, Blackhole bh) {
        try {
            EvaluationResult<Boolean> result = state.newEvaluator.evaluateFlag(
                Boolean.class, "simple-bool", ctx.simpleContextJson);
            bh.consume(result.getValue());
            bh.consume(result.getVariant());
        } catch (Exception e) {
            throw new RuntimeException("Benchmark failed", e);
        }
    }

    @Benchmark
    @Threads(4)
    public void X4_old_concurrentTargeting(ComparisonState state, ThreadContext ctx, Blackhole bh) {
        ProviderEvaluation<Boolean> result = state.oldResolver.booleanEvaluation(
            "targeted-access", false, ctx.matchingContext);
        bh.consume(result);
    }

    @Benchmark
    @Threads(4)
    public void X4_new_concurrentTargeting(ComparisonState state, ThreadContext ctx, Blackhole bh) {
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
