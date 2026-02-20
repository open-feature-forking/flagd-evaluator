using System.Text.Json;
using FluentAssertions;
using Xunit;

namespace FlagdEvaluator.Tests;

public class FlagEvaluatorTests : IDisposable
{
    private readonly FlagEvaluator _evaluator;

    public FlagEvaluatorTests()
    {
        _evaluator = new FlagEvaluator(new FlagEvaluatorOptions
        {
            PermissiveValidation = true,
        });
    }

    public void Dispose() => _evaluator.Dispose();

    private static FlagEvaluator CreateEvaluator(int poolSize = 0, bool permissive = true)
    {
        return new FlagEvaluator(new FlagEvaluatorOptions
        {
            PoolSize = poolSize > 0 ? poolSize : Environment.ProcessorCount,
            PermissiveValidation = permissive,
        });
    }

    [Fact]
    public void SimpleBooleanFlag()
    {
        var config = """
        {
            "flags": {
                "simple-flag": {
                    "state": "ENABLED",
                    "defaultVariant": "on",
                    "variants": {
                        "on": true,
                        "off": false
                    }
                }
            }
        }
        """;

        var result = _evaluator.UpdateState(config);
        result.Success.Should().BeTrue();
        result.ChangedFlags.Should().Contain("simple-flag");

        var eval = _evaluator.EvaluateFlag("simple-flag", new Dictionary<string, object?>());
        eval.Value!.Value.GetBoolean().Should().BeTrue();
        eval.Variant.Should().Be("on");
        eval.Reason.Should().Be("STATIC");
        eval.IsError.Should().BeFalse();
    }

    [Fact]
    public void StringFlag()
    {
        var config = """
        {
            "flags": {
                "color-flag": {
                    "state": "ENABLED",
                    "defaultVariant": "red",
                    "variants": {
                        "red": "red",
                        "blue": "blue",
                        "green": "green"
                    }
                }
            }
        }
        """;

        var result = _evaluator.UpdateState(config);
        result.Success.Should().BeTrue();

        var eval = _evaluator.EvaluateFlag("color-flag", new Dictionary<string, object?>());
        eval.Value!.Value.GetString().Should().Be("red");
        eval.Variant.Should().Be("red");
    }

    [Fact]
    public void NumericFlag()
    {
        var config = """
        {
            "flags": {
                "number-flag": {
                    "state": "ENABLED",
                    "defaultVariant": "default",
                    "variants": {
                        "default": 42,
                        "large": 1000
                    }
                }
            }
        }
        """;

        _evaluator.UpdateState(config);

        var eval = _evaluator.EvaluateFlag("number-flag", new Dictionary<string, object?>());
        eval.Value!.Value.GetInt32().Should().Be(42);
    }

    [Fact]
    public void TargetingRule_Match()
    {
        var config = """
        {
            "flags": {
                "user-flag": {
                    "state": "ENABLED",
                    "defaultVariant": "default",
                    "variants": {
                        "default": false,
                        "premium": true
                    },
                    "targeting": {
                        "if": [
                            { "==": [{ "var": "email" }, "premium@example.com"] },
                            "premium",
                            null
                        ]
                    }
                }
            }
        }
        """;

        _evaluator.UpdateState(config);

        var ctx = new Dictionary<string, object?> { ["email"] = "premium@example.com" };
        var eval = _evaluator.EvaluateFlag("user-flag", ctx);
        eval.Value!.Value.GetBoolean().Should().BeTrue();
        eval.Variant.Should().Be("premium");
        eval.Reason.Should().Be("TARGETING_MATCH");
    }

    [Fact]
    public void TargetingRule_NoMatch()
    {
        var config = """
        {
            "flags": {
                "user-flag": {
                    "state": "ENABLED",
                    "defaultVariant": "default",
                    "variants": {
                        "default": false,
                        "premium": true
                    },
                    "targeting": {
                        "if": [
                            { "==": [{ "var": "email" }, "premium@example.com"] },
                            "premium",
                            null
                        ]
                    }
                }
            }
        }
        """;

        _evaluator.UpdateState(config);

        var ctx = new Dictionary<string, object?> { ["email"] = "regular@example.com" };
        var eval = _evaluator.EvaluateFlag("user-flag", ctx);
        eval.Value!.Value.GetBoolean().Should().BeFalse();
        eval.Variant.Should().Be("default");
    }

