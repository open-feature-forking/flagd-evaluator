using BenchmarkDotNet.Attributes;
using BenchmarkDotNet.Running;
using BenchmarkDotNet.Configs;
using Eval = FlagdEvaluator;

namespace FlagdEvaluator.Benchmarks;

[MemoryDiagnoser]
public class EvaluationBenchmarks
{
    private Eval.FlagEvaluator _evaluator = null!;
    private Dictionary<string, object?> _emptyCtx = null!;
    private Dictionary<string, object?> _smallCtx = null!;
    private Dictionary<string, object?> _largeCtx = null!;

    private const string Config = """
    {
        "flags": {
            "simple-flag": {
                "state": "ENABLED",
                "defaultVariant": "on",
                "variants": { "on": true, "off": false }
            },
            "simple-targeting": {
                "state": "ENABLED",
                "defaultVariant": "off",
                "variants": { "on": true, "off": false },
                "targeting": {
                    "if": [
                        { "==": [{ "var": "email" }, "admin@example.com"] },
                        "on", "off"
                    ]
                }
            },
            "complex-targeting": {
                "state": "ENABLED",
                "defaultVariant": "off",
                "variants": { "on": true, "off": false },
                "targeting": {
                    "if": [
                        { "and": [
                            { "==": [{ "var": "tier" }, "premium"] },
                            { ">=": [{ "var": "age" }, 18] },
                            { "in": [{ "var": "country" }, ["US", "CA", "UK"]] }
                        ]},
                        "on", "off"
                    ]
                }
            }
        }
    }
    """;

    [GlobalSetup]
    public void Setup()
    {
        _evaluator = new Eval.FlagEvaluator(new Eval.FlagEvaluatorOptions
        {
            PermissiveValidation = true,
        });
        _evaluator.UpdateState(Config);

        _emptyCtx = new Dictionary<string, object?>();

        _smallCtx = new Dictionary<string, object?>
        {
            ["targetingKey"] = "user-123",
            ["email"] = "admin@example.com",
            ["tier"] = "premium",
            ["age"] = 25,
            ["country"] = "US",
        };

        _largeCtx = new Dictionary<string, object?> { ["targetingKey"] = "user-123" };
        for (int i = 0; i < 100; i++)
        {
            _largeCtx[$"attr_{i}"] = $"value_{i}";
        }
        _largeCtx["email"] = "admin@example.com";
        _largeCtx["tier"] = "premium";
        _largeCtx["age"] = 25;
        _largeCtx["country"] = "US";
    }

    [GlobalCleanup]
    public void Cleanup() => _evaluator.Dispose();

    // E1: Simple flag, empty context (pre-eval baseline)
    [Benchmark(Description = "E1: PreEvaluated (static flag)")]
    public Eval.EvaluationResult E1_PreEvaluated() => _evaluator.EvaluateFlag("simple-flag", _emptyCtx);

    // E4: Simple targeting, small context
    [Benchmark(Description = "E4: SimpleTargeting SmallCtx")]
    public Eval.EvaluationResult E4_SimpleTargetingSmall() => _evaluator.EvaluateFlag("simple-targeting", _smallCtx);

    // E5: Simple targeting, large context (measures filtering)
    [Benchmark(Description = "E5: SimpleTargeting LargeCtx")]
    public Eval.EvaluationResult E5_SimpleTargetingLarge() => _evaluator.EvaluateFlag("simple-targeting", _largeCtx);

    // E6: Complex targeting, small context
    [Benchmark(Description = "E6: ComplexTargeting SmallCtx")]
    public Eval.EvaluationResult E6_ComplexTargetingSmall() => _evaluator.EvaluateFlag("complex-targeting", _smallCtx);

    // E7: Complex targeting, large context
    [Benchmark(Description = "E7: ComplexTargeting LargeCtx")]
    public Eval.EvaluationResult E7_ComplexTargetingLarge() => _evaluator.EvaluateFlag("complex-targeting", _largeCtx);
}

[MemoryDiagnoser]
public class ConcurrencyBenchmarks
{
    private Eval.FlagEvaluator _evaluator = null!;

    private const string Config = """
    {
        "flags": {
            "targeting-flag": {
                "state": "ENABLED",
                "defaultVariant": "off",
                "variants": { "on": true, "off": false },
                "targeting": {
                    "if": [
                        { "==": [{ "var": "email" }, "admin@example.com"] },
                        "on", "off"
                    ]
                }
            }
        }
    }
    """;

    [GlobalSetup]
    public void Setup()
    {
        _evaluator = new Eval.FlagEvaluator(new Eval.FlagEvaluatorOptions
        {
            PermissiveValidation = true,
        });
        _evaluator.UpdateState(Config);
    }

    [GlobalCleanup]
    public void Cleanup() => _evaluator.Dispose();

    [Benchmark(Description = "C1: 1 thread targeting")]
    public void C1_SingleThread()
    {
        var ctx = new Dictionary<string, object?> { ["email"] = "admin@example.com" };
        _evaluator.EvaluateFlag("targeting-flag", ctx);
    }

    [Benchmark(Description = "C2: 4 threads targeting")]
    public void C2_FourThreads()
    {
        RunConcurrent(4);
    }

    [Benchmark(Description = "C3: 8 threads targeting")]
    public void C3_EightThreads()
    {
        RunConcurrent(8);
    }

    private void RunConcurrent(int threads)
    {
        var tasks = new Task[threads];
        for (int i = 0; i < threads; i++)
        {
            tasks[i] = Task.Run(() =>
            {
                var ctx = new Dictionary<string, object?> { ["email"] = "admin@example.com" };
                _evaluator.EvaluateFlag("targeting-flag", ctx);
            });
        }
        Task.WaitAll(tasks);
    }
}

public class Program
{
    public static void Main(string[] args)
    {
        BenchmarkSwitcher
            .FromTypes([typeof(EvaluationBenchmarks), typeof(ConcurrencyBenchmarks), typeof(ComparisonBenchmarks)])
            .Run(args);
    }
}
