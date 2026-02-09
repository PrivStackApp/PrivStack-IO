using PrivStack.Sdk;

namespace PrivStack.Desktop.Services;

/// <summary>
/// Manages state for the shell's universal info panel.
/// Plugins call SetActiveItem/ClearActiveItem; the InfoPanelViewModel
/// subscribes to ActiveItemChanged to load backlinks and local graph.
/// </summary>
public sealed class InfoPanelService : IInfoPanelService
{
    private readonly BacklinkService _backlinkService;

    public InfoPanelService(BacklinkService backlinkService)
    {
        _backlinkService = backlinkService;
    }

    public string? ActiveLinkType { get; private set; }
    public string? ActiveItemId { get; private set; }
    public string? ActiveItemTitle { get; private set; }
    public IReadOnlyList<InfoPanelDetailField>? ActiveItemDetails { get; private set; }
    public string? ActivePluginId { get; private set; }

    /// <summary>
    /// Raised whenever the active item changes (or is cleared).
    /// </summary>
    public event Action? ActiveItemChanged;

    /// <summary>
    /// Raised when the active plugin changes (tab switch).
    /// </summary>
    public event Action? ActivePluginChanged;

    /// <summary>
    /// Raised when a plugin signals its content was saved/modified,
    /// so cached backlink/graph data should be invalidated and reloaded.
    /// </summary>
    public event Action? ContentChanged;

    public void SetActiveItem(string linkType, string itemId, string title,
        IReadOnlyList<InfoPanelDetailField>? details = null)
    {
        if (ActiveLinkType == linkType && ActiveItemId == itemId)
            return;

        ActiveLinkType = linkType;
        ActiveItemId = itemId;
        ActiveItemTitle = title;
        ActiveItemDetails = details;
        ActiveItemChanged?.Invoke();
    }

    public void ClearActiveItem()
    {
        if (ActiveLinkType == null && ActiveItemId == null)
            return;

        ActiveLinkType = null;
        ActiveItemId = null;
        ActiveItemTitle = null;
        ActiveItemDetails = null;
        ActiveItemChanged?.Invoke();
    }

    public void SetActivePlugin(string pluginId)
    {
        if (ActivePluginId == pluginId) return;
        ActivePluginId = pluginId;
        ActivePluginChanged?.Invoke();
    }

    public void NotifyContentChanged()
    {
        ContentChanged?.Invoke();
    }

    public async Task<IReadOnlyList<BacklinkInfo>> GetBacklinksAsync(string linkType, string itemId)
    {
        var entries = await _backlinkService.GetBacklinksAsync(linkType, itemId);
        return entries.Select(e => new BacklinkInfo(e.SourceId, e.SourceLinkType, e.SourceTitle)).ToList();
    }
}
