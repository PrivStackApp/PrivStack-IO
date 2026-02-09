using System.Text.Json;
using System.Text.Json.Serialization;

namespace PrivStack.Sdk.Json;

/// <summary>
/// Shared JSON serialization options for all Rust FFI communication.
/// All plugins MUST use these options when serializing payloads for SdkMessage
/// to ensure symmetry with the deserialization path in SdkHost.
///
/// Key behaviors:
/// - Property names use snake_case (matching Rust serde conventions)
/// - DateTimeOffset values serialize as i64 milliseconds since Unix epoch (UTC)
/// - Enum values serialize as snake_case strings
/// </summary>
public static class SdkJsonOptions
{
    public static JsonSerializerOptions Default { get; } = new()
    {
        PropertyNamingPolicy = JsonNamingPolicy.SnakeCaseLower,
        PropertyNameCaseInsensitive = true,
        Converters =
        {
            new UnixTimestampConverter(),
            new JsonStringEnumConverter(JsonNamingPolicy.SnakeCaseLower),
        },
    };
}
