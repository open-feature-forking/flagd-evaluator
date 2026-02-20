using System.Text.Json;
using System.Text.Json.Serialization;

namespace FlagdEvaluator;

/// <summary>
/// Contains the result of a flag evaluation.
/// </summary>
public sealed class EvaluationResult
{
    [JsonPropertyName("value")]
    public JsonElement? Value { get; set; }

    [JsonPropertyName("variant")]
    public string Variant { get; set; } = "";

    [JsonPropertyName("reason")]
    public string Reason { get; set; } = "";

    [JsonPropertyName("errorCode")]
    public string? ErrorCode { get; set; }

    [JsonPropertyName("errorMessage")]
    public string? ErrorMessage { get; set; }

    [JsonPropertyName("flagMetadata")]
    public Dictionary<string, JsonElement>? FlagMetadata { get; set; }

    /// <summary>
    /// Returns true if the evaluation resulted in an error.
    /// </summary>
    public bool IsError => !string.IsNullOrEmpty(ErrorCode);
}
