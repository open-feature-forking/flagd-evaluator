/*
 * Copyright 2024 OpenFeature Community
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */

package dev.openfeature.flagd.evaluator;

import com.dylibso.chicory.runtime.ExportFunction;
import com.dylibso.chicory.runtime.Instance;
import com.dylibso.chicory.runtime.Memory;
import com.dylibso.chicory.wasm.Parser;
import com.google.gson.Gson;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;

import java.io.InputStream;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;

/**
 * Example demonstrating how to use the flagd-evaluator WebAssembly module
 * with Chicory (pure Java WASM runtime).
 * 
 * <p>This class shows the complete workflow for:
 * <ul>
 *   <li>Loading the WASM module</li>
 *   <li>Memory management (allocation and deallocation)</li>
 *   <li>Calling the evaluate_logic function</li>
 *   <li>Parsing JSON responses</li>
 *   <li>Using the custom fractional operator for A/B testing</li>
 * </ul>
 * 
 * <h2>Maven Dependencies</h2>
 * <pre>{@code
 * <dependency>
 *     <groupId>com.dylibso.chicory</groupId>
 *     <artifactId>runtime</artifactId>
 *     <version>1.0.0</version>
 * </dependency>
 * <dependency>
 *     <groupId>com.google.code.gson</groupId>
 *     <artifactId>gson</artifactId>
 *     <version>2.10.1</version>
 * </dependency>
 * }</pre>
 */
public class FlagdEvaluatorExample {
    
    private final Instance instance;
    private final Memory memory;
    private final ExportFunction alloc;
    private final ExportFunction dealloc;
    private final ExportFunction evaluateLogic;
    private final Gson gson;

    /**
     * Creates a new FlagdEvaluator instance by loading the WASM module.
     *
     * @param wasmPath Path to the flagd_evaluator.wasm file
     * @throws Exception if the WASM module cannot be loaded
     */
    public FlagdEvaluatorExample(Path wasmPath) throws Exception {
        // Load and instantiate the WASM module
        byte[] wasmBytes = Files.readAllBytes(wasmPath);
        var module = Parser.parse(wasmBytes);
        this.instance = Instance.builder(module).build();
        
        // Get memory and exported functions
        this.memory = instance.memory();
        this.alloc = instance.export("alloc");
        this.dealloc = instance.export("dealloc");
        this.evaluateLogic = instance.export("evaluate_logic");
        this.gson = new Gson();
    }
    
    /**
     * Creates a new FlagdEvaluator instance from an input stream.
     *
     * @param wasmStream InputStream containing the WASM module bytes
     * @throws Exception if the WASM module cannot be loaded
     */
    public FlagdEvaluatorExample(InputStream wasmStream) throws Exception {
        byte[] wasmBytes = wasmStream.readAllBytes();
        var module = Parser.parse(wasmBytes);
        this.instance = Instance.builder(module).build();
        
        this.memory = instance.memory();
        this.alloc = instance.export("alloc");
        this.dealloc = instance.export("dealloc");
        this.evaluateLogic = instance.export("evaluate_logic");
        this.gson = new Gson();
    }

    /**
     * Evaluates a JSON Logic rule against the provided data.
     *
     * @param rule JSON Logic rule as a string
     * @param data Context data as a JSON string
     * @return EvaluationResult containing success status and result/error
     */
    public EvaluationResult evaluate(String rule, String data) {
        byte[] ruleBytes = rule.getBytes(StandardCharsets.UTF_8);
        byte[] dataBytes = data.getBytes(StandardCharsets.UTF_8);
        
        // Allocate memory for rule and data
        long rulePtr = alloc.apply(ruleBytes.length)[0];
        long dataPtr = alloc.apply(dataBytes.length)[0];
        
        try {
            // Write strings to WASM memory
            memory.write((int) rulePtr, ruleBytes);
            memory.write((int) dataPtr, dataBytes);
            
            // Call evaluate_logic - returns packed pointer (ptr << 32 | len)
            long packedResult = evaluateLogic.apply(rulePtr, ruleBytes.length, dataPtr, dataBytes.length)[0];
            
            // Unpack pointer and length
            int resultPtr = (int) (packedResult >>> 32);
            int resultLen = (int) (packedResult & 0xFFFFFFFFL);
            
            // Read result string from memory
            byte[] resultBytes = memory.readBytes(resultPtr, resultLen);
            String resultJson = new String(resultBytes, StandardCharsets.UTF_8);
            
            // Free result memory
            dealloc.apply(resultPtr, resultLen);
            
            // Parse JSON response
            JsonObject response = gson.fromJson(resultJson, JsonObject.class);
            boolean success = response.get("success").getAsBoolean();
            
            if (success) {
                JsonElement result = response.get("result");
                return new EvaluationResult(true, result, null);
            } else {
                String error = response.get("error").getAsString();
                return new EvaluationResult(false, null, error);
            }
        } finally {
            // Always free input memory
            dealloc.apply(rulePtr, ruleBytes.length);
            dealloc.apply(dataPtr, dataBytes.length);
        }
    }

    /**
     * Represents the result of an evaluation.
     */
    public static class EvaluationResult {
        private final boolean success;
        private final JsonElement result;
        private final String error;

