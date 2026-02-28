namespace PrivStack.Services.Abstractions;

/// <summary>
/// Abstraction over the main window's navigation capabilities.
/// Implemented by MainWindowViewModel in Desktop, allowing Services
/// to navigate without referencing Avalonia types.
/// </summary>
public interface INavigationHost
{
    string? SelectedTab { get; }
    Task NavigateToLinkedItemAsync(string linkType, string itemId);
    void SelectTab(string navItemId);
}
