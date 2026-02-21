/** Serialize full context with $flagd enrichment. */
export function serializeContext(
  context: Record<string, unknown> | undefined,
  flagKey: string,
): string {
  if (!context || Object.keys(context).length === 0) {
    return JSON.stringify({
      targetingKey: "",
      $flagd: { flagKey, timestamp: Math.floor(Date.now() / 1000) },
    });
  }

  const enriched: Record<string, unknown> = { ...context };
  if (!("targetingKey" in enriched)) {
    enriched.targetingKey = "";
  }
  enriched.$flagd = { flagKey, timestamp: Math.floor(Date.now() / 1000) };
  return JSON.stringify(enriched);
}

/** Serialize only the required context keys + targetingKey + $flagd. */
export function serializeFilteredContext(
  context: Record<string, unknown> | undefined,
  requiredKeys: Set<string>,
  flagKey: string,
): string {
  const filtered: Record<string, unknown> = {};

  if (context) {
    for (const key of requiredKeys) {
      if (
        key === "targetingKey" ||
        key === "$flagd.flagKey" ||
        key === "$flagd.timestamp"
      ) {
        continue; // handled separately
      }
      if (key in context) {
        filtered[key] = context[key];
      }
    }
  }

  filtered.targetingKey =
    context && "targetingKey" in context ? context.targetingKey : "";
  filtered.$flagd = { flagKey, timestamp: Math.floor(Date.now() / 1000) };
  return JSON.stringify(filtered);
}
