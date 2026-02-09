using System.Text.Json.Serialization;

namespace PrivStack.Desktop.Models;

/// <summary>
/// Supported property value types for custom properties.
/// </summary>
[JsonConverter(typeof(JsonStringEnumConverter<PropertyType>))]
public enum PropertyType
{
    Text,
    Number,
    Date,
    Checkbox,
    Select,
    MultiSelect,
    Url
}

/// <summary>
/// A user-defined property definition. Stored as entity_type "property_definition".
/// Properties can be applied to any entity via its entity_metadata record.
/// </summary>
public sealed record PropertyDefinition
{
    [JsonPropertyName("id")]
    public string Id { get; init; } = "";

    [JsonPropertyName("name")]
    public string Name { get; init; } = "";

    [JsonPropertyName("type")]
    public PropertyType Type { get; init; } = PropertyType.Text;

    [JsonPropertyName("description")]
    public string? Description { get; init; }

    /// <summary>
    /// Available options for Select and MultiSelect property types.
    /// </summary>
    [JsonPropertyName("options")]
    public List<string>? Options { get; init; }

    [JsonPropertyName("default_value")]
    public string? DefaultValue { get; init; }

    [JsonPropertyName("icon")]
    public string? Icon { get; init; }

    [JsonPropertyName("sort_order")]
    public int SortOrder { get; init; }

    [JsonPropertyName("group_id")]
    public string? GroupId { get; init; }
}

/// <summary>
/// A grouping container for property definitions. Stored as entity_type "property_group".
/// </summary>
public sealed record PropertyGroup
{
    [JsonPropertyName("id")]
    public string Id { get; init; } = "";

    [JsonPropertyName("name")]
    public string Name { get; init; } = "";

    [JsonPropertyName("sort_order")]
    public int SortOrder { get; init; }
}
