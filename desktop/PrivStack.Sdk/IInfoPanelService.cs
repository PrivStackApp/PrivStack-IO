namespace PrivStack.Sdk;

/// <summary>
/// A key-value detail field that plugins can pass to the info panel
/// for display alongside the entity's metadata.
/// </summary>
/// <param name="Label">Display label (e.g. "Priority", "Due Date").</param>
/// <param name="Value">Display value (e.g. "High", "Feb 14, 2026").</param>
/// <param name="Color">Optional hex color for the value badge (e.g. "#D08040").</param>
public sealed record InfoPanelDetailField(string Label, string Value, string? Color = null);

/// <summary>
/// Lightweight backlink entry exposed to plugins.
/// Represents an item that directly links TO a queried entity.
/// </summary>
/// <param name="SourceId">ID of the item that links to the target.</param>
/// <param name="SourceLinkType">Link type of the source (e.g. "task", "page").</param>
/// <param name="SourceTitle">Display title of the source item.</param>
public sealed record BacklinkInfo(string SourceId, string SourceLinkType, string SourceTitle);

/// <summary>
/// Service for plugins to report their currently active/selected item
/// to the shell's universal info panel (backlinks + local graph).
/// </summary>
public interface IInfoPanelService
{
    /// <summary>
    /// Sets the currently active item. The info panel will load
    /// backlinks and local graph data for this item.
    /// </summary>
    /// <param name="linkType">Entity link type (e.g. "page", "task", "contact").</param>
    /// <param name="itemId">The entity ID.</param>
    /// <param name="title">Display title for the item.</param>
    /// <param name="details">Optional entity-specific detail fields to display.</param>
    void SetActiveItem(string linkType, string itemId, string title,
        IReadOnlyList<InfoPanelDetailField>? details = null);

    /// <summary>
    /// Clears the active item (e.g. when navigating away or deselecting).
    /// </summary>
    void ClearActiveItem();

    /// <summary>
    /// Notifies that the current item's content has changed (e.g. after save),
    /// so backlinks and graph data should be refreshed.
    /// </summary>
    void NotifyContentChanged();

    /// <summary>
    /// Returns all items that directly link TO the specified entity.
    /// Plugins can use this to discover reverse links (e.g. tasks that link to a contact).
    /// </summary>
    /// <param name="linkType">Entity link type of the target (e.g. "contact").</param>
    /// <param name="itemId">The entity ID of the target.</param>
    Task<IReadOnlyList<BacklinkInfo>> GetBacklinksAsync(string linkType, string itemId);
}
