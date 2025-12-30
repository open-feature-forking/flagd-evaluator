package dev.openfeature.flagd.evaluator;

import com.fasterxml.jackson.databind.ObjectMapper;
import dev.openfeature.sdk.EvaluationContext;
import dev.openfeature.sdk.ImmutableContext;
import dev.openfeature.sdk.LayeredEvaluationContext;
import dev.openfeature.sdk.Value;
import org.junit.jupiter.api.BeforeAll;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;

import java.util.HashMap;
import java.util.Map;
import java.util.Random;
import java.util.concurrent.TimeUnit;

import static org.assertj.core.api.Assertions.assertThat;

/**
 * Benchmark tests for FlagEvaluator to track performance over time.
 *
 * <p>This test performs 1000 flag evaluations with layered contexts to measure
 * and track evaluation performance. The results should be monitored to ensure
 * no performance regressions occur.
 */
class FlagEvaluatorBenchmarkTest {

    private static final int WARMUP_ITERATIONS = 100;
    private static final int BENCHMARK_ITERATIONS = 10000;
    private static final int CONTEXT_ENTRIES_PER_LAYER = 1000;

    private static final Random RANDOM = new Random(42); // Fixed seed for reproducibility
    private static final ObjectMapper OBJECT_MAPPER = new ObjectMapper();

    // Static contexts shared across all evaluations
    private static EvaluationContext apiContext;
    private static EvaluationContext transactionContext;
    private static EvaluationContext clientContext;

    private FlagEvaluator evaluator;

    @BeforeAll
    static void setUpStatic() throws InstantiationException {
        // Create API context with 100 entries
        apiContext = createContextWithEntries("api", CONTEXT_ENTRIES_PER_LAYER);

        // Create transaction context with 100 entries (some overlap with API)
        transactionContext = createContextWithEntries("transaction", CONTEXT_ENTRIES_PER_LAYER);

        // Create client context with 100 entries (some overlap with API and transaction)
        clientContext = createContextWithEntries("client", CONTEXT_ENTRIES_PER_LAYER);
    }

    @BeforeEach
    void setUp() throws EvaluatorException {
        evaluator = new FlagEvaluator(FlagEvaluator.ValidationMode.PERMISSIVE);

        // Load flag configuration with targeting rules
        String config = "{\n" +
            "  \"flags\": {\n" +
            "    \"benchmark-flag\": {\n" +
            "      \"state\": \"ENABLED\",\n" +
            "      \"defaultVariant\": \"default\",\n" +
            "      \"variants\": {\n" +
            "        \"default\": false,\n" +
            "        \"premium\": true\n" +
            "      },\n" +
            "      \"targeting\": {\n" +
            "        \"if\": [\n" +
            "          {\n" +
            "            \"in\": [\n" +
            "              { \"var\": \"tier\" },\n" +
            "              [\"gold\", \"platinum\"]\n" +
            "            ]\n" +
            "          },\n" +
            "          \"premium\",\n" +
            "          \"default\"\n" +
            "        ]\n" +
            "      }\n" +
            "    }\n" +
            "  }\n" +
            "}";

        UpdateStateResult updateResult = evaluator.updateState(config);
        assertThat(updateResult.isSuccess()).isTrue();
    }

    @Test
    void benchmarkLayeredContextEvaluation() throws Exception {
        System.out.println("\n=== FlagEvaluator Benchmark ===");
        System.out.println("Warmup iterations: " + WARMUP_ITERATIONS);
        System.out.println("Benchmark iterations: " + BENCHMARK_ITERATIONS);
        System.out.println("Context entries per layer: " + CONTEXT_ENTRIES_PER_LAYER);
        System.out.println("Layers: API, Transaction, Client, Invocation (random)");

        // Warmup phase
        System.out.println("\nWarming up...");
        for (int i = 0; i < WARMUP_ITERATIONS; i++) {
            performEvaluation();
        }

        // Benchmark phase
        System.out.println("Running benchmark...");
        long startTime = System.nanoTime();

        for (int i = 0; i < BENCHMARK_ITERATIONS; i++) {
            performEvaluation();
        }

        long endTime = System.nanoTime();
        long durationNanos = endTime - startTime;

        // Calculate statistics
        double durationMs = TimeUnit.NANOSECONDS.toMillis(durationNanos);
        double avgTimePerEvalMs = durationMs / BENCHMARK_ITERATIONS;
        double avgTimePerEvalMicros = TimeUnit.NANOSECONDS.toMicros(durationNanos) / (double) BENCHMARK_ITERATIONS;
        double evaluationsPerSecond = (BENCHMARK_ITERATIONS * 1000.0) / durationMs;

        // Print results
        System.out.println("\n=== Results ===");
        System.out.printf("Total time: %.2f ms%n", durationMs);
        System.out.printf("Average time per evaluation: %.3f ms (%.2f µs)%n", avgTimePerEvalMs, avgTimePerEvalMicros);
        System.out.printf("Evaluations per second: %.2f%n", evaluationsPerSecond);

        // Performance assertions - adjust these thresholds based on expected performance
        // These are reasonable targets for WASM-based evaluation with JIT compilation
        assertThat(avgTimePerEvalMs).as("Average evaluation time should be under 5ms").isLessThan(5.0);
        assertThat(evaluationsPerSecond).as("Should achieve at least 200 evaluations/second").isGreaterThan(200.0);

        System.out.println("\n✅ Benchmark completed successfully");
        System.out.println("=====================================\n");
    }

