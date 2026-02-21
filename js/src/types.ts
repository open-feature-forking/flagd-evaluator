/** Result of evaluating a single feature flag. */
export interface EvaluationResult {
  value: unknown;
  variant?: string;
  reason: string;
  errorCode?: string;
  errorMessage?: string;
  flagMetadata?: Record<string, unknown>;
}

/** Result of updating flag state via WASM. */
export interface UpdateStateResult {
  success: boolean;
  error?: string;
  changedFlags?: string[];
  preEvaluated?: Record<string, EvaluationResult>;
  requiredContextKeys?: Record<string, string[]>;
  flagIndices?: Record<string, number>;
}

/** Host-side cache built from UpdateStateResult. */
export interface CacheSnapshot {
  preEvaluated: Map<string, EvaluationResult>;
  requiredContextKeys: Map<string, Set<string>>;
  flagIndices: Map<string, number>;
}
