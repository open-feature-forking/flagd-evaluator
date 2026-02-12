package dev.openfeature.flagd.evaluator;

import dev.openfeature.flagd.evaluator.comparison.MinimalInProcessResolver;
import dev.openfeature.sdk.EvaluationContext;
import dev.openfeature.sdk.ImmutableContext;
import dev.openfeature.sdk.LayeredEvaluationContext;
import dev.openfeature.sdk.ProviderEvaluation;
import dev.openfeature.sdk.Value;
import org.openjdk.jmh.annotations.*;
import org.openjdk.jmh.infra.Blackhole;

import java.util.HashMap;
import java.util.Map;
import java.util.Random;
import java.util.concurrent.TimeUnit;

/**
 * JMH benchmarks comparing old JsonLogic resolver vs new WASM evaluator.
 *
 * <p>Both resolvers use {@link LayeredEvaluationContext} (the production API path),
 * which mirrors how the flagd provider constructs contexts with API, client,
 * invocation, and transaction layers. This enables context key filtering and
 * flag index evaluation on the WASM side.
 *
 * <p>Covers single-threaded comparison (X1, X2), context size sweep (X3),
 * concurrent comparison at 4 threads (X4) and 16 threads (X5).
 *
 * <p><b>Running the benchmarks:</b>
 * <pre>
 * ./mvnw clean package -DskipTests
 * java -jar target/benchmarks.jar ComparisonBenchmark -prof gc
 *
 * # Single-threaded only:
 * java -jar target/benchmarks.jar "ComparisonBenchmark.X[12]_.*"
 *
 * # Concurrent only:
 * java -jar target/benchmarks.jar "ComparisonBenchmark.X[45]_.*"
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

    /** Build a LayeredEvaluationContext matching the flagd provider pattern. */
    private static EvaluationContext layeredContext(
            EvaluationContext apiCtx,
            EvaluationContext clientCtx,
            EvaluationContext invocationCtx) {
        return new LayeredEvaluationContext(apiCtx, clientCtx, invocationCtx, ImmutableContext.EMPTY);
    }

    /** Build an ImmutableContext from a targeting key and attribute map. */
    private static ImmutableContext immutableCtx(String targetingKey, Map<String, Value> attrs) {
        return new ImmutableContext(targetingKey, attrs);
    }

    @State(Scope.Benchmark)
    public static class ComparisonState {
        FlagEvaluator newEvaluator;
        MinimalInProcessResolver oldResolver;

        // Shared API-level context (global attributes like environment/service)
        ImmutableContext apiContext;

        // Layered contexts for benchmarks
        EvaluationContext emptyContext;
        EvaluationContext smallContext;
        EvaluationContext largeContext;

        @Setup(Level.Trial)
        public void setup() {
            try {
                // Setup new WASM evaluator
                newEvaluator = new FlagEvaluator(FlagEvaluator.ValidationMode.PERMISSIVE);
                newEvaluator.updateState(FLAG_CONFIG);

                // Setup old resolver
                oldResolver = new MinimalInProcessResolver();
                oldResolver.loadFlags(FLAG_CONFIG);

                // API-level context (set once at provider init)
                Map<String, Value> apiAttrs = new HashMap<>();
                apiAttrs.put("environment", new Value("production"));
                apiAttrs.put("service", new Value("checkout"));
                apiContext = new ImmutableContext(apiAttrs);

                // Empty invocation context — only API-level attributes
                emptyContext = layeredContext(apiContext, ImmutableContext.EMPTY, ImmutableContext.EMPTY);

                // Small invocation context (4 user attributes)
                Map<String, Value> smallAttrs = new HashMap<>();
                smallAttrs.put("tier", new Value("premium"));
                smallAttrs.put("role", new Value("admin"));
                smallAttrs.put("region", new Value("us-east"));
                smallAttrs.put("score", new Value(85));
                smallContext = layeredContext(apiContext, ImmutableContext.EMPTY,
                    immutableCtx("user-123", smallAttrs));

                // Large invocation context (100+ attributes)
                Map<String, Value> largeAttrs = new HashMap<>();
                largeAttrs.put("tier", new Value("premium"));
                largeAttrs.put("role", new Value("admin"));
                largeAttrs.put("region", new Value("us-east"));
                largeAttrs.put("score", new Value(85));
                for (int i = 0; i < 100; i++) {
                    switch (i % 4) {
                        case 0:
                            largeAttrs.put("attr_" + i, new Value("value-" + i));
                            break;
                        case 1:
                            largeAttrs.put("attr_" + i, new Value(i * 7));
                            break;
                        case 2:
                            largeAttrs.put("attr_" + i, new Value(i % 2 == 0));
                            break;
                        case 3:
                            largeAttrs.put("attr_" + i, new Value(i * 1.5));
                            break;
                    }
                }
                largeContext = layeredContext(apiContext, ImmutableContext.EMPTY,
                    immutableCtx("user-123", largeAttrs));
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
                Boolean.class, "simple-bool", state.emptyContext);
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
                Boolean.class, "targeted-access", state.smallContext);
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
                Boolean.class, "targeted-access", state.emptyContext);
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
                Boolean.class, "targeted-access", state.smallContext);
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
                Boolean.class, "targeted-access", state.largeContext);
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
        EvaluationContext simpleContext;
        EvaluationContext matchingContext;
        EvaluationContext largeMatchingContext;

        @Setup(Level.Trial)
        public void setup() {
            Random random = new Random(42 + Thread.currentThread().getId());
            String targetingKey = "user-" + random.nextInt(10000);

            // API-level context (same as ComparisonState)
            Map<String, Value> apiAttrs = new HashMap<>();
            apiAttrs.put("environment", new Value("production"));
            apiAttrs.put("service", new Value("checkout"));
            ImmutableContext apiCtx = new ImmutableContext(apiAttrs);

            // Simple: only targeting key, no invocation attributes
            simpleContext = layeredContext(apiCtx, ImmutableContext.EMPTY,
                new ImmutableContext(targetingKey));

            // Matching: role + tier that match the targeting rule
            Map<String, Value> matchAttrs = new HashMap<>();
            matchAttrs.put("role", new Value("admin"));
            matchAttrs.put("tier", new Value("premium"));
            matchingContext = layeredContext(apiCtx, ImmutableContext.EMPTY,
                immutableCtx(targetingKey, matchAttrs));

            // Large: 100+ attributes with role + tier
            Map<String, Value> largeAttrs = new HashMap<>();
            largeAttrs.put("role", new Value("admin"));
            largeAttrs.put("tier", new Value("premium"));
            largeAttrs.put("region", new Value("us-east"));
            largeAttrs.put("score", new Value(85));
            for (int i = 0; i < 100; i++) {
                switch (i % 4) {
                    case 0:
                        largeAttrs.put("attr_" + i, new Value("value-" + i));
                        break;
                    case 1:
                        largeAttrs.put("attr_" + i, new Value(i * 7));
                        break;
                    case 2:
                        largeAttrs.put("attr_" + i, new Value(i % 2 == 0));
                        break;
                    case 3:
                        largeAttrs.put("attr_" + i, new Value(i * 1.5));
                        break;
                }
            }
            largeMatchingContext = layeredContext(apiCtx, ImmutableContext.EMPTY,
                immutableCtx(targetingKey, largeAttrs));
        }
    }

    // ========================================================================
    // X4: Old vs New under concurrency (4 threads)
    // ========================================================================

    @Benchmark
    @Threads(4)
    public void X4_old_concurrentSimple(ComparisonState state, ThreadContext ctx, Blackhole bh) {
        ProviderEvaluation<Boolean> result = state.oldResolver.booleanEvaluation(
            "simple-bool", false, ctx.simpleContext);
        bh.consume(result);
    }

    @Benchmark
    @Threads(4)
    public void X4_new_concurrentSimple(ComparisonState state, ThreadContext ctx, Blackhole bh) {
        try {
            EvaluationResult<Boolean> result = state.newEvaluator.evaluateFlag(
                Boolean.class, "simple-bool", ctx.simpleContext);
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

    // ========================================================================
    // X5: Old vs New under high concurrency (16 threads)
    // ========================================================================

    @Benchmark
    @Threads(16)
    public void X5_old_16t_simple(ComparisonState state, ThreadContext ctx, Blackhole bh) {
        ProviderEvaluation<Boolean> result = state.oldResolver.booleanEvaluation(
            "simple-bool", false, ctx.simpleContext);
        bh.consume(result);
    }

    @Benchmark
    @Threads(16)
    public void X5_new_16t_simple(ComparisonState state, ThreadContext ctx, Blackhole bh) {
        try {
            EvaluationResult<Boolean> result = state.newEvaluator.evaluateFlag(
                Boolean.class, "simple-bool", ctx.simpleContext);
            bh.consume(result.getValue());
            bh.consume(result.getVariant());
        } catch (Exception e) {
            throw new RuntimeException("Benchmark failed", e);
        }
    }

    @Benchmark
    @Threads(16)
    public void X5_old_16t_targeting(ComparisonState state, ThreadContext ctx, Blackhole bh) {
        ProviderEvaluation<Boolean> result = state.oldResolver.booleanEvaluation(
            "targeted-access", false, ctx.matchingContext);
        bh.consume(result);
    }

    @Benchmark
    @Threads(16)
    public void X5_new_16t_targeting(ComparisonState state, ThreadContext ctx, Blackhole bh) {
        try {
            EvaluationResult<Boolean> result = state.newEvaluator.evaluateFlag(
                Boolean.class, "targeted-access", ctx.matchingContext);
            bh.consume(result.getValue());
            bh.consume(result.getVariant());
        } catch (Exception e) {
            throw new RuntimeException("Benchmark failed", e);
        }
    }

    @Benchmark
    @Threads(16)
    public void X5_old_16t_largeContext(ComparisonState state, ThreadContext ctx, Blackhole bh) {
        ProviderEvaluation<Boolean> result = state.oldResolver.booleanEvaluation(
            "targeted-access", false, ctx.largeMatchingContext);
        bh.consume(result);
    }

    @Benchmark
    @Threads(16)
    public void X5_new_16t_largeContext(ComparisonState state, ThreadContext ctx, Blackhole bh) {
        try {
            EvaluationResult<Boolean> result = state.newEvaluator.evaluateFlag(
                Boolean.class, "targeted-access", ctx.largeMatchingContext);
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
