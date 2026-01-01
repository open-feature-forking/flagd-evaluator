package dev.openfeature.flagd.evaluator.comparison;

import com.fasterxml.jackson.databind.ObjectMapper;
import com.fasterxml.jackson.databind.module.SimpleModule;
import dev.openfeature.flagd.evaluator.jackson.EvaluationContextSerializer;
import dev.openfeature.sdk.EvaluationContext;
import dev.openfeature.sdk.MutableContext;
import io.github.jamsesso.jsonlogic.JsonLogic;
import org.junit.jupiter.api.Test;

public class JsonLogicDebugTest {

    @Test
    public void testBasicJsonLogic() throws Exception {
        JsonLogic jsonLogic = new JsonLogic();

        // Very basic test from JsonLogic documentation
        System.out.println("=== Basic JsonLogic Test ===");
        String basicRule = "{\"var\":\"a\"}";
        String basicData = "{\"a\":1}";
        Object basicResult = jsonLogic.apply(basicRule, basicData);
        System.out.println("Rule: " + basicRule);
        System.out.println("Data: " + basicData);
        System.out.println("Result: " + basicResult);
    }

    @Test
    public void debugJsonLogicEvaluation() throws Exception {
        ObjectMapper mapper = new ObjectMapper();
        SimpleModule module = new SimpleModule();
        module.addSerializer(EvaluationContext.class, new EvaluationContextSerializer());
        mapper.registerModule(module);

        JsonLogic jsonLogic = new JsonLogic();

        // Create context
        EvaluationContext ctx = new MutableContext()
            .add("role", "admin")
            .add("tier", "premium");

        // Serialize context
        String contextJson = mapper.writeValueAsString(ctx);
        System.out.println("Context JSON: " + contextJson);

        // Try with a plain map
        java.util.Map<String, Object> plainMap = new java.util.HashMap<>();
        plainMap.put("role", "admin");
        plainMap.put("tier", "premium");
        String plainMapJson = mapper.writeValueAsString(plainMap);
        System.out.println("Plain Map JSON: " + plainMapJson);

        // Simple targeting rule
        String rule = "{\n" +
            "  \"if\": [\n" +
            "    {\n" +
            "      \"and\": [\n" +
            "        {\n" +
            "          \"==\": [\n" +
            "            { \"var\": \"role\" },\n" +
            "            \"admin\"\n" +
            "          ]\n" +
            "        },\n" +
            "        {\n" +
            "          \"in\": [\n" +
            "            { \"var\": \"tier\" },\n" +
            "            [\"premium\", \"enterprise\"]\n" +
            "          ]\n" +
            "        }\n" +
            "      ]\n" +
            "    },\n" +
            "    \"granted\",\n" +
            "    null\n" +
            "  ]\n" +
            "}";

        System.out.println("Rule: " + rule);

        // Evaluate - try different approaches
        System.out.println("\n=== Approach 1: String rule, String context ===");
        Object result1 = jsonLogic.apply(rule, contextJson);
        System.out.println("Result: " + result1);

        System.out.println("\n=== Approach 2: JsonNode.toString() for both ===");
        Object result2 = jsonLogic.apply(mapper.readTree(rule).toString(), mapper.readTree(contextJson).toString());
        System.out.println("Result: " + result2);

        System.out.println("\n=== Approach 3: Simple test with just ==  ===");
        String simpleRule = "{\"==\": [{\"var\": \"role\"}, \"admin\"]}";
        Object result3 = jsonLogic.apply(simpleRule, contextJson);
        System.out.println("Result: " + result3);

        System.out.println("\n=== Approach 4: Simple test with just in ===");
        String inRule = "{\"in\": [{\"var\": \"tier\"}, [\"premium\", \"enterprise\"]]}";
        Object result4 = jsonLogic.apply(inRule, contextJson);
        System.out.println("Result: " + result4);

        System.out.println("\n=== Approach 5: Test with plain map ===");
        Object result5 = jsonLogic.apply(simpleRule, plainMapJson);
        System.out.println("Result: " + result5);

        System.out.println("\n=== Approach 6: Test 'in' with plain map ===");
        Object result6 = jsonLogic.apply(inRule, plainMapJson);
        System.out.println("Result: " + result6);
    }
}
