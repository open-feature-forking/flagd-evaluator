package dev.openfeature.flagd.evaluator.comparison;

import dev.openfeature.flagd.evaluator.EvaluationResult;
import dev.openfeature.flagd.evaluator.FlagEvaluator;
import dev.openfeature.sdk.EvaluationContext;
import dev.openfeature.sdk.MutableContext;
import dev.openfeature.sdk.ProviderEvaluation;
import org.openjdk.jmh.annotations.*;
import org.openjdk.jmh.infra.Blackhole;

import java.util.concurrent.TimeUnit;

/**
 * JMH benchmark comparing old JsonLogic-based resolver vs new WASM-based evaluator.
 *
 * <p>This benchmark measures:
 * - Throughput (operations per second)
 * - Latency (average time per operation)
 * - Memory allocation (GC behavior)
 *
 * <p>Run with: ./mvnw exec:java@run-jmh-benchmark -Dbenchmark=ResolverComparisonBenchmark
 */
@BenchmarkMode({Mode.Throughput, Mode.AverageTime})
@OutputTimeUnit(TimeUnit.MICROSECONDS)
@State(Scope.Thread)
@Fork(value = 1, warmups = 1)
@Warmup(iterations = 3, time = 2)
@Measurement(iterations = 5, time = 2)
public class ResolverComparisonBenchmark {

    private MinimalInProcessResolver oldResolver;
    private FlagEvaluator newEvaluator;

    // Test contexts
    private EvaluationContext emptyContext;
    private EvaluationContext matchingContext;
    private EvaluationContext nonMatchingContext;

    @Setup(Level.Trial)
    public void setup() throws Exception {
        // Initialize both resolvers
        oldResolver = new MinimalInProcessResolver();
        newEvaluator = new FlagEvaluator(FlagEvaluator.ValidationMode.PERMISSIVE);

        // Unified configuration with all flags
        String unifiedConfig = "{\n" +
            "  \"flags\": {\n" +
            "    \"simple-flag\": {\n" +
            "      \"state\": \"ENABLED\",\n" +
            "      \"defaultVariant\": \"on\",\n" +
            "      \"variants\": {\n" +
            "        \"on\": true,\n" +
            "        \"off\": false\n" +
            "      }\n" +
            "    },\n" +
            "    \"feature-access\": {\n" +
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
            "              {\n" +
            "                \"==\": [\n" +
            "                  { \"var\": \"role\" },\n" +
            "                  \"admin\"\n" +
            "                ]\n" +
            "              },\n" +
            "              {\n" +
            "                \"in\": [\n" +
            "                  { \"var\": \"tier\" },\n" +
            "                  [\"premium\", \"enterprise\"]\n" +
            "                ]\n" +
            "              }\n" +
            "            ]\n" +
            "          },\n" +
            "          \"granted\",\n" +
            "          null\n" +
            "        ]\n" +
            "      }\n" +
            "    }\n" +
            "  }\n" +
            "}";

        // Pre-load configuration ONCE for both implementations
        oldResolver.loadFlags(unifiedConfig);
        newEvaluator.updateState(unifiedConfig);

        // Contexts
        emptyContext = new MutableContext();
        matchingContext = new MutableContext()
            .add("role", "admin")
            .add("tier", "premium");
        nonMatchingContext = new MutableContext()
            .add("role", "user")
            .add("tier", "basic");
    }

    // ========== Simple Flag Evaluation (No Targeting) ==========

    @Benchmark
    public void oldResolver_SimpleFlag(Blackhole blackhole) {
        ProviderEvaluation<Boolean> result = oldResolver.booleanEvaluation("simple-flag", false, emptyContext);
        blackhole.consume(result);
    }

    @Benchmark
    public void newEvaluator_SimpleFlag(Blackhole blackhole) {
        try {
            EvaluationResult<Boolean> result = newEvaluator.evaluateFlag(Boolean.class, "simple-flag", emptyContext);
            blackhole.consume(result);
        } catch (Exception e) {
            throw new RuntimeException("Evaluation failed", e);
        }
    }

