using System.Text.Json;

namespace PrivStack.Desktop.Models;

/// <summary>
/// Universal metadata for any entity — loaded from a separate entity_metadata record
/// so plugins don't need to know about tags/properties.
/// </summary>
public sealed record EntityMetadata(
    string EntityId,
    string LinkType,
    string? Title,
    string? Preview,
    DateTimeOffset? CreatedAt,
    DateTimeOffset? ModifiedAt,
    string? ParentId,
    string? ParentTitle,
    List<string> Tags,
    Dictionary<string, JsonElement> Properties);
