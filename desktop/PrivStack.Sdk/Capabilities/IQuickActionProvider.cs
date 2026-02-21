namespace PrivStack.Sdk.Capabilities;

/// <summary>
/// Capability interface for plugins that provide global quick actions.
/// Quick actions appear in the command palette and can be invoked from any plugin tab.
/// Actions can either execute immediately or show a modal overlay with custom UI.
/// </summary>
public interface IQuickActionProvider
{
    /// <summary>
    /// Returns all quick actions this plugin provides.
    /// Called at activation and cached by the shell.
    /// </summary>
    IReadOnlyList<QuickActionDescriptor> GetQuickActions();

    /// <summary>
    /// Creates the UI content for a quick action that has <see cref="QuickActionDescriptor.HasUI"/> = true.
    /// The returned object is set as the Content of a modal overlay ContentPresenter.
    /// Return null if the action has no UI.
    /// </summary>
    object? CreateQuickActionContent(string actionId);

    /// <summary>
    /// Executes a quick action that has <see cref="QuickActionDescriptor.HasUI"/> = false.
    /// For UI-based actions, the form handles its own save logic.
    /// </summary>
    Task ExecuteQuickActionAsync(string actionId, CancellationToken ct = default);
}
