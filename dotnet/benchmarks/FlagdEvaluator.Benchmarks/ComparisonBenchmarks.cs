using BenchmarkDotNet.Attributes;
using BenchmarkDotNet.Jobs;
using Eval = FlagdEvaluator;

namespace FlagdEvaluator.Benchmarks;

/// <summary>
/// Comparison benchmarks: old JsonLogic provider (json-everything) vs new WASM evaluator.
///
/// Mirrors the Java ComparisonBenchmark and Go comparison_test.go patterns:
/// - X1: Simple flag (old vs new, single-threaded)
/// - X2: Targeting evaluation (old vs new, single-threaded)
/// - X3: Context size sweep (empty, small, large — old vs new)
/// - X4: Concurrent targeting (4 threads)
/// - X5: Concurrent large context (8 threads)
/// </summary>
[MemoryDiagnoser]
public class ComparisonBenchmarks
{
    private Eval.FlagEvaluator _newEvaluator = null!;
    private MinimalInProcessResolver _oldResolver = null!;

    private Dictionary<string, object?> _emptyCtx = null!;
    private Dictionary<string, object?> _smallCtx = null!;
    private Dictionary<string, object?> _largeCtx = null!;

    private const string FlagConfig = """
    {
        "flags": {
            "simple-bool": {
                "state": "ENABLED",
                "defaultVariant": "on",
                "variants": { "on": true, "off": false }
            },
            "targeted-access": {
                "state": "ENABLED",
                "defaultVariant": "denied",
                "variants": { "denied": false, "granted": true },
                "targeting": {
                    "if": [
                        { "and": [
                            { "==": [{ "var": "role" }, "admin"] },
                            { "in": [{ "var": "tier" }, ["premium", "enterprise"]] }
                        ]},
                        "granted", null
                    ]
                }
            }
        }
    }
    """;

    [GlobalSetup]
    public void Setup()
    {
        // New WASM evaluator
        _newEvaluator = new Eval.FlagEvaluator(new Eval.FlagEvaluatorOptions
        {
            PermissiveValidation = true,
        });
        _newEvaluator.UpdateState(FlagConfig);

        // Old JsonLogic resolver
        _oldResolver = new MinimalInProcessResolver();
        _oldResolver.LoadFlags(FlagConfig);

        // Contexts
        _emptyCtx = new Dictionary<string, object?>();

        _smallCtx = new Dictionary<string, object?>
        {
            ["targetingKey"] = "user-123",
            ["role"] = "admin",
            ["tier"] = "premium",
            ["region"] = "us-east",
            ["score"] = 85,
        };

        _largeCtx = new Dictionary<string, object?> { ["targetingKey"] = "user-123" };
        _largeCtx["role"] = "admin";
        _largeCtx["tier"] = "premium";
        _largeCtx["region"] = "us-east";
        _largeCtx["score"] = 85;
        for (int i = 0; i < 100; i++)
        {
            object val = (i % 4) switch
            {
                0 => $"value_{i}",
                1 => (object)(i * 7),
                2 => (object)(i % 2 == 0),
                _ => (object)(i * 1.5),
            };
            _largeCtx[$"attr_{i}"] = val;
        }
    }

    [GlobalCleanup]
    public void Cleanup() => _newEvaluator.Dispose();

    // ========================================================================
    // X1: Old vs New — Simple flag evaluation (single-threaded)
    // ========================================================================

    [Benchmark(Description = "X1: Old simple flag")]
    public OldEvaluationResult X1_Old_Simple() => _oldResolver.Evaluate("simple-bool", _emptyCtx);

    [Benchmark(Description = "X1: New simple flag (pre-eval)")]
    public Eval.EvaluationResult X1_New_Simple() => _newEvaluator.EvaluateFlag("simple-bool", _emptyCtx);

    // ========================================================================
    // X2: Old vs New — Targeting evaluation (single-threaded, small context)
    // ========================================================================

    [Benchmark(Description = "X2: Old targeting")]
    public OldEvaluationResult X2_Old_Targeting() => _oldResolver.Evaluate("targeted-access", _smallCtx);

    [Benchmark(Description = "X2: New targeting")]
    public Eval.EvaluationResult X2_New_Targeting() => _newEvaluator.EvaluateFlag("targeted-access", _smallCtx);

    // ========================================================================
    // X3: Context size sweep — Old resolver
    // ========================================================================

    [Benchmark(Description = "X3: Old empty ctx")]
    public OldEvaluationResult X3_Old_EmptyCtx() => _oldResolver.Evaluate("targeted-access", _emptyCtx);

    [Benchmark(Description = "X3: Old small ctx")]
    public OldEvaluationResult X3_Old_SmallCtx() => _oldResolver.Evaluate("targeted-access", _smallCtx);

    [Benchmark(Description = "X3: Old large ctx")]
    public OldEvaluationResult X3_Old_LargeCtx() => _oldResolver.Evaluate("targeted-access", _largeCtx);

    // ========================================================================
    // X3: Context size sweep — New evaluator
    // ========================================================================

    [Benchmark(Description = "X3: New empty ctx")]
    public Eval.EvaluationResult X3_New_EmptyCtx() => _newEvaluator.EvaluateFlag("targeted-access", _emptyCtx);

    [Benchmark(Description = "X3: New small ctx")]
    public Eval.EvaluationResult X3_New_SmallCtx() => _newEvaluator.EvaluateFlag("targeted-access", _smallCtx);

    [Benchmark(Description = "X3: New large ctx")]
    public Eval.EvaluationResult X3_New_LargeCtx() => _newEvaluator.EvaluateFlag("targeted-access", _largeCtx);

    // ========================================================================
    // X4: Concurrent targeting — 4 threads
    // ========================================================================

    [Benchmark(Description = "X4: Old 4-thread targeting")]
    public void X4_Old_Concurrent() => RunConcurrent(4, () => _oldResolver.Evaluate("targeted-access", _smallCtx));

    [Benchmark(Description = "X4: New 4-thread targeting")]
    public void X4_New_Concurrent() => RunConcurrent(4, () => _newEvaluator.EvaluateFlag("targeted-access", _smallCtx));

    // ========================================================================
    // X5: Concurrent large context — 8 threads
    // ========================================================================

    [Benchmark(Description = "X5: Old 8-thread large ctx")]
    public void X5_Old_ConcurrentLarge() => RunConcurrent(8, () => _oldResolver.Evaluate("targeted-access", _largeCtx));

    [Benchmark(Description = "X5: New 8-thread large ctx")]
    public void X5_New_ConcurrentLarge() => RunConcurrent(8, () => _newEvaluator.EvaluateFlag("targeted-access", _largeCtx));

    private static void RunConcurrent(int threads, Action action)
    {
        var tasks = new Task[threads];
        for (int i = 0; i < threads; i++)
        {
            tasks[i] = Task.Run(action);
        }
        Task.WaitAll(tasks);
    }
}
