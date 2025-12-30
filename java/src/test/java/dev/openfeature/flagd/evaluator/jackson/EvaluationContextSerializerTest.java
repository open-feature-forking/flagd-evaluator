package dev.openfeature.flagd.evaluator.jackson;

import com.fasterxml.jackson.databind.ObjectMapper;
import com.fasterxml.jackson.databind.module.SimpleModule;
import dev.openfeature.sdk.EvaluationContext;
import dev.openfeature.sdk.MutableContext;
import dev.openfeature.sdk.MutableStructure;
import dev.openfeature.sdk.Value;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;

import java.util.Arrays;
import java.util.List;
import java.util.Map;

import static org.assertj.core.api.Assertions.assertThat;

/**
 * Comprehensive tests for EvaluationContextSerializer.
 *
 * Tests serialization of:
 * - Primitive types (boolean, string, integer, double)
 * - Nested structures (maps within maps)
 * - Lists (including lists of structures)
 * - Mixed types
 * - Edge cases (empty structures, null values)
 */
class EvaluationContextSerializerTest {

    private ObjectMapper objectMapper;

    @BeforeEach
    void setUp() {
        objectMapper = new ObjectMapper();
        SimpleModule module = new SimpleModule();
        module.addSerializer(EvaluationContext.class, new EvaluationContextSerializer());
        objectMapper.registerModule(module);
    }

    @Test
    void testPrimitiveTypes() throws Exception {
        EvaluationContext context = new MutableContext()
            .add("stringField", "test-value")
            .add("booleanField", true)
            .add("integerField", 42)
            .add("doubleField", 3.14);

        String json = objectMapper.writeValueAsString(context);

        @SuppressWarnings("unchecked")
        Map<String, Object> result = objectMapper.readValue(json, Map.class);

        assertThat(result).containsEntry("stringField", "test-value");
        assertThat(result).containsEntry("booleanField", true);
        assertThat(result.get("integerField")).isEqualTo(42.0);  // Numbers are doubles
        assertThat(result.get("doubleField")).isEqualTo(3.14);
    }

    @Test
    void testNestedStructure() throws Exception {
        MutableStructure address = new MutableStructure()
            .add("street", "123 Main St")
            .add("city", "Springfield")
            .add("zipCode", 12345);

        MutableStructure user = new MutableStructure()
            .add("name", "John Doe")
            .add("age", 30)
            .add("address", address);

        EvaluationContext context = new MutableContext()
            .add("user", user)
            .add("tier", "premium");

        String json = objectMapper.writeValueAsString(context);

        @SuppressWarnings("unchecked")
        Map<String, Object> result = objectMapper.readValue(json, Map.class);

        assertThat(result).containsKey("user");
        assertThat(result).containsKey("tier");

        @SuppressWarnings("unchecked")
        Map<String, Object> userMap = (Map<String, Object>) result.get("user");
        assertThat(userMap).containsEntry("name", "John Doe");
        assertThat(userMap.get("age")).isEqualTo(30.0);  // Numbers are doubles

        @SuppressWarnings("unchecked")
        Map<String, Object> addressMap = (Map<String, Object>) userMap.get("address");
        assertThat(addressMap).containsEntry("street", "123 Main St");
        assertThat(addressMap).containsEntry("city", "Springfield");
        assertThat(addressMap.get("zipCode")).isEqualTo(12345.0);  // Numbers are doubles
    }

    @Test
    void testListOfPrimitives() throws Exception {
        EvaluationContext context = new MutableContext()
            .add("tags", Arrays.asList(
                new Value("frontend"),
                new Value("backend"),
                new Value("mobile")
            ))
            .add("scores", Arrays.asList(
                new Value(85),
                new Value(92),
                new Value(78)
            ));

        String json = objectMapper.writeValueAsString(context);

        @SuppressWarnings("unchecked")
        Map<String, Object> result = objectMapper.readValue(json, Map.class);

        @SuppressWarnings("unchecked")
        List<String> tags = (List<String>) result.get("tags");
        assertThat(tags).containsExactly("frontend", "backend", "mobile");

        @SuppressWarnings("unchecked")
        List<Double> scores = (List<Double>) result.get("scores");
        assertThat(scores).containsExactly(85.0, 92.0, 78.0);  // Numbers are doubles
    }

