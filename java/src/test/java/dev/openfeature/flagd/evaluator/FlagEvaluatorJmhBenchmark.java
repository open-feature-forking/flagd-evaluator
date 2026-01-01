package dev.openfeature.flagd.evaluator;

import com.fasterxml.jackson.databind.ObjectMapper;
import com.fasterxml.jackson.databind.module.SimpleModule;
import dev.openfeature.flagd.evaluator.jackson.EvaluationContextSerializer;
import dev.openfeature.sdk.EvaluationContext;
import dev.openfeature.sdk.ImmutableContext;
import dev.openfeature.sdk.LayeredEvaluationContext;
import dev.openfeature.sdk.Value;
import org.openjdk.jmh.annotations.*;
import org.openjdk.jmh.infra.Blackhole;

import java.util.HashMap;
import java.util.Map;
import java.util.Random;
import java.util.concurrent.TimeUnit;

import static dev.openfeature.flagd.evaluator.FlagEvaluator.OBJECT_MAPPER;

/**
 * JMH Benchmark for FlagEvaluator to measure and track performance over time.
 *
 * <p>This benchmark uses layered evaluation contexts with 100 entries per layer
 * (API, Transaction, Client) plus a random invocation context to simulate
 * realistic flag evaluation scenarios.
 *
 * <p><b>Running the benchmark:</b>
 * <pre>
 * mvn test-compile exec:java -Dexec.classpathScope=test \
 *   -Dexec.mainClass=org.openjdk.jmh.Main \
 *   -Dexec.args="FlagEvaluatorJmhBenchmark"
 * </pre>
 *
 * <p>Or create a benchmark JAR:
 * <pre>
 * mvn clean package
 * java -jar target/benchmarks.jar FlagEvaluatorJmhBenchmark
 * </pre>
 */
@BenchmarkMode(Mode.Throughput)
@OutputTimeUnit(TimeUnit.SECONDS)
@State(Scope.Benchmark)
@Fork(value = 1, warmups = 1)
@Warmup(iterations = 3, time = 2)
@Measurement(iterations = 5, time = 3)
public class FlagEvaluatorJmhBenchmark {

    private static final int CONTEXT_ENTRIES_PER_LAYER = 100;

    // Static contexts shared across all evaluations
    private EvaluationContext apiContext;
    private EvaluationContext transactionContext;
    private EvaluationContext clientContext;

    private FlagEvaluator evaluator;

    // Thread-local random for thread-safe random context generation
    @State(Scope.Thread)
    public static class RandomState {
        Random random = new Random(42);
    }

    @Setup(Level.Trial)
    public void setupTrial() {
        try {
            // Create static contexts (API, Transaction, Client layers)
            apiContext = createContextWithEntries("api", CONTEXT_ENTRIES_PER_LAYER);
            transactionContext = createContextWithEntries("transaction", CONTEXT_ENTRIES_PER_LAYER);
            clientContext = createContextWithEntries("client", CONTEXT_ENTRIES_PER_LAYER);

            // Create evaluator and load flag configuration
            evaluator = new FlagEvaluator(FlagEvaluator.ValidationMode.PERMISSIVE);

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
            if (!updateResult.isSuccess()) {
                throw new RuntimeException("Failed to update flag state: " + updateResult.getError());
            }
        } catch (Exception e) {
            throw new RuntimeException("Failed to setup benchmark", e);
        }
    }

    @TearDown(Level.Trial)
    public void tearDownTrial() throws Exception {
        if (evaluator != null) {
            evaluator.close();
        }
    }

    /**
     * Benchmark: Single flag evaluation with layered context.
     * Measures throughput of flag evaluations with realistic context sizes.
     */
    @Benchmark
    public void evaluateWithLayeredContext(RandomState randomState, Blackhole blackhole) {
        try {
            // Create random invocation context
            EvaluationContext invocationContext = createRandomContext(randomState.random);

            // Build layered context: API -> Transaction -> Client -> Invocation
            LayeredEvaluationContext layeredContext = new LayeredEvaluationContext(
                apiContext,
                transactionContext,
                clientContext,
                invocationContext
            );

            // Perform evaluation
            EvaluationResult<Boolean> result = evaluator.evaluateFlag(Boolean.class, "benchmark-flag", layeredContext);

            // Consume result to prevent dead code elimination
            blackhole.consume(result.getValue());
            blackhole.consume(result.getVariant());
        } catch (Exception e) {
            throw new RuntimeException("Benchmark failed", e);
        }
    }

    /**
     * Benchmark: Flag evaluation with simple context.
     * Measures baseline performance with minimal context overhead.
     */
    @Benchmark
    public void evaluateWithSimpleContext(Blackhole blackhole) {
        try {
            String simpleContext = "{\"targetingKey\": \"user-123\"}";

            EvaluationResult<Boolean> result = evaluator.evaluateFlag(Boolean.class, "benchmark-flag", simpleContext);

            blackhole.consume(result.getValue());
            blackhole.consume(result.getVariant());
        } catch (Exception e) {
            throw new RuntimeException("Benchmark failed", e);
        }
    }

