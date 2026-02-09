namespace PrivStack.UI.Adaptive.Models;

/// <summary>
/// UI-layer DTO for linkable items returned by the host's search delegate.
/// Avoids coupling the renderer to PrivStack.Sdk.
/// </summary>
public sealed record LinkableItemResult(
    string Id,
    string LinkType,
    string LinkTypeDisplayName,
    string Title,
    string? Subtitle,
    string? Icon);
