namespace FlagdEvaluator;

/// <summary>
/// Configuration options for <see cref="FlagEvaluator"/>.
/// </summary>
public sealed class FlagEvaluatorOptions
{
    /// <summary>
    /// Number of WASM instances in the evaluation pool.
    /// Defaults to <see cref="Environment.ProcessorCount"/>.
    /// </summary>
    public int PoolSize { get; set; } = Environment.ProcessorCount;

    /// <summary>
    /// When true, accept invalid flag configurations with warnings instead of rejecting them.
    /// </summary>
    public bool PermissiveValidation { get; set; }
}
