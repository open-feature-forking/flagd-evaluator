/**
 * MinimalInProcessResolver — matches the actual @openfeature/flagd-core v1.2.0 evaluation path.
 *
 * Uses json-logic-engine v4.0.2 with AOT compilation via engine.build().
 * The build() call happens once at load time per flag, NOT per evaluation.
 * Evaluation calls the compiled function directly with a plain JS object — zero serialization.
 */
import { LogicEngine } from "json-logic-engine";

interface FeatureFlag {
  state: string;
  defaultVariant: string;
  variants: Record<string, unknown>;
  targeting?: Record<string, unknown>;
  compiledTargeting?: (data: Record<string, unknown>) => unknown;
}

export interface OldEvaluationResult {
  value?: unknown;
  variant?: string;
  reason: string;
  errorCode?: string;
  errorMessage?: string;
}

export class MinimalInProcessResolver {
  private flags = new Map<string, FeatureFlag>();
  private engine: LogicEngine;

  constructor() {
    this.engine = new LogicEngine();

    // Register custom operators matching flagd spec
    // These are simplified — enough for benchmark correctness
    this.engine.addMethod("starts_with", {
      method: (args: unknown[]) => {
        if (!Array.isArray(args) || args.length < 2) return false;
        return String(args[0]).startsWith(String(args[1]));
      },
      traverse: true,
    });

    this.engine.addMethod("ends_with", {
      method: (args: unknown[]) => {
        if (!Array.isArray(args) || args.length < 2) return false;
        return String(args[0]).endsWith(String(args[1]));
      },
      traverse: true,
    });
  }

  /** Parse flag config and AOT-compile targeting rules (happens once at load time). */
  loadFlags(jsonConfig: string): void {
    const config = JSON.parse(jsonConfig);
    this.flags.clear();

    for (const [key, flagData] of Object.entries(
      config.flags as Record<string, Record<string, unknown>>,
    )) {
      const flag: FeatureFlag = {
        state: flagData.state as string,
        defaultVariant: flagData.defaultVariant as string,
        variants: flagData.variants as Record<string, unknown>,
        targeting: flagData.targeting as Record<string, unknown> | undefined,
      };

      // AOT compile targeting rules — this is the key optimization
      // engine.build() compiles the JSON Logic into a native JS function
      if (flag.targeting) {
        try {
          flag.compiledTargeting = this.engine.build(flag.targeting);
        } catch {
          // If build fails, fall back to interpreted mode
        }
      }

      this.flags.set(key, flag);
    }
  }

  /** Evaluate a flag — calls the AOT-compiled function directly with context. */
  evaluate(
    flagKey: string,
    context?: Record<string, unknown>,
  ): OldEvaluationResult {
    const flag = this.flags.get(flagKey);
    if (!flag) {
      return {
        reason: "FLAG_NOT_FOUND",
        errorCode: "FLAG_NOT_FOUND",
        errorMessage: `flag: ${flagKey} not found`,
      };
    }

    if (flag.state === "DISABLED") {
      return {
        reason: "DISABLED",
        errorCode: "FLAG_NOT_FOUND",
        errorMessage: `flag: ${flagKey} is disabled`,
      };
    }

    let resolvedVariant: string;
    let reason: string;

    if (!flag.targeting) {
      resolvedVariant = flag.defaultVariant;
      reason = "STATIC";
    } else {
      // Build enriched context — plain object spread, zero serialization
      const data: Record<string, unknown> = {
        ...context,
        $flagd: { flagKey, timestamp: Math.floor(Date.now() / 1000) },
      };
      if (!("targetingKey" in data)) {
        data.targetingKey = "";
      }

      let result: unknown;
      if (flag.compiledTargeting) {
        // Fast path: AOT-compiled function call
        result = flag.compiledTargeting(data);
      } else {
        // Fallback: interpreted
        result = this.engine.run(flag.targeting, data);
      }

      if (result == null) {
        resolvedVariant = flag.defaultVariant;
        reason = "DEFAULT";
      } else {
        resolvedVariant = String(result);
        reason = "TARGETING_MATCH";
      }
    }

    const value = flag.variants[resolvedVariant];
    if (value === undefined) {
      return {
        reason: "ERROR",
        errorCode: "GENERAL",
        errorMessage: `variant ${resolvedVariant} not found in flag ${flagKey}`,
      };
    }

    return { value, variant: resolvedVariant, reason };
  }
}