    @Test
    void testListOfStructures() throws Exception {
        MutableStructure product1 = new MutableStructure()
            .add("name", "Widget")
            .add("price", 19.99)
            .add("inStock", true);

        MutableStructure product2 = new MutableStructure()
            .add("name", "Gadget")
            .add("price", 29.99)
            .add("inStock", false);

        EvaluationContext context = new MutableContext()
            .add("products", Arrays.asList(
                new Value(product1),
                new Value(product2)
            ));

        String json = objectMapper.writeValueAsString(context);

        @SuppressWarnings("unchecked")
        Map<String, Object> result = objectMapper.readValue(json, Map.class);

        @SuppressWarnings("unchecked")
        List<Map<String, Object>> products = (List<Map<String, Object>>) result.get("products");

        assertThat(products).hasSize(2);
        assertThat(products.get(0)).containsEntry("name", "Widget");
        assertThat(products.get(0)).containsEntry("price", 19.99);
        assertThat(products.get(0)).containsEntry("inStock", true);

        assertThat(products.get(1)).containsEntry("name", "Gadget");
        assertThat(products.get(1)).containsEntry("price", 29.99);
        assertThat(products.get(1)).containsEntry("inStock", false);
    }

    @Test
    void testDeeplyNestedStructures() throws Exception {
        MutableStructure level3 = new MutableStructure()
            .add("deepValue", "buried treasure")
            .add("deepNumber", 999);

        MutableStructure level2 = new MutableStructure()
            .add("middleValue", "mid-level")
            .add("level3", level3);

        MutableStructure level1 = new MutableStructure()
            .add("topValue", "top-level")
            .add("level2", level2);

        EvaluationContext context = new MutableContext()
            .add("level1", level1);

        String json = objectMapper.writeValueAsString(context);

        @SuppressWarnings("unchecked")
        Map<String, Object> result = objectMapper.readValue(json, Map.class);

        @SuppressWarnings("unchecked")
        Map<String, Object> l1 = (Map<String, Object>) result.get("level1");
        assertThat(l1).containsEntry("topValue", "top-level");

        @SuppressWarnings("unchecked")
        Map<String, Object> l2 = (Map<String, Object>) l1.get("level2");
        assertThat(l2).containsEntry("middleValue", "mid-level");

        @SuppressWarnings("unchecked")
        Map<String, Object> l3 = (Map<String, Object>) l2.get("level3");
        assertThat(l3).containsEntry("deepValue", "buried treasure");
        assertThat(l3.get("deepNumber")).isEqualTo(999.0);  // Numbers are doubles
    }

    @Test
    void testMixedTypesInList() throws Exception {
        EvaluationContext context = new MutableContext()
            .add("mixedList", Arrays.asList(
                new Value("string"),
                new Value(42),
                new Value(true),
                new Value(3.14)
            ));

        String json = objectMapper.writeValueAsString(context);

        @SuppressWarnings("unchecked")
        Map<String, Object> result = objectMapper.readValue(json, Map.class);

        @SuppressWarnings("unchecked")
        List<Object> mixedList = (List<Object>) result.get("mixedList");

        assertThat(mixedList).hasSize(4);
        assertThat(mixedList.get(0)).isEqualTo("string");
        assertThat(mixedList.get(1)).isEqualTo(42.0);  // Numbers are doubles
        assertThat(mixedList.get(2)).isEqualTo(true);
        assertThat(mixedList.get(3)).isEqualTo(3.14);
    }

    @Test
    void testEmptyStructure() throws Exception {
        MutableStructure emptyStruct = new MutableStructure();

        EvaluationContext context = new MutableContext()
            .add("empty", emptyStruct)
            .add("notEmpty", "value");

        String json = objectMapper.writeValueAsString(context);

        @SuppressWarnings("unchecked")
        Map<String, Object> result = objectMapper.readValue(json, Map.class);

        @SuppressWarnings("unchecked")
        Map<String, Object> emptyMap = (Map<String, Object>) result.get("empty");
        assertThat(emptyMap).isEmpty();
        assertThat(result).containsEntry("notEmpty", "value");
    }