    @Test
    void benchmarkConcurrentEvaluation() throws Exception {
        System.out.println("\n=== FlagEvaluator Concurrent Benchmark ===");

        int threadCount = 4;
        int iterationsPerThread = 250; // 4 threads * 250 = 1000 total

        System.out.println("Threads: " + threadCount);
        System.out.println("Iterations per thread: " + iterationsPerThread);
        System.out.println("Total iterations: " + (threadCount * iterationsPerThread));

        // Warmup
        for (int i = 0; i < WARMUP_ITERATIONS; i++) {
            performEvaluation();
        }

        // Concurrent benchmark
        System.out.println("\nRunning concurrent benchmark...");
        Thread[] threads = new Thread[threadCount];
        long[] threadDurations = new long[threadCount];

        long startTime = System.nanoTime();

        for (int t = 0; t < threadCount; t++) {
            final int threadIndex = t;
            threads[t] = new Thread(() -> {
                long threadStart = System.nanoTime();
                for (int i = 0; i < iterationsPerThread; i++) {
                    try {
                        performEvaluation();
                    } catch (Exception e) {
                        throw new RuntimeException("Evaluation failed in thread " + threadIndex, e);
                    }
                }
                threadDurations[threadIndex] = System.nanoTime() - threadStart;
            });
            threads[t].start();
        }

        // Wait for all threads to complete
        for (Thread thread : threads) {
            thread.join();
        }

        long endTime = System.nanoTime();
        long totalDurationNanos = endTime - startTime;

        // Calculate statistics
        double totalDurationMs = TimeUnit.NANOSECONDS.toMillis(totalDurationNanos);
        double throughput = (threadCount * iterationsPerThread * 1000.0) / totalDurationMs;

        System.out.println("\n=== Results ===");
        System.out.printf("Total wall-clock time: %.2f ms%n", totalDurationMs);
        System.out.printf("Total throughput: %.2f evaluations/second%n", throughput);

        // Per-thread statistics
        System.out.println("\nPer-thread statistics:");
        for (int i = 0; i < threadCount; i++) {
            double threadMs = TimeUnit.NANOSECONDS.toMillis(threadDurations[i]);
            double avgMs = threadMs / iterationsPerThread;
            System.out.printf("  Thread %d: %.2f ms total, %.3f ms avg%n", i, threadMs, avgMs);
        }

        // Performance assertions for concurrent execution
        assertThat(throughput).as("Concurrent throughput should be at least 400 eval/sec").isGreaterThan(400.0);

        System.out.println("\n✅ Concurrent benchmark completed successfully");
        System.out.println("=====================================\n");
    }

    /**
     * Performs a single evaluation with a layered context.
     * Creates a new random invocation context for each call.
     */
    private void performEvaluation() throws Exception {
        // Create random invocation context
        EvaluationContext invocationContext = createRandomContext();

        // Build layered context: API -> Transaction -> Client -> Invocation
        LayeredEvaluationContext layeredContext = new LayeredEvaluationContext(
            apiContext,
            transactionContext,
            clientContext,
            invocationContext
        );

        // Serialize to JSON for evaluation
        String contextJson = OBJECT_MAPPER.writeValueAsString(layeredContext);

        // Perform evaluation
        EvaluationResult<Boolean> result = evaluator.evaluateFlag(Boolean.class, "benchmark-flag", contextJson);

        // Verify result is valid
        assertThat(result).isNotNull();
        assertThat(result.getValue()).isNotNull();
    }

    /**
     * Creates an evaluation context with the specified number of entries.
     * Keys are prefixed with the given prefix to allow for overlapping keys across contexts.
     */
    private static EvaluationContext createContextWithEntries(String prefix, int count) throws InstantiationException {
        Map<String, Value> attributes = new HashMap<>();

        for (int i = 0; i < count; i++) {
            // Create some overlap by using both prefixed and non-prefixed keys
            if (i % 3 == 0) {
                // Non-prefixed key (will overlap across contexts)
                attributes.put("key" + i, new Value(generateRandomValue(i)));
            } else {
                // Prefixed key (unique to this context)
                attributes.put(prefix + ".key" + i, new Value(generateRandomValue(i)));
            }
        }

        // Add some structured data
        attributes.put("id", new Value(prefix + "-user-123"));
        attributes.put("tier", new Value(RANDOM.nextBoolean() ? "gold" : "silver"));
        attributes.put("score", new Value(RANDOM.nextInt(1000)));

        return new ImmutableContext("benchmark-key-" + prefix, attributes);
    }

    /**
     * Creates a random evaluation context with varying attributes.
     */
    private static EvaluationContext createRandomContext() throws InstantiationException {
        Map<String, Value> attributes = new HashMap<>();

        // Add some random entries (fewer than static contexts)
        int entryCount = RANDOM.nextInt(20) + 10; // 10-30 entries
        for (int i = 0; i < entryCount; i++) {
            attributes.put("invocation.key" + i, new Value(generateRandomValue(i)));
        }

        // Add random user data
        attributes.put("id", new Value("user-" + RANDOM.nextInt(1000)));
        attributes.put("tier", new Value(RANDOM.nextBoolean() ? "platinum" : "bronze"));
        attributes.put("region", new Value(pickRandom("us-east", "us-west", "eu-central", "ap-south")));
        attributes.put("timestamp", new Value(System.currentTimeMillis()));

        return new ImmutableContext("invocation-key-" + RANDOM.nextInt(10000), attributes);
    }

    /**
     * Generates a random value based on the index.
     */
    private static Object generateRandomValue(int index) {
        int type = index % 5;
        switch (type) {
            case 0:
                return RANDOM.nextBoolean();
            case 1:
                return RANDOM.nextInt(1000);
            case 2:
                return RANDOM.nextDouble() * 100;
            case 3:
                return "string-value-" + RANDOM.nextInt(100);
            default:
                return RANDOM.nextLong();
        }
    }

    /**
     * Picks a random element from the given array.
     */
    private static String pickRandom(String... options) {
        return options[RANDOM.nextInt(options.length)];
    }
}
