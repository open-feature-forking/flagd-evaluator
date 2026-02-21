/**
 * Comparison benchmarks: WASM evaluator vs json-logic-engine (AOT compiled)
 *
 * Measures the trade-offs of WASM boundary + serialization overhead vs
 * native JS function calls with zero serialization.
 */
import { resolve, dirname } from "node:path";
import { fileURLToPath } from "node:url";
import { Bench } from "tinybench";
import { FlagEvaluator } from "../src/evaluator.js";
import { MinimalInProcessResolver } from "./old-resolver.js";

const __dirname = dirname(fileURLToPath(import.meta.url));
const WASM_PATH = resolve(__dirname, "../flagd_evaluator.wasm");

// ============================================================================
// Test configurations
// ============================================================================

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
    "static-bool": {
      state: "ENABLED",
      variants: { on: true, off: false },
      defaultVariant: "on",
    },
    "static-string": {
      state: "ENABLED",
      variants: { hello: "Hello, World!", goodbye: "Goodbye!" },
      defaultVariant: "hello",
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
    "targeting-flag-2": {
      state: "ENABLED",
      variants: { admin: "admin-access", user: "user-access" },
      defaultVariant: "user",
      targeting: {
        if: [{ "==": [{ var: "role" }, "admin"] }, "admin", "user"],
      },
    },
  },
});

// ============================================================================
// Context generators
// ============================================================================

const emptyCtx = {};
const smallCtx = {
  tier: "premium",
  targetingKey: "user-123",
  role: "admin",
  region: "us-east",
  score: 85,
};

function makeLargeCtx(size: number): Record<string, unknown> {
  const ctx: Record<string, unknown> = { ...smallCtx };
  for (let i = 0; i < size; i++) {
    ctx[`attr_${i}`] = `value_${i}`;
  }
  return ctx;
}

// ============================================================================
// Benchmark runner
// ============================================================================