    [Fact]
    public void FlagNotFound()
    {
        _evaluator.UpdateState("""{"flags": {}}""");

        var eval = _evaluator.EvaluateFlag("nonexistent-flag", new Dictionary<string, object?>());
        eval.Reason.Should().Be("FLAG_NOT_FOUND");
    }

    [Fact]
    public void DisabledFlag()
    {
        var config = """
        {
            "flags": {
                "disabled-flag": {
                    "state": "DISABLED",
                    "defaultVariant": "off",
                    "variants": {
                        "on": true,
                        "off": false
                    }
                }
            }
        }
        """;

        _evaluator.UpdateState(config);

        var eval = _evaluator.EvaluateFlag("disabled-flag", new Dictionary<string, object?>());
        eval.Reason.Should().Be("DISABLED");
        // Disabled flags return null value
        (eval.Value == null || eval.Value.Value.ValueKind == JsonValueKind.Null).Should().BeTrue();
    }

    [Fact]
    public void ContextEnrichment()
    {
        var config = """
        {
            "flags": {
                "targeting-key-flag": {
                    "state": "ENABLED",
                    "defaultVariant": "default",
                    "variants": {
                        "default": "unknown",
                        "known": "known-user"
                    },
                    "targeting": {
                        "if": [
                            { "!=": [{ "var": "targetingKey" }, ""] },
                            "known",
                            null
                        ]
                    }
                }
            }
        }
        """;

        _evaluator.UpdateState(config);

        var ctx = new Dictionary<string, object?> { ["targetingKey"] = "user-123" };
        var eval = _evaluator.EvaluateFlag("targeting-key-flag", ctx);
        eval.Value!.Value.GetString().Should().Be("known-user");
        eval.Reason.Should().Be("TARGETING_MATCH");
    }

    [Fact]
    public void UpdateStateChangedFlags()
    {
        var config1 = """
        {
            "flags": {
                "flag-a": {
                    "state": "ENABLED",
                    "defaultVariant": "on",
                    "variants": { "on": true }
                }
            }
        }
        """;

        var result1 = _evaluator.UpdateState(config1);
        result1.Success.Should().BeTrue();
        result1.ChangedFlags.Should().Contain("flag-a");

        var config2 = """
        {
            "flags": {
                "flag-a": {
                    "state": "DISABLED",
                    "defaultVariant": "off",
                    "variants": { "off": false }
                },
                "flag-b": {
                    "state": "ENABLED",
                    "defaultVariant": "on",
                    "variants": { "on": true }
                }
            }
        }
        """;

        var result2 = _evaluator.UpdateState(config2);
        result2.Success.Should().BeTrue();
        result2.ChangedFlags.Should().Contain("flag-a");
        result2.ChangedFlags.Should().Contain("flag-b");
    }

    [Fact]
    public void RequiredContextKeys()
    {
        var config = """
        {
            "flags": {
                "targeted-flag": {
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
                "static-flag": {
                    "state": "ENABLED",
                    "defaultVariant": "on",
                    "variants": { "on": true }
                }
            }
        }
        """;

        var result = _evaluator.UpdateState(config);
        result.Success.Should().BeTrue();

        result.RequiredContextKeys.Should().ContainKey("targeted-flag");
        result.RequiredContextKeys!["targeted-flag"].Should().Contain("email");
        result.RequiredContextKeys["targeted-flag"].Should().Contain("targetingKey");

        // Static flags should not have required context keys
        result.RequiredContextKeys.Should().NotContainKey("static-flag");
    }

    [Fact]
    public void FlagIndices()
    {
        var config = """
        {
            "flags": {
                "flagB": {
                    "state": "ENABLED",
                    "defaultVariant": "on",
                    "variants": { "on": true }
                },
                "flagA": {
                    "state": "ENABLED",
                    "defaultVariant": "off",
                    "variants": { "off": false }
                }
            }
        }
        """;

        var result = _evaluator.UpdateState(config);
        result.Success.Should().BeTrue();
        result.FlagIndices.Should().ContainKey("flagA");
        result.FlagIndices.Should().ContainKey("flagB");
        // Indices should be in sorted order
        result.FlagIndices!["flagA"].Should().Be(0u);
        result.FlagIndices["flagB"].Should().Be(1u);
    }