        public EvaluationResult(boolean success, JsonElement result, String error) {
            this.success = success;
            this.result = result;
            this.error = error;
        }

        public boolean isSuccess() {
            return success;
        }

        public JsonElement getResult() {
            return result;
        }

        public String getError() {
            return error;
        }
        
        public boolean getAsBoolean() {
            return result != null && result.getAsBoolean();
        }
        
        public String getAsString() {
            return result != null ? result.getAsString() : null;
        }
        
        public int getAsInt() {
            return result != null ? result.getAsInt() : 0;
        }
        
        public double getAsDouble() {
            return result != null ? result.getAsDouble() : 0.0;
        }

        @Override
        public String toString() {
            if (success) {
                return "EvaluationResult{success=true, result=" + result + "}";
            } else {
                return "EvaluationResult{success=false, error=" + error + "}";
            }
        }
    }

    /**
     * Main method demonstrating various use cases.
     */
    public static void main(String[] args) throws Exception {
        // Load the WASM module
        Path wasmPath = Path.of("target/wasm32-unknown-unknown/release/flagd_evaluator.wasm");
        FlagdEvaluatorExample evaluator = new FlagdEvaluatorExample(wasmPath);

        System.out.println("=== Basic JSON Logic Examples ===\n");

        // Example 1: Simple equality check
        EvaluationResult result1 = evaluator.evaluate(
            "{\"==\": [1, 1]}",
            "{}"
        );
        System.out.println("1 == 1: " + result1.getAsBoolean());

        // Example 2: Variable access
        EvaluationResult result2 = evaluator.evaluate(
            "{\"var\": \"user.name\"}",
            "{\"user\": {\"name\": \"Alice\"}}"
        );
        System.out.println("user.name: " + result2.getAsString());

        // Example 3: Comparison with variable
        EvaluationResult result3 = evaluator.evaluate(
            "{\">=\": [{\"var\": \"age\"}, 18]}",
            "{\"age\": 25}"
        );
        System.out.println("age >= 18: " + result3.getAsBoolean());

        // Example 4: Conditional (if-then-else)
        EvaluationResult result4 = evaluator.evaluate(
            "{\"if\": [{\">\": [{\"var\": \"temp\"}, 30]}, \"hot\", \"normal\"]}",
            "{\"temp\": 35}"
        );
        System.out.println("Temperature status: " + result4.getAsString());

        // Example 5: Boolean logic
        EvaluationResult result5 = evaluator.evaluate(
            "{\"and\": [{\">\": [{\"var\": \"age\"}, 18]}, {\"<\": [{\"var\": \"age\"}, 65]}]}",
            "{\"age\": 30}"
        );
        System.out.println("Is working age: " + result5.getAsBoolean());

        System.out.println("\n=== Fractional (A/B Testing) Examples ===\n");

        // Example 6: Fractional operator for A/B testing (50/50 split)
        EvaluationResult result6 = evaluator.evaluate(
            "{\"fractional\": [\"user-123\", [\"control\", 50, \"treatment\", 50]]}",
            "{}"
        );
        System.out.println("User bucket (50/50): " + result6.getAsString());

        // Example 7: Fractional with variable reference
        EvaluationResult result7 = evaluator.evaluate(
            "{\"fractional\": [{\"var\": \"userId\"}, [\"A\", 33, \"B\", 33, \"C\", 34]]}",
            "{\"userId\": \"test-user-42\"}"
        );
        System.out.println("User bucket (A/B/C): " + result7.getAsString());

        // Example 8: Fractional with unequal weights (10/90 split)
        EvaluationResult result8 = evaluator.evaluate(
            "{\"fractional\": [\"user-456\", [\"beta\", 10, \"stable\", 90]]}",
            "{}"
        );
        System.out.println("User bucket (10/90): " + result8.getAsString());

        // Demonstrate consistency - same key always gets same bucket
        System.out.println("\n=== Consistency Check ===\n");
        for (int i = 0; i < 3; i++) {
            EvaluationResult consistencyCheck = evaluator.evaluate(
                "{\"fractional\": [\"consistent-user\", [\"X\", 50, \"Y\", 50]]}",
                "{}"
            );
            System.out.println("Attempt " + (i + 1) + ": " + consistencyCheck.getAsString());
        }

        System.out.println("\n=== Error Handling Example ===\n");

        // Example 9: Invalid JSON
        EvaluationResult errorResult = evaluator.evaluate(
            "not valid json",
            "{}"
        );
        System.out.println("Error handling: " + errorResult);

        System.out.println("\n=== Feature Flag Use Case ===\n");

        // Complete feature flag evaluation example
        String featureFlagRule = """
            {
                "if": [
                    {">=": [{"var": "user.tier"}, 2]},
                    {"fractional": [{"var": "user.id"}, ["new-ui", 50, "old-ui", 50]]},
                    "old-ui"
                ]
            }
            """;
        
        String userData = """
            {
                "user": {
                    "id": "user-abc-123",
                    "tier": 3,
                    "country": "US"
                }
            }
            """;
        
        EvaluationResult flagResult = evaluator.evaluate(featureFlagRule, userData);
        System.out.println("Feature flag result: " + flagResult);
        
        System.out.println("\n=== Done ===");
    }
}
