package dev.openfeature.flagd.evaluator;

import dev.openfeature.sdk.EvaluationContext;
import dev.openfeature.sdk.MutableContext;
import dev.openfeature.sdk.ProviderEvaluation;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;

import java.util.ArrayList;
import java.util.HashMap;
import java.util.List;
import java.util.Map;
import java.util.concurrent.CopyOnWriteArrayList;
import java.util.concurrent.CountDownLatch;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;
import java.util.concurrent.Future;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.atomic.AtomicBoolean;
import java.util.concurrent.atomic.AtomicInteger;

import static org.assertj.core.api.Assertions.assertThat;

/**
 * Integration tests for FlagEvaluator.
 */
class FlagEvaluatorTest {

    private FlagEvaluator evaluator;

    @BeforeEach
    void setUp() {
        evaluator = new FlagEvaluator(FlagEvaluator.ValidationMode.PERMISSIVE);
    }

    @AfterEach
    void tearDown() {
        if (evaluator != null) {
            evaluator.close();
        }
    }

    @Test
    void testSimpleBooleanFlag() throws EvaluatorException {
        String config = "{\n" +
                "                  \"flags\": {\n" +
                "                    \"simple-flag\": {\n" +
                "                      \"state\": \"ENABLED\",\n" +
                "                      \"defaultVariant\": \"on\",\n" +
                "                      \"variants\": {\n" +
                "                        \"on\": true,\n" +
                "                        \"off\": false\n" +
                "                      }\n" +
                "                    }\n" +
                "                  }\n" +
                "                }";

        UpdateStateResult updateResult = evaluator.updateState(config);
        assertThat(updateResult.isSuccess()).isTrue();
        assertThat(updateResult.getChangedFlags()).contains("simple-flag");

        EvaluationResult<Boolean> result = evaluator.evaluateFlag(Boolean.class, "simple-flag", "{}");
        assertThat(result.getValue()).isEqualTo(true);
        assertThat(result.getVariant()).isEqualTo("on");
        assertThat(result.getReason()).isEqualTo("STATIC");
        assertThat(result.isError()).isFalse();
    }

    @Test
    void testStringFlag() throws EvaluatorException {
        String config = " {\n" +
                "                  \"flags\": {\n" +
                "                    \"color-flag\": {\n" +
                "                      \"state\": \"ENABLED\",\n" +
                "                      \"defaultVariant\": \"red\",\n" +
                "                      \"variants\": {\n" +
                "                        \"red\": \"red\",\n" +
                "                        \"blue\": \"blue\",\n" +
                "                        \"green\": \"green\"\n" +
                "                      }\n" +
                "                    }\n" +
                "                  }\n" +
                "                }";

        UpdateStateResult updateResult = evaluator.updateState(config);
        assertThat(updateResult.isSuccess()).isTrue();

        EvaluationResult<String> result = evaluator.evaluateFlag(String.class, "color-flag", "{}");
        assertThat(result.getValue()).isEqualTo("red");
        assertThat(result.getVariant()).isEqualTo("red");
    }

    @Test
    void testTargetingRule() throws EvaluatorException {
        String config = " {\n" +
                "                  \"flags\": {\n" +
                "                    \"user-flag\": {\n" +
                "                      \"state\": \"ENABLED\",\n" +
                "                      \"defaultVariant\": \"default\",\n" +
                "                      \"variants\": {\n" +
                "                        \"default\": false,\n" +
                "                        \"premium\": true\n" +
                "                      },\n" +
                "                      \"targeting\": {\n" +
                "                        \"if\": [\n" +
                "                          {\n" +
                "                            \"==\": [\n" +
                "                              { \"var\": \"email\" },\n" +
                "                              \"premium@example.com\"\n" +
                "                            ]\n" +
                "                          },\n" +
                "                          \"premium\",\n" +
                "                          null\n" +
                "                        ]\n" +
                "                      }\n" +
                "                    }\n" +
                "                  }\n" +
                "                }";

        UpdateStateResult updateResult = evaluator.updateState(config);
        assertThat(updateResult.isSuccess()).isTrue();

        // Test with matching context
        EvaluationContext context = new MutableContext().add("email", "premium@example.com");
        EvaluationResult<Boolean> result = evaluator.evaluateFlag(Boolean.class, "user-flag", context);
        assertThat(result.getValue()).isEqualTo(true);
        assertThat(result.getVariant()).isEqualTo("premium");
        assertThat(result.getReason()).isEqualTo("TARGETING_MATCH");

        // Test with non-matching context
        context = new MutableContext().add("email", "regular@example.com");
        result = evaluator.evaluateFlag(Boolean.class, "user-flag", context);
        assertThat(result.getValue()).isEqualTo(false);
        assertThat(result.getVariant()).isEqualTo("default");
    }