    [Fact]
    public void FilteredContextEvaluation()
    {
        var config = """
        {
            "flags": {
                "email-flag": {
                    "state": "ENABLED",
                    "defaultVariant": "default",
                    "variants": { "default": false, "premium": true },
                    "targeting": {
                        "if": [
                            { "==": [{ "var": "email" }, "admin@example.com"] },
                            "premium", null
                        ]
                    }
                }
            }
        }
        """;

        _evaluator.UpdateState(config);

        // Large context — only "email" matters
        var ctx = new Dictionary<string, object?>
        {
            ["targetingKey"] = "user-123",
            ["email"] = "admin@example.com",
            ["name"] = "Admin User",
            ["age"] = 30,
            ["country"] = "US",
            ["tier"] = "premium",
            ["department"] = "engineering",
        };

        var eval = _evaluator.EvaluateFlag("email-flag", ctx);
        eval.Value!.Value.GetBoolean().Should().BeTrue();
        eval.Variant.Should().Be("premium");
        eval.Reason.Should().Be("TARGETING_MATCH");

        // Non-matching email
        var ctx2 = new Dictionary<string, object?>
        {
            ["targetingKey"] = "user-456",
            ["email"] = "regular@example.com",
            ["name"] = "Regular User",
            ["age"] = 25,
        };
        var eval2 = _evaluator.EvaluateFlag("email-flag", ctx2);
        eval2.Value!.Value.GetBoolean().Should().BeFalse();
        eval2.Variant.Should().Be("default");
    }

    [Fact]
    public void PreEvaluatedCache()
    {
        var config = """
        {
            "flags": {
                "static-flag": {
                    "state": "ENABLED",
                    "defaultVariant": "on",
                    "variants": { "on": true, "off": false }
                },
                "disabled-flag": {
                    "state": "DISABLED",
                    "defaultVariant": "on",
                    "variants": { "on": true, "off": false }
                }
            }
        }
        """;

        _evaluator.UpdateState(config);

        // These should be served from pre-evaluated cache (context is ignored)
        var ctx = new Dictionary<string, object?>
        {
            ["targetingKey"] = "user-1",
            ["anything"] = "value",
        };

        var eval = _evaluator.EvaluateFlag("static-flag", ctx);
        eval.Value!.Value.GetBoolean().Should().BeTrue();
        eval.Reason.Should().Be("STATIC");

        var eval2 = _evaluator.EvaluateFlag("disabled-flag", ctx);
        eval2.Reason.Should().Be("DISABLED");
    }

    [Fact]
    public void ConcurrentAccess()
    {
        var config = """
        {
            "flags": {
                "concurrent-flag": {
                    "state": "ENABLED",
                    "defaultVariant": "on",
                    "variants": { "on": true, "off": false },
                    "targeting": {
                        "if": [
                            { "==": [{ "var": "user" }, "admin"] },
                            "on", "off"
                        ]
                    }
                }
            }
        }
        """;

        _evaluator.UpdateState(config);

        var errors = new System.Collections.Concurrent.ConcurrentBag<string>();
        var tasks = new Task[20];

        for (int i = 0; i < 20; i++)
        {
            int id = i;
            tasks[i] = Task.Run(() =>
            {
                var ctx = new Dictionary<string, object?> { ["user"] = "admin" };
                var result = _evaluator.EvaluateFlag("concurrent-flag", ctx);
                if (!result.Value!.Value.GetBoolean())
                    errors.Add($"Thread {id}: expected true, got {result.Value}");
            });
        }

        Task.WaitAll(tasks);
        errors.Should().BeEmpty();
    }

    [Fact]
    public void TypedEvaluators()
    {
        var config = """
        {
            "flags": {
                "bool-flag": {
                    "state": "ENABLED",
                    "defaultVariant": "on",
                    "variants": { "on": true, "off": false }
                },
                "string-flag": {
                    "state": "ENABLED",
                    "defaultVariant": "hello",
                    "variants": { "hello": "world" }
                },
                "int-flag": {
                    "state": "ENABLED",
                    "defaultVariant": "val",
                    "variants": { "val": 42 }
                },
                "float-flag": {
                    "state": "ENABLED",
                    "defaultVariant": "val",
                    "variants": { "val": 3.14 }
                }
            }
        }
        """;

        _evaluator.UpdateState(config);
        var ctx = new Dictionary<string, object?>();

        _evaluator.EvaluateBool("bool-flag", ctx, false).Should().BeTrue();
        _evaluator.EvaluateString("string-flag", ctx, "default").Should().Be("world");
        _evaluator.EvaluateInt("int-flag", ctx, 0).Should().Be(42);
        _evaluator.EvaluateDouble("float-flag", ctx, 0.0).Should().Be(3.14);

        // Test defaults for missing flags
        _evaluator.EvaluateBool("missing", ctx, true).Should().BeTrue();
        _evaluator.EvaluateString("missing", ctx, "fallback").Should().Be("fallback");
        _evaluator.EvaluateInt("missing", ctx, 99).Should().Be(99);
        _evaluator.EvaluateDouble("missing", ctx, 1.5).Should().Be(1.5);
    }

