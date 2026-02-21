import { describe, it, expect, beforeAll, afterAll } from "vitest";
import { resolve, dirname } from "node:path";
import { fileURLToPath } from "node:url";
import { FlagEvaluator } from "../src/evaluator.js";

const __dirname = dirname(fileURLToPath(import.meta.url));
const WASM_PATH = resolve(__dirname, "../flagd_evaluator.wasm");

const simpleFlagConfig = JSON.stringify({
  flags: {
    "simple-flag": {
      state: "ENABLED",
      variants: { on: true, off: false },
      defaultVariant: "on",
    },
  },
});

const targetingFlagConfig = JSON.stringify({
  flags: {
    "targeting-flag": {
      state: "ENABLED",
      variants: { premium: "premium-experience", basic: "basic-experience" },
      defaultVariant: "basic",
      targeting: {
        if: [{ "==": [{ var: "tier" }, "premium"] }, "premium", "basic"],
      },
    },
  },
});

const mixedConfig = JSON.stringify({
  flags: {
    "static-flag": {
      state: "ENABLED",
      variants: { on: true, off: false },
      defaultVariant: "on",
    },
    "disabled-flag": {
      state: "DISABLED",
      variants: { on: true, off: false },
      defaultVariant: "on",
    },
    "targeting-flag": {
      state: "ENABLED",
      variants: { premium: "premium-experience", basic: "basic-experience" },
      defaultVariant: "basic",
      targeting: {
        if: [{ "==": [{ var: "tier" }, "premium"] }, "premium", "basic"],
      },
    },
    "string-flag": {
      state: "ENABLED",
      variants: { hello: "Hello, World!", goodbye: "Goodbye!" },
      defaultVariant: "hello",
    },
    "object-flag": {
      state: "ENABLED",
      variants: {
        config1: { key: "value1", nested: { a: 1 } },
        config2: { key: "value2" },
      },
      defaultVariant: "config1",
    },
  },
});

describe("FlagEvaluator", () => {
  let evaluator: FlagEvaluator;

  beforeAll(async () => {
    evaluator = await FlagEvaluator.create(WASM_PATH);
  });

  afterAll(() => {
    evaluator.dispose();
  });

  it("should load WASM and update state", () => {
    const result = evaluator.updateState(simpleFlagConfig);
    expect(result.success).toBe(true);
    expect(result.changedFlags).toContain("simple-flag");
  });

  it("should evaluate a simple static flag", () => {
    evaluator.updateState(simpleFlagConfig);
    const result = evaluator.evaluateFlag("simple-flag");
    expect(result.value).toBe(true);
    expect(result.variant).toBe("on");
    expect(result.reason).toBe("STATIC");
  });

  it("should return pre-evaluated cache for static flags", () => {
    const stateResult = evaluator.updateState(simpleFlagConfig);
    expect(stateResult.preEvaluated).toBeDefined();
    expect(stateResult.preEvaluated!["simple-flag"]).toBeDefined();
    expect(stateResult.preEvaluated!["simple-flag"].value).toBe(true);
  });

  it("should evaluate targeting rule — match", () => {
    evaluator.updateState(targetingFlagConfig);
    const result = evaluator.evaluateFlag("targeting-flag", {
      tier: "premium",
      targetingKey: "user-123",
    });
    expect(result.value).toBe("premium-experience");
    expect(result.variant).toBe("premium");
    expect(result.reason).toBe("TARGETING_MATCH");
  });

  it("should evaluate targeting rule — no match (default)", () => {
    evaluator.updateState(targetingFlagConfig);
    const result = evaluator.evaluateFlag("targeting-flag", {
      tier: "free",
      targetingKey: "user-456",
    });
    expect(result.value).toBe("basic-experience");
    expect(result.variant).toBe("basic");
    expect(result.reason).toBe("TARGETING_MATCH");
  });

  it("should return error for flag not found", () => {
    evaluator.updateState(simpleFlagConfig);
    const result = evaluator.evaluateFlag("nonexistent-flag");
    expect(result.reason).toBe("FLAG_NOT_FOUND");
    expect(result.errorCode).toBe("FLAG_NOT_FOUND");
  });

  it("should handle disabled flags", () => {
    evaluator.updateState(mixedConfig);
    const result = evaluator.evaluateFlag("disabled-flag");
    expect(result.reason).toBe("DISABLED");
    expect(result.errorCode).toBe("FLAG_NOT_FOUND");
  });

  it("should handle disabled flags via pre-eval cache", () => {
    const stateResult = evaluator.updateState(mixedConfig);
    expect(stateResult.preEvaluated!["disabled-flag"]).toBeDefined();
    expect(stateResult.preEvaluated!["disabled-flag"].reason).toBe("DISABLED");
  });

  it("should evaluate string flags", () => {
    evaluator.updateState(mixedConfig);
    const result = evaluator.evaluateFlag("string-flag");
    expect(result.value).toBe("Hello, World!");
    expect(result.variant).toBe("hello");
  });

  it("should evaluate object flags", () => {
    evaluator.updateState(mixedConfig);
    const result = evaluator.evaluateFlag("object-flag");
    expect(result.value).toEqual({ key: "value1", nested: { a: 1 } });
    expect(result.variant).toBe("config1");
  });

  it("should produce same result with large vs small context", () => {
    evaluator.updateState(targetingFlagConfig);

    const smallCtx = { tier: "premium", targetingKey: "user-123" };
    const largeCtx: Record<string, unknown> = {
      ...smallCtx,
    };
    for (let i = 0; i < 200; i++) {
      largeCtx[`attr_${i}`] = `value_${i}`;
    }

    const smallResult = evaluator.evaluateFlag("targeting-flag", smallCtx);
    const largeResult = evaluator.evaluateFlag("targeting-flag", largeCtx);

    expect(smallResult.value).toBe(largeResult.value);
    expect(smallResult.variant).toBe(largeResult.variant);
    expect(smallResult.reason).toBe(largeResult.reason);
  });

  it("should report requiredContextKeys for targeting flags", () => {
    const stateResult = evaluator.updateState(targetingFlagConfig);
    expect(stateResult.requiredContextKeys).toBeDefined();
    const keys = stateResult.requiredContextKeys!["targeting-flag"];
    expect(keys).toBeDefined();
    expect(keys).toContain("tier");
  });

  it("should report flagIndices for targeting flags", () => {
    const stateResult = evaluator.updateState(targetingFlagConfig);
    expect(stateResult.flagIndices).toBeDefined();
    expect(
      typeof stateResult.flagIndices!["targeting-flag"],
    ).toBe("number");
  });

  it("should work with permissive validation mode", async () => {
    const permissive = await FlagEvaluator.create(WASM_PATH, {
      permissiveValidation: true,
    });

    // fractional operator not valid under strict schema
    const config = JSON.stringify({
      flags: {
        abTest: {
          state: "ENABLED",
          variants: {
            control: "control-exp",
            treatment: "treatment-exp",
          },
          defaultVariant: "control",
          targeting: {
            fractional: [
              ["control", 50],
              ["treatment", 50],
            ],
          },
        },
      },
    });

    const result = permissive.updateState(config);
    expect(result.success).toBe(true);

    const evalResult = permissive.evaluateFlag("abTest", {
      targetingKey: "user-abc",
    });
    expect(evalResult.reason).toBe("TARGETING_MATCH");
    expect(["control-exp", "treatment-exp"]).toContain(evalResult.value);

    permissive.dispose();
  });
});