    @Test
    void testFlagNotFound() throws EvaluatorException {
        String config = "{\n" +
                "                  \"flags\": {}\n" +
                "                }";

        evaluator.updateState(config);

        EvaluationResult<Boolean> result = evaluator.evaluateFlag(Boolean.class, "nonexistent-flag", "{}");
        assertThat(result.getReason()).isEqualTo("FLAG_NOT_FOUND");
    }

    @Test
    void testDisabledFlag() throws EvaluatorException {
        String config = "{\n" +
                "                  \"flags\": {\n" +
                "                    \"disabled-flag\": {\n" +
                "                      \"state\": \"DISABLED\",\n" +
                "                      \"defaultVariant\": \"off\",\n" +
                "                      \"variants\": {\n" +
                "                        \"on\": true,\n" +
                "                        \"off\": false\n" +
                "                      }\n" +
                "                    }\n" +
                "                  }\n" +
                "                }";

        UpdateStateResult updateResult = evaluator.updateState(config);
        assertThat(updateResult.isSuccess()).isTrue();

        EvaluationResult<Boolean> result = evaluator.evaluateFlag(Boolean.class, "disabled-flag", "{}");
        assertThat(result.getValue()).isNull();
        assertThat(result.getReason()).isEqualTo("DISABLED");
    }

    @Test
    void testNumericFlag() throws EvaluatorException {
        String config = "{\n" +
                "                  \"flags\": {\n" +
                "                    \"number-flag\": {\n" +
                "                      \"state\": \"ENABLED\",\n" +
                "                      \"defaultVariant\": \"default\",\n" +
                "                      \"variants\": {\n" +
                "                        \"default\": 42,\n" +
                "                        \"large\": 1000\n" +
                "                      }\n" +
                "                    }\n" +
                "                  }\n" +
                "                }";

        UpdateStateResult updateResult = evaluator.updateState(config);
        assertThat(updateResult.isSuccess()).isTrue();

        EvaluationResult<Integer> result = evaluator.evaluateFlag(Integer.class, "number-flag", "{}");
        assertThat(result.getValue()).isEqualTo(42);
    }

    @Test
    void testContextEnrichment() throws EvaluatorException {
        String config = " {\n" +
                "                  \"flags\": {\n" +
                "                    \"targeting-key-flag\": {\n" +
                "                      \"state\": \"ENABLED\",\n" +
                "                      \"defaultVariant\": \"default\",\n" +
                "                      \"variants\": {\n" +
                "                        \"default\": \"unknown\",\n" +
                "                        \"known\": \"known-user\"\n" +
                "                      },\n" +
                "                      \"targeting\": {\n" +
                "                        \"if\": [\n" +
                "                          {\n" +
                "                            \"!=\": [\n" +
                "                              { \"var\": \"targetingKey\" },\n" +
                "                              \"\"\n" +
                "                            ]\n" +
                "                          },\n" +
                "                          \"known\",\n" +
                "                          null\n" +
                "                        ]\n" +
                "                      }\n" +
                "                    }\n" +
                "                  }\n" +
                "                }";

        UpdateStateResult updateResult = evaluator.updateState(config);
        assertThat(updateResult.isSuccess()).isTrue();

        // Test with targeting key
        EvaluationContext context = new MutableContext("user-123");
        EvaluationResult<String> result = evaluator.evaluateFlag(String.class, "targeting-key-flag", context);
        assertThat(result.getValue()).isEqualTo("known-user");
        assertThat(result.getReason()).isEqualTo("TARGETING_MATCH");
    }

    @Test
    void testUpdateStateWithChangedFlags() throws EvaluatorException {
        // Initial config
        String config1 = "{\n" +
                "                  \"flags\": {\n" +
                "                    \"flag-a\": {\n" +
                "                      \"state\": \"ENABLED\",\n" +
                "                      \"defaultVariant\": \"on\",\n" +
                "                      \"variants\": { \"on\": true }\n" +
                "                    }\n" +
                "                  }\n" +
                "                }";

        UpdateStateResult result1 = evaluator.updateState(config1);
        assertThat(result1.isSuccess()).isTrue();
        assertThat(result1.getChangedFlags()).containsExactly("flag-a");

        // Update with new and modified flags
        String config2 = "{\n" +
                "                  \"flags\": {\n" +
                "                    \"flag-a\": {\n" +
                "                      \"state\": \"DISABLED\",\n" +
                "                      \"defaultVariant\": \"off\",\n" +
                "                      \"variants\": { \"off\": false }\n" +
                "                    },\n" +
                "                    \"flag-b\": {\n" +
                "                      \"state\": \"ENABLED\",\n" +
                "                      \"defaultVariant\": \"on\",\n" +
                "                      \"variants\": { \"on\": true }\n" +
                "                    }\n" +
                "                  }\n" +
                "                }";

        UpdateStateResult result2 = evaluator.updateState(config2);
        assertThat(result2.isSuccess()).isTrue();
        assertThat(result2.getChangedFlags()).containsExactlyInAnyOrder("flag-a", "flag-b");
    }