async function main() {
  console.log("Loading WASM evaluator...");
  const wasmEval = await FlagEvaluator.create(WASM_PATH, {
    permissiveValidation: true,
  });

  console.log("Setting up json-logic-engine resolver...\n");
  const oldResolver = new MinimalInProcessResolver();

  // ========================================================================
  // X1: Simple flag — pre-eval cache vs AOT compiled
  // ========================================================================
  console.log("═══════════════════════════════════════════════════════");
  console.log("X1: Simple flag (static, no targeting)");
  console.log("═══════════════════════════════════════════════════════");

  wasmEval.updateState(simpleFlagConfig);
  oldResolver.loadFlags(simpleFlagConfig);

  const x1 = new Bench({ warmupIterations: 1000 });
  x1.add("WASM (pre-eval cache)", () => {
    wasmEval.evaluateFlag("simple-flag");
  });
  x1.add("json-logic-engine (AOT)", () => {
    oldResolver.evaluate("simple-flag");
  });
  await x1.run();
  console.table(
    x1.tasks.map((t) => ({
      Name: t.name,
      "ops/sec": Math.round(t.result!.hz).toLocaleString(),
      "avg (ns)": Math.round(t.result!.mean * 1e6),
      "p99 (ns)": Math.round(t.result!.p999 * 1e6),
    })),
  );

  // ========================================================================
  // X2: Targeting with small context (5 attrs)
  // ========================================================================
  console.log("\n═══════════════════════════════════════════════════════");
  console.log("X2: Targeting with small context (5 attrs)");
  console.log("═══════════════════════════════════════════════════════");

  wasmEval.updateState(targetingFlagConfig);
  oldResolver.loadFlags(targetingFlagConfig);

  const x2 = new Bench({ warmupIterations: 1000 });
  x2.add("WASM (targeting + filtered ctx)", () => {
    wasmEval.evaluateFlag("targeting-flag", smallCtx);
  });
  x2.add("json-logic-engine (AOT + object spread)", () => {
    oldResolver.evaluate("targeting-flag", smallCtx);
  });
  await x2.run();
  console.table(
    x2.tasks.map((t) => ({
      Name: t.name,
      "ops/sec": Math.round(t.result!.hz).toLocaleString(),
      "avg (ns)": Math.round(t.result!.mean * 1e6),
      "p99 (ns)": Math.round(t.result!.p999 * 1e6),
    })),
  );

  // ========================================================================
  // X3: Context size sweep
  // ========================================================================
  console.log("\n═══════════════════════════════════════════════════════");
  console.log("X3: Context size sweep (targeting flag)");
  console.log("═══════════════════════════════════════════════════════");

  wasmEval.updateState(targetingFlagConfig);
  oldResolver.loadFlags(targetingFlagConfig);

  const contextSizes = [
    { label: "empty (0)", ctx: emptyCtx },
    { label: "small (5)", ctx: smallCtx },
    { label: "medium (50)", ctx: makeLargeCtx(50) },
    { label: "large (200)", ctx: makeLargeCtx(200) },
    { label: "xlarge (1000)", ctx: makeLargeCtx(1000) },
  ];

  const x3 = new Bench({ warmupIterations: 500 });
  for (const { label, ctx } of contextSizes) {
    x3.add(`WASM ${label}`, () => {
      wasmEval.evaluateFlag("targeting-flag", ctx);
    });
    x3.add(`json-logic-engine ${label}`, () => {
      oldResolver.evaluate("targeting-flag", ctx);
    });
  }
  await x3.run();
  console.table(
    x3.tasks.map((t) => ({
      Name: t.name,
      "ops/sec": Math.round(t.result!.hz).toLocaleString(),
      "avg (ns)": Math.round(t.result!.mean * 1e6),
      "p99 (ns)": Math.round(t.result!.p999 * 1e6),
    })),
  );

  // ========================================================================
  // X4: Mixed workload (80% static + 20% targeting)
  // ========================================================================
  console.log("\n═══════════════════════════════════════════════════════");
  console.log("X4: Mixed workload (80% static + 20% targeting)");
  console.log("═══════════════════════════════════════════════════════");

  wasmEval.updateState(mixedConfig);
  oldResolver.loadFlags(mixedConfig);

  const staticFlags = ["static-bool", "static-string", "disabled-flag"];
  const targetingFlags = ["targeting-flag", "targeting-flag-2"];

  let wasmCallIdx = 0;
  let oldCallIdx = 0;

  // Simulate 80/20 workload: pick from 10-item cycle (8 static, 2 targeting)
  const flagCycle = [
    ...Array(8)
      .fill(null)
      .map((_, i) => staticFlags[i % staticFlags.length]),
    ...targetingFlags,
  ];

  const x4 = new Bench({ warmupIterations: 500 });
  x4.add("WASM (mixed 80/20)", () => {
    const flagKey = flagCycle[wasmCallIdx % flagCycle.length];
    wasmCallIdx++;
    wasmEval.evaluateFlag(flagKey, smallCtx);
  });
  x4.add("json-logic-engine (mixed 80/20)", () => {
    const flagKey = flagCycle[oldCallIdx % flagCycle.length];
    oldCallIdx++;
    oldResolver.evaluate(flagKey, smallCtx);
  });
  await x4.run();
  console.table(
    x4.tasks.map((t) => ({
      Name: t.name,
      "ops/sec": Math.round(t.result!.hz).toLocaleString(),
      "avg (ns)": Math.round(t.result!.mean * 1e6),
      "p99 (ns)": Math.round(t.result!.p999 * 1e6),
    })),
  );

  // ========================================================================
  // Summary
  // ========================================================================
  console.log("\n═══════════════════════════════════════════════════════");
  console.log("Summary");
  console.log("═══════════════════════════════════════════════════════");
  console.log(
    "X1 (simple): Tests pre-evaluated cache (Map.get) vs AOT function call",
  );
  console.log(
    "X2 (targeting): Tests WASM boundary + JSON serialize vs direct JS call",
  );
  console.log(
    "X3 (ctx sweep): Tests context filtering advantage at scale",
  );
  console.log(
    "X4 (mixed 80/20): Realistic workload — pre-eval cache dominates",
  );

  wasmEval.dispose();
}

main().catch(console.error);
