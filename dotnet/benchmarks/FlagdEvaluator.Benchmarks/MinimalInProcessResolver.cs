using System.Text.Json;
using System.Text.Json.Nodes;
using Json.Logic;

namespace FlagdEvaluator.Benchmarks;

/// <summary>
/// Minimal in-process resolver extracted from the old dotnet-sdk-contrib flagd provider.
/// Uses json-everything's JsonLogic for targeting evaluation — the same library as the
/// production OpenFeature.Contrib.Providers.Flagd package.
///
/// Contains ONLY the core evaluation path for fair benchmarking:
/// 1. Parse targeting rule from string → JsonNode (per evaluation, like the old provider)
/// 2. Serialize context dictionary → JSON string → JsonNode (per evaluation)
/// 3. JsonLogic.Apply(rule, data) → variant name
/// 4. Variant lookup → value
///
/// No FlagStore, no sync connectors, no event handling.
/// </summary>
internal sealed class MinimalInProcessResolver
{
    private readonly Dictionary<string, FeatureFlag> _flags = new();

    internal void LoadFlags(string jsonConfig)
    {
        using var doc = JsonDocument.Parse(jsonConfig);
        var flagsObj = doc.RootElement.GetProperty("flags");

        foreach (var flagProp in flagsObj.EnumerateObject())
        {
            var flagKey = flagProp.Name;
            var flagData = flagProp.Value;

            var state = flagData.GetProperty("state").GetString()!;
            var defaultVariant = flagData.GetProperty("defaultVariant").GetString()!;

            var variants = new Dictionary<string, JsonElement>();
            foreach (var v in flagData.GetProperty("variants").EnumerateObject())
            {
                variants[v.Name] = v.Value.Clone();
            }

            string? targeting = null;
            if (flagData.TryGetProperty("targeting", out var targetingEl) &&
                targetingEl.ValueKind != JsonValueKind.Null)
            {
                targeting = targetingEl.GetRawText();
            }

            _flags[flagKey] = new FeatureFlag(state, defaultVariant, variants, targeting);
        }
    }

    /// <summary>
    /// Evaluate a flag following the old provider's hot path.
    /// This intentionally re-parses the targeting rule and re-serializes context
    /// on every call, matching the old provider's behavior.
    /// </summary>
    internal OldEvaluationResult Evaluate(string flagKey, Dictionary<string, object?>? context)
    {
        if (!_flags.TryGetValue(flagKey, out var flag))
        {
            return new OldEvaluationResult
            {
                Reason = "FLAG_NOT_FOUND",
                ErrorCode = "FLAG_NOT_FOUND",
                ErrorMessage = $"flag: {flagKey} not found",
            };
        }

        if (flag.State == "DISABLED")
        {
            return new OldEvaluationResult
            {
                Reason = "DISABLED",
                ErrorCode = "FLAG_NOT_FOUND",
                ErrorMessage = $"flag: {flagKey} is disabled",
            };
        }

        string resolvedVariant;
        string reason;

        if (flag.Targeting == null)
        {
            resolvedVariant = flag.DefaultVariant;
            reason = "STATIC";
        }
        else
        {
            // Step 1: Parse targeting rule from string (per-evaluation, like old provider)
            var rule = JsonNode.Parse(flag.Targeting);

            // Step 2: Serialize context to JSON string then parse to JsonNode
            // This mirrors the old provider's ConvertToDynamicObject → Serialize → Parse path
            JsonNode? data;
            if (context != null && context.Count > 0)
            {
                var contextJson = JsonSerializer.Serialize(context);
                data = JsonNode.Parse(contextJson);
            }
            else
            {
                data = JsonNode.Parse("{}");
            }

            // Step 3: Evaluate using json-everything's JsonLogic
            var result = JsonLogic.Apply(rule, data);

            if (result == null)
            {
                resolvedVariant = flag.DefaultVariant;
                reason = "DEFAULT";
            }
            else
            {
                resolvedVariant = result.ToString();
                reason = "TARGETING_MATCH";
            }
        }

        if (!flag.Variants.TryGetValue(resolvedVariant, out var value))
        {
            return new OldEvaluationResult
            {
                Reason = "ERROR",
                ErrorCode = "GENERAL",
                ErrorMessage = $"variant {resolvedVariant} not found in flag {flagKey}",
            };
        }

        return new OldEvaluationResult
        {
            Value = value,
            Variant = resolvedVariant,
            Reason = reason,
        };
    }

    internal sealed class FeatureFlag
    {
        public string State { get; }
        public string DefaultVariant { get; }
        public Dictionary<string, JsonElement> Variants { get; }
        public string? Targeting { get; }

        public FeatureFlag(string state, string defaultVariant,
            Dictionary<string, JsonElement> variants, string? targeting)
        {
            State = state;
            DefaultVariant = defaultVariant;
            Variants = variants;
            Targeting = targeting;
        }
    }
}

public sealed class OldEvaluationResult
{
    public JsonElement? Value { get; init; }
    public string? Variant { get; init; }
    public string? Reason { get; init; }
    public string? ErrorCode { get; init; }
    public string? ErrorMessage { get; init; }
}
