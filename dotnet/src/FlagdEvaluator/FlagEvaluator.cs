using System.Collections.Concurrent;
using System.Text;
using System.Text.Json;

namespace FlagdEvaluator;

/// <summary>
/// Evaluates feature flags using a pool of flagd-evaluator WASM instances.
/// Thread-safe for concurrent use. Pre-evaluated (static/disabled) flags are
/// served lock-free via volatile cache. Targeting flags evaluate in parallel
/// up to the pool size.
/// </summary>
public sealed class FlagEvaluator : IDisposable
{
    private readonly WasmRuntime _runtime;
    private readonly BlockingCollection<WasmInstance> _pool;
    private readonly int _poolSize;
    private readonly bool _permissiveValidation;
    private readonly object _updateLock = new();

    // Host-side caches — swapped atomically on UpdateState
    private volatile CacheSnapshot _cache;

    // Generation counter — incremented on each UpdateState
    private ulong _generation;

    /// <summary>
    /// Number of WASM instances in the evaluation pool.
    /// </summary>
    public int PoolSize => _poolSize;

    /// <summary>
    /// Creates a new flag evaluator with the given options.
    /// </summary>
    public FlagEvaluator(FlagEvaluatorOptions? options = null)
    {
        var opts = options ?? new FlagEvaluatorOptions();
        _poolSize = Math.Max(1, opts.PoolSize);
        _permissiveValidation = opts.PermissiveValidation;

        _runtime = new WasmRuntime();
        _pool = new BlockingCollection<WasmInstance>(_poolSize);
        _cache = new CacheSnapshot();

        for (int i = 0; i < _poolSize; i++)
        {
            var instance = new WasmInstance(_runtime, _permissiveValidation);
            _pool.Add(instance);
        }
    }

    /// <summary>
    /// Updates the flag configuration across all WASM instances.
    /// Returns information about changed flags and populates internal caches.
    /// </summary>
    public UpdateStateResult UpdateState(string configJson)
    {
        lock (_updateLock)
        {
            // Drain all instances from pool
            var instances = new WasmInstance[_poolSize];
            for (int i = 0; i < _poolSize; i++)
            {
                instances[i] = _pool.Take();
            }

            try
            {
                // Update first instance and capture result
                var resultJson = instances[0].CallUpdateState(configJson);
                var result = JsonSerializer.Deserialize<UpdateStateResult>(resultJson)
                    ?? throw new EvaluatorException("Failed to deserialize update_state result");

                if (!result.Success)
                {
                    return result;
                }

                // Update remaining instances in parallel
                if (instances.Length > 1)
                {
                    Parallel.For(1, instances.Length, i =>
                    {
                        instances[i].CallUpdateState(configJson);
                    });
                }

                // Increment generation and stamp all instances
                _generation++;
                var gen = _generation;

                foreach (var inst in instances)
                {
                    inst.Generation = gen;
                }

                // Build and atomically swap cache
                _cache = CacheSnapshot.Build(result, gen);

                return result;
            }
            finally
            {
                // Return all instances to pool
                foreach (var inst in instances)
                {
                    _pool.Add(inst);
                }
            }
        }
    }

    /// <summary>
    /// Evaluates a flag and returns the full result.
    /// </summary>
    public EvaluationResult EvaluateFlag(string flagKey, Dictionary<string, object?>? context = null)
    {
        // Load cache snapshot (volatile read, lock-free)
        var snap = _cache;

        // Fast path: pre-evaluated cache hit (static/disabled flags)
        if (snap.PreEvaluated.TryGetValue(flagKey, out var cached))
            return cached;

        // Acquire an instance from the pool
        var inst = _pool.Take();
        try
        {
            // Generation guard: if an UpdateState completed between cache read and pool acquire,
            // the snap has stale indices. Reload to match the instance's generation.
            if (snap.Generation != inst.Generation)
            {
                snap = _cache;
                // Re-check pre-eval cache — flag may now be static
                if (snap.PreEvaluated.TryGetValue(flagKey, out cached))
                    return cached;
            }

            // Determine context serialization strategy
            byte[] contextBytes;
            HashSet<string>? requiredKeys = null;
            snap.RequiredContextKeys.TryGetValue(flagKey, out requiredKeys);

            if (requiredKeys != null && context != null && context.Count > 0)
            {
                contextBytes = ContextSerializer.SerializeFiltered(context, requiredKeys, flagKey);
            }
            else if (context != null && context.Count > 0)
            {
                contextBytes = ContextSerializer.Serialize(context);
            }
            else
            {
                contextBytes = Array.Empty<byte>();
            }

            // Pick evaluation path
            string resultJson;
            if (snap.FlagIndices.TryGetValue(flagKey, out var flagIndex)
                && inst.HasEvaluateByIndex
                && requiredKeys != null)
            {
                resultJson = inst.CallEvaluateByIndex(flagIndex, contextBytes);
            }
            else
            {
                var flagKeyBytes = Encoding.UTF8.GetBytes(flagKey);
                resultJson = inst.CallEvaluateReusable(flagKeyBytes, contextBytes);
            }

            return JsonSerializer.Deserialize<EvaluationResult>(resultJson)
                ?? throw new EvaluatorException("Failed to deserialize evaluation result");
        }
        finally
        {
            _pool.Add(inst);
        }
    }

    /// <summary>Evaluates a boolean flag. Returns defaultValue on error or type mismatch.</summary>
    public bool EvaluateBool(string flagKey, Dictionary<string, object?>? context, bool defaultValue)
    {
        var result = EvaluateFlag(flagKey, context);
        if (result.IsError || result.Value == null)
            return defaultValue;

        if (result.Value.Value.ValueKind == JsonValueKind.True) return true;
        if (result.Value.Value.ValueKind == JsonValueKind.False) return false;
        return defaultValue;
    }

    /// <summary>Evaluates a string flag. Returns defaultValue on error or type mismatch.</summary>
    public string EvaluateString(string flagKey, Dictionary<string, object?>? context, string defaultValue)
    {
        var result = EvaluateFlag(flagKey, context);
        if (result.IsError || result.Value == null)
            return defaultValue;

        if (result.Value.Value.ValueKind == JsonValueKind.String)
            return result.Value.Value.GetString() ?? defaultValue;
        return defaultValue;
    }

    /// <summary>Evaluates an integer flag. Returns defaultValue on error or type mismatch.</summary>
    public int EvaluateInt(string flagKey, Dictionary<string, object?>? context, int defaultValue)
    {
        var result = EvaluateFlag(flagKey, context);
        if (result.IsError || result.Value == null)
            return defaultValue;

        if (result.Value.Value.ValueKind == JsonValueKind.Number
            && result.Value.Value.TryGetInt32(out var intValue))
            return intValue;
        return defaultValue;
    }

    /// <summary>Evaluates a double flag. Returns defaultValue on error or type mismatch.</summary>
    public double EvaluateDouble(string flagKey, Dictionary<string, object?>? context, double defaultValue)
    {
        var result = EvaluateFlag(flagKey, context);
        if (result.IsError || result.Value == null)
            return defaultValue;

        if (result.Value.Value.ValueKind == JsonValueKind.Number
            && result.Value.Value.TryGetDouble(out var doubleValue))
            return doubleValue;
        return defaultValue;
    }

    public void Dispose()
    {
        // Drain and dispose all instances
        for (int i = 0; i < _poolSize; i++)
        {
            if (_pool.TryTake(out var inst, TimeSpan.FromSeconds(5)))
            {
                inst.Dispose();
            }
        }
        _pool.Dispose();
        _runtime.Dispose();
    }
}
