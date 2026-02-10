package dev.openfeature.flagd.evaluator;

import org.openjdk.jmh.annotations.*;
import org.openjdk.jmh.infra.Blackhole;

import java.util.concurrent.TimeUnit;

/**
 * JMH benchmarks for state management (updateState) performance.
 *
 * <p>These benchmarks measure the cost of loading and updating flag configurations
 * of varying sizes. State updates happen less frequently than evaluations in
 * production, but their latency matters for config sync responsiveness.
 *
 * <p><b>Running the benchmarks:</b>
 * <pre>
 * ./mvnw clean package
 * java -jar target/benchmarks.jar StateManagementBenchmark
 *
 * # Run a specific scenario:
 * java -jar target/benchmarks.jar StateManagementBenchmark.S1_updateState_5flags
 * </pre>
 */
@BenchmarkMode({Mode.Throughput, Mode.AverageTime})
@OutputTimeUnit(TimeUnit.MICROSECONDS)
@Fork(1)
@Warmup(iterations = 3, time = 2)
@Measurement(iterations = 5, time = 3)
public class StateManagementBenchmark {

    /**
     * Generates a flag configuration JSON with the specified number of flags.
     * Each flag has a simple boolean variant with optional targeting.
     */
    private static String generateFlagConfig(int flagCount) {
        StringBuilder sb = new StringBuilder(flagCount * 256);
        sb.append("{\"flags\": {");
        for (int i = 0; i < flagCount; i++) {
            if (i > 0) {
                sb.append(",");
            }
            sb.append("\"flag-").append(i).append("\": {");
            sb.append("\"state\": \"ENABLED\",");
            sb.append("\"defaultVariant\": \"on\",");
            sb.append("\"variants\": {\"on\": true, \"off\": false}");
            // Add targeting to every 3rd flag for realism
            if (i % 3 == 0) {
                sb.append(",\"targeting\": {\"if\": [{\"==\": [{\"var\": \"role\"}, \"admin\"]}, \"on\", null]}");
            }
            sb.append("}");
        }
        sb.append("}}");
        return sb.toString();
    }

    /**
     * Generates a config with one flag changed compared to the base config.
     * Changes flag-50's defaultVariant from "on" to "off".
     */
    private static String generateConfigWithOneChange(int flagCount) {
        StringBuilder sb = new StringBuilder(flagCount * 256);
        sb.append("{\"flags\": {");
        for (int i = 0; i < flagCount; i++) {
            if (i > 0) {
                sb.append(",");
            }
            sb.append("\"flag-").append(i).append("\": {");
            sb.append("\"state\": \"ENABLED\",");
            // Change flag-50's default variant
            if (i == 50) {
                sb.append("\"defaultVariant\": \"off\",");
            } else {
                sb.append("\"defaultVariant\": \"on\",");
            }
            sb.append("\"variants\": {\"on\": true, \"off\": false}");
            if (i % 3 == 0) {
                sb.append(",\"targeting\": {\"if\": [{\"==\": [{\"var\": \"role\"}, \"admin\"]}, \"on\", null]}");
            }
            sb.append("}");
        }
        sb.append("}}");
        return sb.toString();
    }

    // ========================================================================
    // S1: Update state with 5 flags
    // ========================================================================

    @State(Scope.Thread)
    public static class State5Flags {
        FlagEvaluator evaluator;
        String config;

        @Setup(Level.Trial)
        public void setup() {
            config = generateFlagConfig(5);
        }

        @Setup(Level.Invocation)
        public void setupInvocation() {
            try {
                evaluator = new FlagEvaluator(FlagEvaluator.ValidationMode.PERMISSIVE);
            } catch (Exception e) {
                throw new RuntimeException("Failed to create evaluator", e);
            }
        }

        @TearDown(Level.Invocation)
        public void tearDown() {
            if (evaluator != null) {
                evaluator.close();
            }
        }
    }

    @Benchmark
    public void S1_updateState_5flags(State5Flags state, Blackhole bh) {
        try {
            UpdateStateResult result = state.evaluator.updateState(state.config);
            bh.consume(result.isSuccess());
            bh.consume(result.getChangedFlags());
        } catch (Exception e) {
            throw new RuntimeException("Benchmark failed", e);
        }
    }

    // ========================================================================
    // S2: Update state with 50 flags
    // ========================================================================

    @State(Scope.Thread)
    public static class State50Flags {
        FlagEvaluator evaluator;
        String config;

        @Setup(Level.Trial)
        public void setup() {
            config = generateFlagConfig(50);
        }