    @Benchmark
    public void newEvaluator_SimpleFlag_Binary(Blackhole blackhole) {
        try {
            EvaluationResult<Boolean> result = newEvaluator.evaluateFlagBinary(Boolean.class, "simple-flag", emptyContext);
            blackhole.consume(result);
        } catch (Exception e) {
            throw new RuntimeException("Evaluation failed", e);
        }
    }

    // ========== Complex Targeting Evaluation (Match) ==========

    @Benchmark
    public void oldResolver_TargetingMatch(Blackhole blackhole) {
        ProviderEvaluation<Boolean> result = oldResolver.booleanEvaluation("feature-access", false, matchingContext);
        blackhole.consume(result);
    }

    @Benchmark
    public void newEvaluator_TargetingMatch(Blackhole blackhole) {
        try {
            EvaluationResult<Boolean> result = newEvaluator.evaluateFlag(Boolean.class, "feature-access", matchingContext);
            blackhole.consume(result);
        } catch (Exception e) {
            throw new RuntimeException("Evaluation failed", e);
        }
    }

    @Benchmark
    public void newEvaluator_TargetingMatch_Binary(Blackhole blackhole) {
        try {
            EvaluationResult<Boolean> result = newEvaluator.evaluateFlagBinary(Boolean.class, "feature-access", matchingContext);
            blackhole.consume(result);
        } catch (Exception e) {
            throw new RuntimeException("Evaluation failed", e);
        }
    }

    // ========== Complex Targeting Evaluation (No Match) ==========

    @Benchmark
    public void oldResolver_TargetingNoMatch(Blackhole blackhole) {
        ProviderEvaluation<Boolean> result = oldResolver.booleanEvaluation("feature-access", false, nonMatchingContext);
        blackhole.consume(result);
    }

    @Benchmark
    public void newEvaluator_TargetingNoMatch(Blackhole blackhole) {
        try {
            EvaluationResult<Boolean> result = newEvaluator.evaluateFlag(Boolean.class, "feature-access", nonMatchingContext);
            blackhole.consume(result);
        } catch (Exception e) {
            throw new RuntimeException("Evaluation failed", e);
        }
    }

    // ========== Empty Context Tests (No Serialization Overhead) ==========

    @Benchmark
    public void oldResolver_EmptyContext(Blackhole blackhole) {
        ProviderEvaluation<Boolean> result = oldResolver.booleanEvaluation("simple-flag", false, emptyContext);
        blackhole.consume(result);
    }

    @Benchmark
    public void newEvaluator_EmptyContext(Blackhole blackhole) {
        try {
            EvaluationResult<Boolean> result = newEvaluator.evaluateFlag(Boolean.class, "simple-flag", emptyContext);
            blackhole.consume(result);
        } catch (Exception e) {
            throw new RuntimeException("Evaluation failed", e);
        }
    }

    // ========== GC Profiling (Many Evaluations) ==========

    @Benchmark
    @OperationsPerInvocation(1000)
    public void oldResolver_ManyEvaluations(Blackhole blackhole) {
        for (int i = 0; i < 1000; i++) {
            ProviderEvaluation<Boolean> result = oldResolver.booleanEvaluation("feature-access", false, matchingContext);
            blackhole.consume(result);
        }
    }

    @Benchmark
    @OperationsPerInvocation(1000)
    public void newEvaluator_ManyEvaluations(Blackhole blackhole) {
        try {
            for (int i = 0; i < 1000; i++) {
                EvaluationResult<Boolean> result = newEvaluator.evaluateFlag(Boolean.class, "feature-access", matchingContext);
                blackhole.consume(result);
            }
        } catch (Exception e) {
            throw new RuntimeException("Evaluation failed", e);
        }
    }
}
