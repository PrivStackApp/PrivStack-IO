using System.Text.Json.Serialization;

namespace PrivStack.Desktop.Models;

/// <summary>
/// A palette definition from a plugin's command_palettes.json.
/// Each palette is a filterable list of items shown in the command palette overlay.
/// </summary>
public sealed record PluginPaletteDefinition(
    [property: JsonPropertyName("id")] string Id,
    [property: JsonPropertyName("title")] string Title,
    [property: JsonPropertyName("placeholder")] string Placeholder,
    [property: JsonPropertyName("shortcut")] string? Shortcut,
    [property: JsonPropertyName("items")] List<PluginPaletteItem> Items)
{
    /// <summary>
    /// Plugin that owns this palette. Set at registration time, not from JSON.
    /// </summary>
    [JsonIgnore]
    public string PluginId { get; init; } = "";
}

/// <summary>
/// A single item within a plugin palette.
/// </summary>
public sealed record PluginPaletteItem(
    [property: JsonPropertyName("id")] string Id,
    [property: JsonPropertyName("name")] string Name,
    [property: JsonPropertyName("description")] string Description,
    [property: JsonPropertyName("icon")] string? Icon,
    [property: JsonPropertyName("keywords")] string Keywords,
    [property: JsonPropertyName("command")] string Command,
    [property: JsonPropertyName("args")] string ArgsJson);