    @Test
    void testEmptyList() throws Exception {
        EvaluationContext context = new MutableContext()
            .add("emptyList", Arrays.asList())
            .add("notEmpty", "value");

        String json = objectMapper.writeValueAsString(context);

        @SuppressWarnings("unchecked")
        Map<String, Object> result = objectMapper.readValue(json, Map.class);

        @SuppressWarnings("unchecked")
        List<Object> emptyList = (List<Object>) result.get("emptyList");
        assertThat(emptyList).isEmpty();
        assertThat(result).containsEntry("notEmpty", "value");
    }

    @Test
    void testEmptyContext() throws Exception {
        EvaluationContext context = new MutableContext();

        String json = objectMapper.writeValueAsString(context);

        @SuppressWarnings("unchecked")
        Map<String, Object> result = objectMapper.readValue(json, Map.class);

        assertThat(result).isEmpty();
    }

    @Test
    void testComplexRealWorldScenario() throws Exception {
        // Simulate a complex evaluation context with user profile, feature flags context, etc.
        MutableStructure permissions = new MutableStructure()
            .add("canEdit", true)
            .add("canDelete", false)
            .add("canShare", true);

        MutableStructure subscription = new MutableStructure()
            .add("tier", "premium")
            .add("expiresAt", "2024-12-31")
            .add("features", Arrays.asList(
                new Value("advanced-analytics"),
                new Value("api-access"),
                new Value("priority-support")
            ));

        MutableStructure user = new MutableStructure()
            .add("id", "user-123")
            .add("email", "user@example.com")
            .add("role", "admin")
            .add("permissions", permissions)
            .add("subscription", subscription);

        EvaluationContext context = new MutableContext("user-123")
            .add("user", user)
            .add("environment", "production")
            .add("requestId", "req-456")
            .add("timestamp", 1703980800)
            .add("experiments", Arrays.asList(
                new Value("experiment-a"),
                new Value("experiment-b")
            ));

        String json = objectMapper.writeValueAsString(context);

        @SuppressWarnings("unchecked")
        Map<String, Object> result = objectMapper.readValue(json, Map.class);

        // Verify top-level fields
        assertThat(result).containsEntry("targetingKey", "user-123");
        assertThat(result).containsEntry("environment", "production");
        assertThat(result).containsEntry("requestId", "req-456");
        assertThat(result.get("timestamp")).isEqualTo(1703980800.0);  // Numbers are doubles

        // Verify user structure
        @SuppressWarnings("unchecked")
        Map<String, Object> userMap = (Map<String, Object>) result.get("user");
        assertThat(userMap).containsEntry("id", "user-123");
        assertThat(userMap).containsEntry("email", "user@example.com");
        assertThat(userMap).containsEntry("role", "admin");

        // Verify nested permissions
        @SuppressWarnings("unchecked")
        Map<String, Object> permsMap = (Map<String, Object>) userMap.get("permissions");
        assertThat(permsMap).containsEntry("canEdit", true);
        assertThat(permsMap).containsEntry("canDelete", false);

        // Verify nested subscription
        @SuppressWarnings("unchecked")
        Map<String, Object> subMap = (Map<String, Object>) userMap.get("subscription");
        assertThat(subMap).containsEntry("tier", "premium");

        @SuppressWarnings("unchecked")
        List<String> features = (List<String>) subMap.get("features");
        assertThat(features).containsExactly("advanced-analytics", "api-access", "priority-support");

        // Verify experiments list
        @SuppressWarnings("unchecked")
        List<String> experiments = (List<String>) result.get("experiments");
        assertThat(experiments).containsExactly("experiment-a", "experiment-b");
    }

    @Test
    void testTargetingKeyInContext() throws Exception {
        EvaluationContext context = new MutableContext("user-456")
            .add("email", "test@example.com");

        String json = objectMapper.writeValueAsString(context);

        @SuppressWarnings("unchecked")
        Map<String, Object> result = objectMapper.readValue(json, Map.class);

        assertThat(result).containsEntry("targetingKey", "user-456");
        assertThat(result).containsEntry("email", "test@example.com");
    }
}
