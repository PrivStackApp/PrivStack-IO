using System.Text.Json;
using System.Text.Json.Serialization;

namespace PrivStack.Sdk;

/// <summary>
/// A structured message sent from a plugin to the SDK host for routing to the Rust core.
/// </summary>
public sealed record SdkMessage
{
    /// <summary>
    /// The plugin sending this message (e.g., "privstack.notes").
    /// </summary>
    [JsonPropertyName("plugin_id")]
    public required string PluginId { get; init; }

    /// <summary>
    /// The CRUD action to perform.
    /// </summary>
    [JsonPropertyName("action")]
    public required SdkAction Action { get; init; }

    /// <summary>
    /// The entity type to operate on (e.g., "page", "task", "event", "credential").
    /// </summary>
    [JsonPropertyName("entity_type")]
    public required string EntityType { get; init; }

    /// <summary>
    /// Optional entity ID for read/update/delete operations.
    /// </summary>
    [JsonPropertyName("entity_id")]
    public string? EntityId { get; init; }

    /// <summary>
    /// Optional JSON payload for create/update operations.
    /// </summary>
    [JsonPropertyName("payload")]
    public string? Payload { get; init; }

    /// <summary>
    /// Optional additional parameters for queries and commands.
    /// </summary>
    [JsonPropertyName("parameters")]
    public Dictionary<string, string>? Parameters { get; init; }
}

/// <summary>
/// CRUD + query actions supported by the SDK.
/// </summary>
[JsonConverter(typeof(SdkActionConverter))]
public enum SdkAction
{
    Create,
    Read,
    ReadList,
    Count,
    Update,
    Delete,
    Query,
    Command,
    Trash,
    Restore,
    Link,
    Unlink,
    GetLinks
}

/// <summary>
/// Serializes SdkAction as snake_case_lower to match the Rust FFI expectations.
/// </summary>
internal sealed class SdkActionConverter : JsonStringEnumConverter<SdkAction>
{
    public SdkActionConverter() : base(JsonNamingPolicy.SnakeCaseLower) { }
}