    @Test
    void testUpdateStateReturnsRequiredContextKeys() throws EvaluatorException {
        String config = "{\n" +
                "  \"flags\": {\n" +
                "    \"targeted-flag\": {\n" +
                "      \"state\": \"ENABLED\",\n" +
                "      \"defaultVariant\": \"off\",\n" +
                "      \"variants\": { \"on\": true, \"off\": false },\n" +
                "      \"targeting\": {\n" +
                "        \"if\": [\n" +
                "          { \"==\": [{ \"var\": \"email\" }, \"admin@example.com\"] },\n" +
                "          \"on\", \"off\"\n" +
                "        ]\n" +
                "      }\n" +
                "    },\n" +
                "    \"static-flag\": {\n" +
                "      \"state\": \"ENABLED\",\n" +
                "      \"defaultVariant\": \"on\",\n" +
                "      \"variants\": { \"on\": true }\n" +
                "    }\n" +
                "  }\n" +
                "}";

        UpdateStateResult result = evaluator.updateState(config);
        assertThat(result.isSuccess()).isTrue();

        // Should have required context keys for the targeted flag
        assertThat(result.getRequiredContextKeys()).isNotNull();
        assertThat(result.getRequiredContextKeys()).containsKey("targeted-flag");
        assertThat(result.getRequiredContextKeys().get("targeted-flag")).contains("email", "targetingKey");

        // Static flags should not be in required context keys
        assertThat(result.getRequiredContextKeys()).doesNotContainKey("static-flag");
    }

    @Test
    void testUpdateStateReturnsFlagIndices() throws EvaluatorException {
        String config = "{\n" +
                "  \"flags\": {\n" +
                "    \"flagB\": {\n" +
                "      \"state\": \"ENABLED\",\n" +
                "      \"defaultVariant\": \"on\",\n" +
                "      \"variants\": { \"on\": true }\n" +
                "    },\n" +
                "    \"flagA\": {\n" +
                "      \"state\": \"ENABLED\",\n" +
                "      \"defaultVariant\": \"off\",\n" +
                "      \"variants\": { \"off\": false }\n" +
                "    }\n" +
                "  }\n" +
                "}";

        UpdateStateResult result = evaluator.updateState(config);
        assertThat(result.isSuccess()).isTrue();

        // Should have indices for all flags
        assertThat(result.getFlagIndices()).isNotNull();
        assertThat(result.getFlagIndices()).containsKey("flagA");
        assertThat(result.getFlagIndices()).containsKey("flagB");
        // Indices should be assigned in sorted order
        assertThat(result.getFlagIndices().get("flagA")).isEqualTo(0);
        assertThat(result.getFlagIndices().get("flagB")).isEqualTo(1);
    }

    @Test
    void testFilteredContextEvaluation() throws EvaluatorException {
        // This test verifies that filtered context serialization produces correct results
        // The flag uses only "email" from the context, but we pass many attributes
        String config = "{\n" +
                "  \"flags\": {\n" +
                "    \"email-flag\": {\n" +
                "      \"state\": \"ENABLED\",\n" +
                "      \"defaultVariant\": \"default\",\n" +
                "      \"variants\": { \"default\": false, \"premium\": true },\n" +
                "      \"targeting\": {\n" +
                "        \"if\": [\n" +
                "          { \"==\": [{ \"var\": \"email\" }, \"admin@example.com\"] },\n" +
                "          \"premium\", null\n" +
                "        ]\n" +
                "      }\n" +
                "    }\n" +
                "  }\n" +
                "}";

        evaluator.updateState(config);

        // Create a "large" context with many attributes - only email matters
        MutableContext context = new MutableContext("user-123");
        context.add("email", "admin@example.com");
        context.add("name", "Admin User");
        context.add("age", 30);
        context.add("country", "US");
        context.add("tier", "premium");
        context.add("department", "engineering");

        // Should match via filtered context path
        EvaluationResult<Boolean> result = evaluator.evaluateFlag(Boolean.class, "email-flag", context);
        assertThat(result.getValue()).isEqualTo(true);
        assertThat(result.getVariant()).isEqualTo("premium");
        assertThat(result.getReason()).isEqualTo("TARGETING_MATCH");

        // Non-matching email
        MutableContext context2 = new MutableContext("user-456");
        context2.add("email", "regular@example.com");
        context2.add("name", "Regular User");
        context2.add("age", 25);

        result = evaluator.evaluateFlag(Boolean.class, "email-flag", context2);
        assertThat(result.getValue()).isEqualTo(false);
        assertThat(result.getVariant()).isEqualTo("default");
    }