        @Setup(Level.Invocation)
        public void setupInvocation() {
            try {
                evaluator = new FlagEvaluator(FlagEvaluator.ValidationMode.PERMISSIVE);
            } catch (Exception e) {
                throw new RuntimeException("Failed to create evaluator", e);
            }
        }

        @TearDown(Level.Invocation)
        public void tearDown() {
            if (evaluator != null) {
                evaluator.close();
            }
        }
    }

    @Benchmark
    public void S2_updateState_50flags(State50Flags state, Blackhole bh) {
        try {
            UpdateStateResult result = state.evaluator.updateState(state.config);
            bh.consume(result.isSuccess());
            bh.consume(result.getChangedFlags());
        } catch (Exception e) {
            throw new RuntimeException("Benchmark failed", e);
        }
    }

    // ========================================================================
    // S3: Update state with 200 flags
    // ========================================================================

    @State(Scope.Thread)
    public static class State200Flags {
        FlagEvaluator evaluator;
        String config;

        @Setup(Level.Trial)
        public void setup() {
            config = generateFlagConfig(200);
        }

        @Setup(Level.Invocation)
        public void setupInvocation() {
            try {
                evaluator = new FlagEvaluator(FlagEvaluator.ValidationMode.PERMISSIVE);
            } catch (Exception e) {
                throw new RuntimeException("Failed to create evaluator", e);
            }
        }

        @TearDown(Level.Invocation)
        public void tearDown() {
            if (evaluator != null) {
                evaluator.close();
            }
        }
    }

    @Benchmark
    public void S3_updateState_200flags(State200Flags state, Blackhole bh) {
        try {
            UpdateStateResult result = state.evaluator.updateState(state.config);
            bh.consume(result.isSuccess());
            bh.consume(result.getChangedFlags());
        } catch (Exception e) {
            throw new RuntimeException("Benchmark failed", e);
        }
    }

    // ========================================================================
    // S4: Update state with no changes (re-apply same config)
    // ========================================================================

    @State(Scope.Thread)
    public static class StateNoChange {
        FlagEvaluator evaluator;
        String config;

        @Setup(Level.Trial)
        public void setup() {
            config = generateFlagConfig(100);
        }

        @Setup(Level.Invocation)
        public void setupInvocation() {
            try {
                evaluator = new FlagEvaluator(FlagEvaluator.ValidationMode.PERMISSIVE);
                // Load config once so re-apply detects no changes
                UpdateStateResult result = evaluator.updateState(config);
                if (!result.isSuccess()) {
                    throw new RuntimeException("Failed initial state load: " + result.getError());
                }
            } catch (Exception e) {
                throw new RuntimeException("Failed to create evaluator", e);
            }
        }

        @TearDown(Level.Invocation)
        public void tearDown() {
            if (evaluator != null) {
                evaluator.close();
            }
        }
    }

    @Benchmark
    public void S4_updateState_noChanges(StateNoChange state, Blackhole bh) {
        try {
            UpdateStateResult result = state.evaluator.updateState(state.config);
            bh.consume(result.isSuccess());
            bh.consume(result.getChangedFlags());
        } catch (Exception e) {
            throw new RuntimeException("Benchmark failed", e);
        }
    }

    // ========================================================================
    // S5: Update state with 1 flag changed in 100
    // ========================================================================

    @State(Scope.Thread)
    public static class StateOneChange {
        FlagEvaluator evaluator;
        String baseConfig;
        String changedConfig;

        @Setup(Level.Trial)
        public void setup() {
            baseConfig = generateFlagConfig(100);
            changedConfig = generateConfigWithOneChange(100);
        }

        @Setup(Level.Invocation)
        public void setupInvocation() {
            try {
                evaluator = new FlagEvaluator(FlagEvaluator.ValidationMode.PERMISSIVE);
                // Load base config so update detects the single change
                UpdateStateResult result = evaluator.updateState(baseConfig);
                if (!result.isSuccess()) {
                    throw new RuntimeException("Failed initial state load: " + result.getError());
                }
            } catch (Exception e) {
                throw new RuntimeException("Failed to create evaluator", e);
            }
        }

        @TearDown(Level.Invocation)
        public void tearDown() {
            if (evaluator != null) {
                evaluator.close();
            }
        }
    }

    @Benchmark
    public void S5_updateState_1changedIn100(StateOneChange state, Blackhole bh) {
        try {
            UpdateStateResult result = state.evaluator.updateState(state.changedConfig);
            bh.consume(result.isSuccess());
            bh.consume(result.getChangedFlags());
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