    [Fact]
    public void GenerationGuard()
    {
        // Small pool to increase contention window
        using var eval = CreateEvaluator(poolSize: 2);

        var configA = """
        {
            "flags": {
                "aaa-pad-1": {
                    "state": "ENABLED",
                    "defaultVariant": "v",
                    "variants": { "v": "PADDING_A1" }
                },
                "aaa-pad-2": {
                    "state": "ENABLED",
                    "defaultVariant": "v",
                    "variants": { "v": "PADDING_A2" }
                },
                "probe": {
                    "state": "ENABLED",
                    "defaultVariant": "no",
                    "variants": { "yes": "AAA", "no": "default-A" },
                    "targeting": {
                        "if": [{ "==": [{ "var": "tier" }, "premium"] }, "yes", null]
                    }
                }
            }
        }
        """;

        var configB = """
        {
            "flags": {
                "zzz-pad-3": {
                    "state": "ENABLED",
                    "defaultVariant": "v",
                    "variants": { "v": "PADDING_B3" }
                },
                "zzz-pad-4": {
                    "state": "ENABLED",
                    "defaultVariant": "v",
                    "variants": { "v": "PADDING_B4" }
                },
                "zzz-pad-5": {
                    "state": "ENABLED",
                    "defaultVariant": "v",
                    "variants": { "v": "PADDING_B5" }
                },
                "probe": {
                    "state": "ENABLED",
                    "defaultVariant": "no",
                    "variants": { "yes": "BBB", "no": "default-B" },
                    "targeting": {
                        "if": [{ "==": [{ "var": "tier" }, "premium"] }, "yes", null]
                    }
                }
            }
        }
        """;

        var validValues = new HashSet<string> { "AAA", "BBB", "default-A", "default-B" };

        eval.UpdateState(configA);

        var errors = new System.Collections.Concurrent.ConcurrentBag<string>();
        var cts = new CancellationTokenSource();
        var evalTasks = new Task[8];

        // Evaluator threads: continuously evaluate "probe" and verify results
        for (int g = 0; g < evalTasks.Length; g++)
        {
            int id = g;
            evalTasks[g] = Task.Run(() =>
            {
                var ctx = new Dictionary<string, object?> { ["tier"] = "premium", ["targetingKey"] = "user-1" };
                while (!cts.IsCancellationRequested)
                {
                    var result = eval.EvaluateFlag("probe", ctx);
                    var val = result.Value.HasValue ? result.Value.Value.GetString() : null;
                    if (val == null || !validValues.Contains(val))
                    {
                        errors.Add($"Thread {id}: INVALID value '{val}' (variant={result.Variant} reason={result.Reason})");
                    }
                }
            });
        }

        // Updater: rapidly alternate configs to maximize the race window
        for (int i = 0; i < 50; i++)
        {
            eval.UpdateState(i % 2 == 0 ? configB : configA);
        }

        cts.Cancel();
        Task.WaitAll(evalTasks);

        errors.Should().BeEmpty();
    }

    [Fact]
    public void PoolSize1()
    {
        // Regression test: single instance pool should work correctly
        using var eval = CreateEvaluator(poolSize: 1);

        var config = """
        {
            "flags": {
                "single-pool-flag": {
                    "state": "ENABLED",
                    "defaultVariant": "on",
                    "variants": { "on": true, "off": false },
                    "targeting": {
                        "if": [
                            { "==": [{ "var": "user" }, "admin"] },
                            "on", "off"
                        ]
                    }
                }
            }
        }
        """;

        eval.UpdateState(config);

        var ctx = new Dictionary<string, object?> { ["user"] = "admin" };
        var result = eval.EvaluateFlag("single-pool-flag", ctx);
        result.Value!.Value.GetBoolean().Should().BeTrue();
        result.Variant.Should().Be("on");
        result.Reason.Should().Be("TARGETING_MATCH");
    }
}
