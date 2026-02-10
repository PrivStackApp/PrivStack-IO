namespace PrivStack.Sdk.Capabilities;

/// <summary>
/// Capability interface for plugins that support deep-link navigation to a specific item.
/// The shell uses <see cref="LinkType"/> to route incoming link requests to the correct plugin.
/// </summary>
public interface IDeepLinkTarget
{
    /// <summary>
    /// The link type this plugin handles (e.g., "task", "note", "contact", "event", "journal").
    /// </summary>
    string LinkType { get; }

    /// <summary>
    /// Navigates to the specified item within this plugin's view.
    /// Called by the shell after switching to this plugin's tab.
    /// </summary>
    Task NavigateToItemAsync(string itemId);
}
