namespace FlagdEvaluator;

/// <summary>
/// Immutable snapshot of host-side caches. Replaced atomically via volatile reference on UpdateState.
/// </summary>
internal sealed class CacheSnapshot
{
    internal ulong Generation { get; init; }
    internal IReadOnlyDictionary<string, EvaluationResult> PreEvaluated { get; init; }
    internal IReadOnlyDictionary<string, HashSet<string>> RequiredContextKeys { get; init; }
    internal IReadOnlyDictionary<string, uint> FlagIndices { get; init; }

    internal CacheSnapshot()
    {
        PreEvaluated = new Dictionary<string, EvaluationResult>();
        RequiredContextKeys = new Dictionary<string, HashSet<string>>();
        FlagIndices = new Dictionary<string, uint>();
    }

    /// <summary>
    /// Builds a cache snapshot from an UpdateStateResult.
    /// </summary>
    internal static CacheSnapshot Build(UpdateStateResult result, ulong generation)
    {
        var preEvaluated = result.PreEvaluated != null
            ? new Dictionary<string, EvaluationResult>(result.PreEvaluated)
            : new Dictionary<string, EvaluationResult>();

        var requiredKeys = new Dictionary<string, HashSet<string>>();
        if (result.RequiredContextKeys != null)
        {
            foreach (var (flagKey, keys) in result.RequiredContextKeys)
            {
                requiredKeys[flagKey] = new HashSet<string>(keys, StringComparer.Ordinal);
            }
        }

        var flagIndices = result.FlagIndices != null
            ? new Dictionary<string, uint>(result.FlagIndices)
            : new Dictionary<string, uint>();

        return new CacheSnapshot
        {
            Generation = generation,
            PreEvaluated = preEvaluated,
            RequiredContextKeys = requiredKeys,
            FlagIndices = flagIndices,
        };
    }
}