    @Test
    void testPreEvaluatedCacheStillWorks() throws EvaluatorException {
        // Verify that pre-evaluated (static/disabled) flags still use the cache
        String config = "{\n" +
                "  \"flags\": {\n" +
                "    \"static-flag\": {\n" +
                "      \"state\": \"ENABLED\",\n" +
                "      \"defaultVariant\": \"on\",\n" +
                "      \"variants\": { \"on\": true, \"off\": false }\n" +
                "    },\n" +
                "    \"disabled-flag\": {\n" +
                "      \"state\": \"DISABLED\",\n" +
                "      \"defaultVariant\": \"on\",\n" +
                "      \"variants\": { \"on\": true, \"off\": false }\n" +
                "    }\n" +
                "  }\n" +
                "}";

        evaluator.updateState(config);

        // These should be served from cache (no WASM call)
        MutableContext context = new MutableContext("user-1");
        context.add("anything", "value");

        EvaluationResult<Boolean> staticResult = evaluator.evaluateFlag(Boolean.class, "static-flag", context);
        assertThat(staticResult.getValue()).isEqualTo(true);
        assertThat(staticResult.getReason()).isEqualTo("STATIC");

        EvaluationResult<Boolean> disabledResult = evaluator.evaluateFlag(Boolean.class, "disabled-flag", context);
        assertThat(disabledResult.getValue()).isNull();
        assertThat(disabledResult.getReason()).isEqualTo("DISABLED");
    }

    // ========================================================================
    // Concurrency Tests
    // ========================================================================

    @Test
    void testConcurrentEvaluation() throws Exception {
        // N threads evaluating targeting flags simultaneously — verify correctness
        String config = "{\n" +
                "  \"flags\": {\n" +
                "    \"user-flag\": {\n" +
                "      \"state\": \"ENABLED\",\n" +
                "      \"defaultVariant\": \"default\",\n" +
                "      \"variants\": { \"default\": false, \"premium\": true },\n" +
                "      \"targeting\": {\n" +
                "        \"if\": [\n" +
                "          { \"==\": [{ \"var\": \"email\" }, \"premium@example.com\"] },\n" +
                "          \"premium\", null\n" +
                "        ]\n" +
                "      }\n" +
                "    }\n" +
                "  }\n" +
                "}";

        evaluator.updateState(config);

        int threadCount = 8;
        int iterationsPerThread = 100;
        ExecutorService executor = Executors.newFixedThreadPool(threadCount);
        CountDownLatch startLatch = new CountDownLatch(1);
        List<Future<?>> futures = new ArrayList<>();
        CopyOnWriteArrayList<Throwable> errors = new CopyOnWriteArrayList<>();

        for (int t = 0; t < threadCount; t++) {
            final int threadId = t;
            futures.add(executor.submit(() -> {
                try {
                    startLatch.await();
                    for (int i = 0; i < iterationsPerThread; i++) {
                        // Even threads get matching context, odd threads get non-matching
                        MutableContext ctx;
                        if (threadId % 2 == 0) {
                            ctx = new MutableContext("user-" + threadId);
                            ctx.add("email", "premium@example.com");
                        } else {
                            ctx = new MutableContext("user-" + threadId);
                            ctx.add("email", "regular@example.com");
                        }

                        EvaluationResult<Boolean> result = evaluator.evaluateFlag(
                            Boolean.class, "user-flag", ctx);

                        if (threadId % 2 == 0) {
                            assertThat(result.getValue()).isEqualTo(true);
                            assertThat(result.getVariant()).isEqualTo("premium");
                        } else {
                            assertThat(result.getValue()).isEqualTo(false);
                            assertThat(result.getVariant()).isEqualTo("default");
                        }
                    }
                } catch (Throwable e) {
                    errors.add(e);
                }
            }));
        }

        startLatch.countDown();
        for (Future<?> f : futures) {
            f.get(30, TimeUnit.SECONDS);
        }
        executor.shutdown();

        assertThat(errors).isEmpty();
    }

