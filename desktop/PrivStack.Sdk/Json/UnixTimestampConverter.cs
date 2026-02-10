using System.Globalization;
using System.Text.Json;
using System.Text.Json.Serialization;

namespace PrivStack.Sdk.Json;

/// <summary>
/// Converts Unix timestamps (milliseconds since epoch) to DateTimeOffset.
/// Handles both number and string formats for flexibility.
/// Rust stores all timestamps as i64 millis (UTC); this converter bridges
/// that representation to the .NET DateTimeOffset type used by plugins.
/// </summary>
public sealed class UnixTimestampConverter : JsonConverter<DateTimeOffset>
{
    public override DateTimeOffset Read(ref Utf8JsonReader reader, Type typeToConvert, JsonSerializerOptions options)
    {
        return reader.TokenType switch
        {
            JsonTokenType.Number => DateTimeOffset.FromUnixTimeMilliseconds(reader.GetInt64()),
            JsonTokenType.String => DateTimeOffset.TryParse(reader.GetString(), CultureInfo.InvariantCulture, DateTimeStyles.None, out var dt)
                ? dt
                : DateTimeOffset.FromUnixTimeMilliseconds(long.Parse(reader.GetString()!, CultureInfo.InvariantCulture)),
            _ => throw new JsonException($"Cannot convert {reader.TokenType} to DateTimeOffset")
        };
    }

    public override void Write(Utf8JsonWriter writer, DateTimeOffset value, JsonSerializerOptions options)
    {
        writer.WriteNumberValue(value.ToUnixTimeMilliseconds());
    }
}
