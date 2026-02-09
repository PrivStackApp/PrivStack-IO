namespace PrivStack.Sdk;

/// <summary>
/// Service for cross-plugin navigation within the host application.
/// </summary>
public interface INavigationService
{
    /// <summary>
    /// Navigates to the specified plugin's tab.
    /// </summary>
    /// <param name="pluginId">The plugin ID or navigation item ID to navigate to.</param>
    void NavigateTo(string pluginId);

    /// <summary>
    /// Navigates back to the previously selected tab.
    /// </summary>
    void NavigateBack();

    /// <summary>
    /// Navigates to a specific item within its source plugin (deep-link navigation).
    /// Switches to the target plugin's tab and selects the item.
    /// </summary>
    /// <param name="linkType">The link type identifying the target plugin (e.g., "task", "event", "page").</param>
    /// <param name="itemId">The unique identifier of the item to navigate to.</param>
    Task NavigateToItemAsync(string linkType, string itemId);
}
