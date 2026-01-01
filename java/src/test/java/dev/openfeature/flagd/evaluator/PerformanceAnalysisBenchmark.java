package dev.openfeature.flagd.evaluator;

import dev.openfeature.sdk.EvaluationContext;
import dev.openfeature.sdk.ImmutableContext;
import dev.openfeature.sdk.Value;

import java.util.HashMap;

/**
 * Performance analysis benchmark to identify bottlenecks in the evaluation hot path.
 * <p>
 * This benchmark instruments each phase of the evaluation to measure where time is spent:
 * 1. Java-side JSON serialization
 * 2. WASM memory allocation
 * 3. Memory copy to WASM
 * 4. WASM function call (includes all WASM-side processing)
 * 5. Memory read from WASM
 * 6. Java-side JSON deserialization
 * 7. Memory deallocation
 */
public class PerformanceAnalysisBenchmark {

    private static final int WARMUP_ITERATIONS = 1000;
    private static final int MEASUREMENT_ITERATIONS = 10000;

    private FlagEvaluator evaluator;

    public PerformanceAnalysisBenchmark() {

    }

    public void initialize(String flagConfig) throws Exception {
        evaluator = new FlagEvaluator();
        evaluator.updateState(flagConfig);
    }

    /**
     * Measures detailed timing breakdown for a single evaluation.
     */
    public TimingBreakdown measureSingleEvaluation(String flagKey, EvaluationContext contextJson) throws Exception {


        // Phase 2: WASM memory allocation
        long start = System.nanoTime();
        evaluator.evaluateFlag(String.class, flagKey, contextJson);
        long evalTime = System.nanoTime() - start;

        return new TimingBreakdown(
                evalTime
        );
    }

    /**
     * Runs the benchmark and returns aggregated results.
     */
    public AggregatedResults runBenchmark(String flagKey, EvaluationContext contextJson) throws Exception {
        // Warmup
        System.out.println("Warming up (" + WARMUP_ITERATIONS + " iterations)...");
        for (int i = 0; i < WARMUP_ITERATIONS; i++) {
            measureSingleEvaluation(flagKey, contextJson);
        }

        // Measurement
        System.out.println("Measuring (" + MEASUREMENT_ITERATIONS + " iterations)...");
        long[] evalTime = new long[MEASUREMENT_ITERATIONS];

        for (int i = 0; i < MEASUREMENT_ITERATIONS; i++) {
            TimingBreakdown timing = measureSingleEvaluation(flagKey, contextJson);
            evalTime[i] = timing.evalTime;
        }

        return new AggregatedResults(
                average(evalTime),
                0
        );
    }

    private static double average(long[] values) {
        long sum = 0;
        for (long v : values) sum += v;
        return (double) sum / values.length;
    }

    public static class TimingBreakdown {
        public final long evalTime;

        public TimingBreakdown(long evalTime) {
            this.evalTime = evalTime;
        }

        public long total() {
            return evalTime;
        }
    }

    public static class AggregatedResults {
        public final double evalTime;
        public final int contextJsonSize;

        public AggregatedResults(double evalTime, int contextJsonSize) {
            this.evalTime = evalTime;
            this.contextJsonSize = contextJsonSize;
        }

        public double total() {
            return evalTime;
        }

        public void print() {
            double total = total();
            System.out.println("\n=== Performance Analysis Results ===\n");
            System.out.println("Data sizes:");
            System.out.printf("  Context JSON: %d bytes%n", contextJsonSize);

            System.out.println("Timing breakdown (average over " + MEASUREMENT_ITERATIONS + " iterations):\n");
            printPhase("Evaluation (Java)", evalTime, total);
            System.out.println("   ─────────────────────────────────────────────────────────────");
            System.out.printf("   TOTAL: %.2f µs%n%n", total / 1000.0);

        }

        private void printPhase(String name, double ns, double total) {
            double us = ns / 1000.0;
            double percent = ns / total * 100;
            int barLen = (int) (percent / 2);  // Scale to ~50 chars max
            String bar = "█".repeat(Math.max(1, barLen));
            System.out.printf("   %-30s %8.2f µs (%5.1f%%) %s%n", name, us, percent, bar);
        }
    }

    public static void main(String[] args) throws Exception {
        PerformanceAnalysisBenchmark benchmark = new PerformanceAnalysisBenchmark();

        // Initialize with a typical flag configuration
        String config = "{\n" +
                "  \"flags\": {\n" +
                "    \"simple-flag\": {\n" +
                "      \"state\": \"ENABLED\",\n" +
                "      \"defaultVariant\": \"on\",\n" +
                "      \"variants\": {\n" +
                "        \"on\": \"enabled-value\",\n" +
                "        \"off\": \"disabled-value\"\n" +
                "      }\n" +
                "    },\n" +
                "    \"targeting-flag\": {\n" +
                "      \"state\": \"ENABLED\",\n" +
                "      \"defaultVariant\": \"default\",\n" +
                "      \"variants\": {\n" +
                "        \"default\": \"default-value\",\n" +
                "        \"premium\": \"premium-value\",\n" +
                "        \"beta\": \"beta-value\"\n" +
                "      },\n" +
                "      \"targeting\": {\n" +
                "        \"if\": [\n" +
                "          {\"==\": [{\"var\": \"tier\"}, \"premium\"]},\n" +
                "          \"premium\",\n" +
                "          {\"if\": [\n" +
                "            {\"==\": [{\"var\": \"beta\"}, true]},\n" +
                "            \"beta\",\n" +
                "            \"default\"\n" +
                "          ]}\n" +
                "        ]\n" +
                "      }\n" +
                "    }\n" +
                "  }\n" +
                "}";
        benchmark.initialize(config);

        System.out.println("\n========== SIMPLE FLAG (no targeting) ==========");
        AggregatedResults simpleResults = benchmark.runBenchmark("simple-flag", ImmutableContext.EMPTY);
        simpleResults.print();

        System.out.println("\n========== TARGETING FLAG (with targeting rules) ==========");
        String complexContext = "{\n" +
                "  \"targetingKey\": \"user-12345\",\n" +
                "  \"tier\": \"premium\",\n" +
                "  \"beta\": false,\n" +
                "  \"email\": \"user@example.com\",\n" +
                "  \"attributes\": {\n" +
                "    \"country\": \"US\",\n" +
                "    \"age\": 25,\n" +
                "    \"subscriptionType\": \"annual\"\n" +
                "  }\n" +
                "}";
        HashMap<String, Value> attributes = new HashMap<>();
        attributes.put("tier", Value.objectToValue("premium"));
        attributes.put("beta", Value.objectToValue(false));

        for (int i = 0; i < 5000; i++) {
            attributes.put("key" + 1, Value.objectToValue("value" + 1));
        }
        EvaluationContext ctx = new ImmutableContext(attributes);
        AggregatedResults targetingResults = benchmark.runBenchmark("targeting-flag", ctx);
        targetingResults.print();
    }
}
