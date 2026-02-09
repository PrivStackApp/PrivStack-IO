namespace PrivStack.Sdk.Capabilities;

/// <summary>
/// Capability interface for plugins that provide items which can be linked to
/// from other plugins (cross-plugin item references).
/// </summary>
public interface ILinkableItemProvider
{
    string LinkType { get; }
    string LinkTypeDisplayName { get; }
    string LinkTypeIcon { get; }

    Task<IReadOnlyList<LinkableItem>> SearchItemsAsync(
        string query,
        int maxResults = 20,
        CancellationToken cancellationToken = default);

    Task<LinkableItem?> GetItemByIdAsync(
        string itemId,
        CancellationToken cancellationToken = default);
}
