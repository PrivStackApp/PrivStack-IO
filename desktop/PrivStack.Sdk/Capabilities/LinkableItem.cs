namespace PrivStack.Sdk.Capabilities;

/// <summary>
/// Plugin-agnostic representation of an item that can be linked to from other plugins.
/// </summary>
public sealed record LinkableItem
{
    public required string Id { get; init; }
    public required string LinkType { get; init; }
    public required string Title { get; init; }
    public string? Subtitle { get; init; }
    public string? Icon { get; init; }
    public DateTime? ModifiedAt { get; init; }
    public IReadOnlyDictionary<string, string>? Metadata { get; init; }
}