    /**
     * Benchmark: Flag evaluation with binary protocol (protobuf).
     * Measures baseline performance using the faster binary protocol.
     */
    @Benchmark
    public void evaluateWithSimpleContextBinary(Blackhole blackhole) {
        try {
            String simpleContext = "{\"targetingKey\": \"user-123\"}";

            EvaluationResult<Boolean> result = evaluator.evaluateFlagBinary(Boolean.class, "benchmark-flag", simpleContext);

            blackhole.consume(result.getValue());
            blackhole.consume(result.getVariant());
        } catch (Exception e) {
            throw new RuntimeException("Benchmark failed", e);
        }
    }

    /**
     * Benchmark: Flag evaluation with layered context using binary protocol.
     * Measures throughput of binary flag evaluations with realistic context sizes.
     */
    @Benchmark
    public void evaluateWithLayeredContextBinary(RandomState randomState, Blackhole blackhole) {
        try {
            // Create random invocation context
            EvaluationContext invocationContext = createRandomContext(randomState.random);

            // Build layered context: API -> Transaction -> Client -> Invocation
            LayeredEvaluationContext layeredContext = new LayeredEvaluationContext(
                apiContext,
                transactionContext,
                clientContext,
                invocationContext
            );

            // Perform evaluation using binary protocol
            EvaluationResult<Boolean> result = evaluator.evaluateFlagBinary(Boolean.class, "benchmark-flag", layeredContext);

            // Consume result to prevent dead code elimination
            blackhole.consume(result.getValue());
            blackhole.consume(result.getVariant());
        } catch (Exception e) {
            throw new RuntimeException("Benchmark failed", e);
        }
    }

    /**
     * Benchmark: Context serialization only.
     * Measures the overhead of serializing layered contexts to JSON.
     */
    @Benchmark
    public void serializeLayeredContext(RandomState randomState, Blackhole blackhole) {
        try {
            EvaluationContext invocationContext = createRandomContext(randomState.random);

            LayeredEvaluationContext layeredContext = new LayeredEvaluationContext(
                apiContext,
                transactionContext,
                clientContext,
                invocationContext
            );

            String contextJson = OBJECT_MAPPER.writeValueAsString(layeredContext);

            blackhole.consume(contextJson);
        } catch (Exception e) {
            throw new RuntimeException("Benchmark failed", e);
        }
    }

    // ========================================================================
    // Helper Methods
    // ========================================================================

    /**
     * Creates an evaluation context with the specified number of entries.
     */
    private static EvaluationContext createContextWithEntries(String prefix, int count) throws InstantiationException {
        Map<String, Value> attributes = new HashMap<>();
        Random random = new Random(prefix.hashCode()); // Deterministic per prefix

        for (int i = 0; i < count; i++) {
            // Create some overlap by using both prefixed and non-prefixed keys
            if (i % 3 == 0) {
                // Non-prefixed key (will overlap across contexts)
                attributes.put("key" + i, new Value(generateRandomValue(i, random)));
            } else {
                // Prefixed key (unique to this context)
                attributes.put(prefix + ".key" + i, new Value(generateRandomValue(i, random)));
            }
        }

        // Add some user data directly to attributes
        attributes.put("id", new Value(prefix + "-user-123"));
        attributes.put("tier", new Value(random.nextBoolean() ? "gold" : "silver"));
        attributes.put("score", new Value(random.nextInt(1000)));

        return new ImmutableContext("benchmark-key-" + prefix, attributes);
    }

    /**
     * Creates a random evaluation context with varying attributes.
     */
    private static EvaluationContext createRandomContext(Random random) throws InstantiationException {
        Map<String, Value> attributes = new HashMap<>();

        // Add some random entries (fewer than static contexts)
        int entryCount = random.nextInt(20) + 10; // 10-30 entries
        for (int i = 0; i < entryCount; i++) {
            attributes.put("invocation.key" + i, new Value(generateRandomValue(i, random)));
        }

        // Add random user data directly to attributes
        attributes.put("id", new Value("user-" + random.nextInt(1000)));
        attributes.put("tier", new Value(random.nextBoolean() ? "platinum" : "bronze"));
        attributes.put("region", new Value(pickRandom(random, "us-east", "us-west", "eu-central", "ap-south")));
        attributes.put("timestamp", new Value(System.currentTimeMillis()));

        return new ImmutableContext("invocation-key-" + random.nextInt(10000), attributes);
    }

    /**
     * Generates a random value based on the index.
     */
    private static Object generateRandomValue(int index, Random random) {
        int type = index % 5;
        switch (type) {
            case 0:
                return random.nextBoolean();
            case 1:
                return random.nextInt(1000);
            case 2:
                return random.nextDouble() * 100;
            case 3:
                return "string-value-" + random.nextInt(100);
            default:
                return random.nextLong();
        }
    }

    /**
     * Picks a random element from the given array.
     */
    private static String pickRandom(Random random, String... options) {
        return options[random.nextInt(options.length)];
    }

    /**
     * Main method to run benchmarks standalone.
     */
    public static void main(String[] args) throws Exception {
        org.openjdk.jmh.Main.main(args);
    }
}
