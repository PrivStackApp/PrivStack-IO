using System.Text.Json.Serialization;

namespace PrivStack.Desktop.Models;

/// <summary>
/// A reusable template that bundles a set of property definitions with optional default values.
/// Stored as entity_type "property_template".
/// When applied to an entity, missing property definitions are created and default values stamped.
/// </summary>
public sealed record PropertyTemplate
{
    [JsonPropertyName("id")]
    public string Id { get; init; } = "";

    [JsonPropertyName("name")]
    public string Name { get; init; } = "";

    [JsonPropertyName("description")]
    public string? Description { get; init; }

    [JsonPropertyName("icon")]
    public string? Icon { get; init; }

    /// <summary>
    /// Property entries in this template, each referencing a property definition
    /// and optionally specifying a default value to stamp when applied.
    /// </summary>
    [JsonPropertyName("entries")]
    public List<PropertyTemplateEntry> Entries { get; init; } = [];
}

/// <summary>
/// A single property entry within a template.
/// </summary>
public sealed record PropertyTemplateEntry
{
    /// <summary>
    /// References PropertyDefinition.Id. If the definition doesn't exist yet,
    /// it will be created from InlineDefinition when the template is applied.
    /// </summary>
    [JsonPropertyName("property_def_id")]
    public string PropertyDefId { get; init; } = "";

    /// <summary>
    /// Default value to stamp when the template is applied.
    /// Serialized as a string — coerced to the appropriate type at application time.
    /// </summary>
    [JsonPropertyName("default_value")]
    public string? DefaultValue { get; init; }

    /// <summary>
    /// If PropertyDefId doesn't resolve to an existing definition,
    /// this inline definition is used to create one automatically.
    /// </summary>
    [JsonPropertyName("inline_definition")]
    public PropertyDefinition? InlineDefinition { get; init; }
}
