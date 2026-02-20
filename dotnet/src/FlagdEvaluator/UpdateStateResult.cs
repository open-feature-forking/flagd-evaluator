using System.Text.Json.Serialization;

namespace FlagdEvaluator;

/// <summary>
/// Contains the result of updating flag state.
/// </summary>
public sealed class UpdateStateResult
{
    [JsonPropertyName("success")]
    public bool Success { get; set; }

    [JsonPropertyName("error")]
    public string? Error { get; set; }

    [JsonPropertyName("changedFlags")]
    public List<string>? ChangedFlags { get; set; }

    [JsonPropertyName("preEvaluated")]
    public Dictionary<string, EvaluationResult>? PreEvaluated { get; set; }

    [JsonPropertyName("requiredContextKeys")]
    public Dictionary<string, List<string>>? RequiredContextKeys { get; set; }

    [JsonPropertyName("flagIndices")]
    public Dictionary<string, uint>? FlagIndices { get; set; }
}