    @Test
    void testConcurrentUpdateAndEvaluate() throws Exception {
        // 1 writer + N readers running simultaneously — verify no exceptions
        String configA = "{\n" +
                "  \"flags\": {\n" +
                "    \"toggle-flag\": {\n" +
                "      \"state\": \"ENABLED\",\n" +
                "      \"defaultVariant\": \"on\",\n" +
                "      \"variants\": { \"on\": true, \"off\": false },\n" +
                "      \"targeting\": {\n" +
                "        \"if\": [\n" +
                "          { \"==\": [{ \"var\": \"role\" }, \"admin\"] },\n" +
                "          \"on\", \"off\"\n" +
                "        ]\n" +
                "      }\n" +
                "    }\n" +
                "  }\n" +
                "}";

        String configB = "{\n" +
                "  \"flags\": {\n" +
                "    \"toggle-flag\": {\n" +
                "      \"state\": \"ENABLED\",\n" +
                "      \"defaultVariant\": \"off\",\n" +
                "      \"variants\": { \"on\": true, \"off\": false },\n" +
                "      \"targeting\": {\n" +
                "        \"if\": [\n" +
                "          { \"==\": [{ \"var\": \"role\" }, \"admin\"] },\n" +
                "          \"on\", \"off\"\n" +
                "        ]\n" +
                "      }\n" +
                "    }\n" +
                "  }\n" +
                "}";

        evaluator.updateState(configA);

        int readerCount = 4;
        AtomicBoolean running = new AtomicBoolean(true);
        CopyOnWriteArrayList<Throwable> errors = new CopyOnWriteArrayList<>();
        ExecutorService executor = Executors.newFixedThreadPool(readerCount + 1);
        List<Future<?>> futures = new ArrayList<>();

        // Writer thread: alternates configs
        futures.add(executor.submit(() -> {
            try {
                for (int i = 0; i < 20; i++) {
                    String config = (i % 2 == 0) ? configB : configA;
                    evaluator.updateState(config);
                    Thread.sleep(5);
                }
            } catch (Throwable e) {
                errors.add(e);
            } finally {
                running.set(false);
            }
        }));

        // Reader threads: evaluate continuously
        for (int t = 0; t < readerCount; t++) {
            futures.add(executor.submit(() -> {
                try {
                    MutableContext ctx = new MutableContext("user-1");
                    ctx.add("role", "admin");
                    while (running.get()) {
                        EvaluationResult<Boolean> result = evaluator.evaluateFlag(
                            Boolean.class, "toggle-flag", ctx);
                        // Value should always be true (targeting matches "admin")
                        assertThat(result.getValue()).isEqualTo(true);
                        assertThat(result.getVariant()).isEqualTo("on");
                    }
                } catch (Throwable e) {
                    errors.add(e);
                }
            }));
        }

        for (Future<?> f : futures) {
            f.get(30, TimeUnit.SECONDS);
        }
        executor.shutdown();

        assertThat(errors).isEmpty();
    }

    @Test
    void testPoolSize1() throws EvaluatorException {
        // Regression: single-instance pool still works correctly
        FlagEvaluator singlePool = new FlagEvaluator(FlagEvaluator.ValidationMode.PERMISSIVE, 1);
        try {
            assertThat(singlePool.getPoolSize()).isEqualTo(1);

            String config = "{\n" +
                    "  \"flags\": {\n" +
                    "    \"simple-flag\": {\n" +
                    "      \"state\": \"ENABLED\",\n" +
                    "      \"defaultVariant\": \"on\",\n" +
                    "      \"variants\": { \"on\": true, \"off\": false }\n" +
                    "    }\n" +
                    "  }\n" +
                    "}";

            UpdateStateResult updateResult = singlePool.updateState(config);
            assertThat(updateResult.isSuccess()).isTrue();

            EvaluationResult<Boolean> result = singlePool.evaluateFlag(Boolean.class, "simple-flag", "{}");
            assertThat(result.getValue()).isEqualTo(true);
            assertThat(result.getVariant()).isEqualTo("on");
            assertThat(result.getReason()).isEqualTo("STATIC");
        } finally {
            singlePool.close();
        }
    }

    @Test
    void testPoolSizeMatchesConstructorArg() {
        FlagEvaluator customPool = new FlagEvaluator(FlagEvaluator.ValidationMode.PERMISSIVE, 4);
        try {
            assertThat(customPool.getPoolSize()).isEqualTo(4);
        } finally {
            customPool.close();
        }
    }
}
