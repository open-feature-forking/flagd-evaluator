using System.Buffers;
using System.Text.Json;

namespace FlagdEvaluator;

/// <summary>
/// Serializes evaluation context to UTF-8 JSON bytes, with optional key filtering.
/// Uses Utf8JsonWriter for zero-copy serialization.
/// </summary>
internal static class ContextSerializer
{
    [ThreadStatic]
    private static ArrayBufferWriter<byte>? t_buffer;

    /// <summary>
    /// Serializes a full context dictionary to UTF-8 JSON bytes.
    /// </summary>
    internal static byte[] Serialize(Dictionary<string, object?>? context)
    {
        if (context == null || context.Count == 0)
            return Array.Empty<byte>();

        var buffer = GetBuffer();
        buffer.Clear();

        using var writer = new Utf8JsonWriter(buffer);
        writer.WriteStartObject();

        foreach (var (key, value) in context)
        {
            writer.WritePropertyName(key);
            WriteValue(writer, value);
        }

        writer.WriteEndObject();
        writer.Flush();

        return buffer.WrittenSpan.ToArray();
    }

    /// <summary>
    /// Serializes a filtered context: only required keys + targetingKey + $flagd enrichment.
    /// </summary>
    internal static byte[] SerializeFiltered(
        Dictionary<string, object?> context,
        HashSet<string> requiredKeys,
        string flagKey)
    {
        var buffer = GetBuffer();
        buffer.Clear();

        using var writer = new Utf8JsonWriter(buffer);
        writer.WriteStartObject();

        // Write required keys from context
        foreach (var key in requiredKeys)
        {
            if (key == "targetingKey" || key == "$flagd.flagKey" || key == "$flagd.timestamp")
                continue; // handled separately

            if (context.TryGetValue(key, out var value))
            {
                writer.WritePropertyName(key);
                WriteValue(writer, value);
            }
        }

        // Always include targetingKey
        writer.WritePropertyName("targetingKey");
        if (context.TryGetValue("targetingKey", out var tk))
            WriteValue(writer, tk);
        else
            writer.WriteStringValue("");

        // $flagd enrichment
        writer.WritePropertyName("$flagd");
        writer.WriteStartObject();
        writer.WriteString("flagKey", flagKey);
        writer.WriteNumber("timestamp", DateTimeOffset.UtcNow.ToUnixTimeSeconds());
        writer.WriteEndObject();

        writer.WriteEndObject();
        writer.Flush();

        return buffer.WrittenSpan.ToArray();
    }

    private static void WriteValue(Utf8JsonWriter writer, object? value)
    {
        switch (value)
        {
            case null:
                writer.WriteNullValue();
                break;
            case bool b:
                writer.WriteBooleanValue(b);
                break;
            case string s:
                writer.WriteStringValue(s);
                break;
            case int i:
                writer.WriteNumberValue(i);
                break;
            case long l:
                writer.WriteNumberValue(l);
                break;
            case double d:
                writer.WriteNumberValue(d);
                break;
            case float f:
                writer.WriteNumberValue(f);
                break;
            case decimal dec:
                writer.WriteNumberValue(dec);
                break;
            case JsonElement je:
                je.WriteTo(writer);
                break;
            default:
                // Fall back to JsonSerializer for complex types
                JsonSerializer.Serialize(writer, value, value.GetType());
                break;
        }
    }

    private static ArrayBufferWriter<byte> GetBuffer()
    {
        return t_buffer ??= new ArrayBufferWriter<byte>(512);
    }
}
