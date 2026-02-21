import { loadWasm, unpackPtrLen, readString, writeToBuffer } from "./wasm-runtime.js";
import type { WasmExports } from "./wasm-runtime.js";
import { serializeContext, serializeFilteredContext } from "./context.js";
import type {
  EvaluationResult,
  UpdateStateResult,
  CacheSnapshot,
} from "./types.js";

export type { EvaluationResult, UpdateStateResult };

const MAX_FLAG_KEY_SIZE = 256;
const MAX_CONTEXT_SIZE = 1024 * 1024; // 1MB

export interface FlagEvaluatorOptions {
  permissiveValidation?: boolean;
}

export class FlagEvaluator {
  private wasm: WasmExports;
  private cache: CacheSnapshot;
  private flagKeyBufPtr: number;
  private contextBufPtr: number;
  private hasEvalByIndex: boolean;

  private constructor(wasm: WasmExports) {
    this.wasm = wasm;
    this.cache = {
      preEvaluated: new Map(),
      requiredContextKeys: new Map(),
      flagIndices: new Map(),
    };
    // Pre-allocate reusable buffers
    this.flagKeyBufPtr = wasm.alloc(MAX_FLAG_KEY_SIZE);
    this.contextBufPtr = wasm.alloc(MAX_CONTEXT_SIZE);
    this.hasEvalByIndex = typeof wasm.evaluate_by_index === "function";
  }

  /** Create a new FlagEvaluator by loading and instantiating the WASM module. */
  static async create(
    wasmPath: string,
    options?: FlagEvaluatorOptions,
  ): Promise<FlagEvaluator> {
    const wasm = await loadWasm(wasmPath);
    const evaluator = new FlagEvaluator(wasm);

    if (options?.permissiveValidation) {
      const packed = wasm.set_validation_mode(1);
      const [ptr, len] = unpackPtrLen(packed);
      // Must read before dealloc
      readString(wasm.memory, ptr, len);
      wasm.dealloc(ptr, len);
    }

    return evaluator;
  }

  /** Update flag state. Returns the parsed result with changed flags. */
  updateState(configJson: string): UpdateStateResult {
    const { wasm } = this;
    const configBytes = new TextEncoder().encode(configJson);

    // Allocate and write config
    const configPtr = wasm.alloc(configBytes.byteLength);
    new Uint8Array(wasm.memory.buffer).set(configBytes, configPtr);

    const packed = wasm.update_state(configPtr, configBytes.byteLength);
    const [resultPtr, resultLen] = unpackPtrLen(packed);

    // Copy result before dealloc
    const resultJson = readString(wasm.memory, resultPtr, resultLen);
    wasm.dealloc(resultPtr, resultLen);
    wasm.dealloc(configPtr, configBytes.byteLength);

    const result: UpdateStateResult = JSON.parse(resultJson);

    // Build host-side cache
    this.cache = buildCacheSnapshot(result);

    return result;
  }

  /** Evaluate a flag against the provided context. */
  evaluateFlag(
    flagKey: string,
    context?: Record<string, unknown>,
  ): EvaluationResult {
    // Fast path: pre-evaluated cache hit (static/disabled flags)
    const cached = this.cache.preEvaluated.get(flagKey);
    if (cached) return cached;

    const { wasm } = this;

    // Determine context serialization strategy
    const requiredKeys = this.cache.requiredContextKeys.get(flagKey);
    let contextJson: string;
    if (requiredKeys && context && Object.keys(context).length > 0) {
      contextJson = serializeFilteredContext(context, requiredKeys, flagKey);
    } else {
      contextJson = serializeContext(context, flagKey);
    }

    // Pick eval path: evaluate_by_index or evaluate_reusable
    const flagIndex = this.cache.flagIndices.get(flagKey);
    if (
      this.hasEvalByIndex &&
      flagIndex !== undefined &&
      requiredKeys !== undefined
    ) {
      return this.evaluateByIndex(flagIndex, contextJson);
    }
    return this.evaluateReusable(flagKey, contextJson);
  }

  /** Evaluate using the flag's numeric index (avoids flag key serialization). */
  private evaluateByIndex(
    flagIndex: number,
    contextJson: string,
  ): EvaluationResult {
    const { wasm } = this;

    let contextPtr = 0;
    let contextLen = 0;
    if (contextJson) {
      contextLen = writeToBuffer(wasm.memory, this.contextBufPtr, contextJson);
      contextPtr = this.contextBufPtr;
    }

    const packed = wasm.evaluate_by_index!(flagIndex, contextPtr, contextLen);
    const [resultPtr, resultLen] = unpackPtrLen(packed);

    // Copy before dealloc
    const resultJson = readString(wasm.memory, resultPtr, resultLen);
    wasm.dealloc(resultPtr, resultLen);

    return JSON.parse(resultJson);
  }

  /** Evaluate using flag key string (fallback path). */
  private evaluateReusable(
    flagKey: string,
    contextJson: string,
  ): EvaluationResult {
    const { wasm } = this;

    const flagKeyLen = writeToBuffer(
      wasm.memory,
      this.flagKeyBufPtr,
      flagKey,
    );

    let contextPtr = 0;
    let contextLen = 0;
    if (contextJson) {
      contextLen = writeToBuffer(wasm.memory, this.contextBufPtr, contextJson);
      contextPtr = this.contextBufPtr;
    }

    const packed = wasm.evaluate_reusable(
      this.flagKeyBufPtr,
      flagKeyLen,
      contextPtr,
      contextLen,
    );
    const [resultPtr, resultLen] = unpackPtrLen(packed);

    // Copy before dealloc
    const resultJson = readString(wasm.memory, resultPtr, resultLen);
    wasm.dealloc(resultPtr, resultLen);

    return JSON.parse(resultJson);
  }

  /** Release pre-allocated WASM buffers. */
  dispose(): void {
    this.wasm.dealloc(this.flagKeyBufPtr, MAX_FLAG_KEY_SIZE);
    this.wasm.dealloc(this.contextBufPtr, MAX_CONTEXT_SIZE);
  }
}

/** Build a CacheSnapshot from the WASM update_state response. */
function buildCacheSnapshot(result: UpdateStateResult): CacheSnapshot {
  const preEvaluated = new Map<string, EvaluationResult>();
  if (result.preEvaluated) {
    for (const [key, val] of Object.entries(result.preEvaluated)) {
      preEvaluated.set(key, val);
    }
  }

  const requiredContextKeys = new Map<string, Set<string>>();
  if (result.requiredContextKeys) {
    for (const [key, keys] of Object.entries(result.requiredContextKeys)) {
      requiredContextKeys.set(key, new Set(keys));
    }
  }

  const flagIndices = new Map<string, number>();
  if (result.flagIndices) {
    for (const [key, idx] of Object.entries(result.flagIndices)) {
      flagIndices.set(key, idx);
    }
  }

  return { preEvaluated, requiredContextKeys, flagIndices };
}
