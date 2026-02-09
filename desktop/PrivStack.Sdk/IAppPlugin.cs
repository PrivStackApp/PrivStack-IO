namespace PrivStack.Sdk;

/// <summary>
/// Core plugin interface. All PrivStack plugins must implement this contract.
/// </summary>
/// <remarks>
/// Plugin Lifecycle:
/// 1. Discovery     - Plugin class found via assembly scanning
/// 2. Instantiation - Parameterless constructor called
/// 3. Initialize    - InitializeAsync() called with IPluginHost
/// 4. Activate      - Activate() called, commands registered
/// 5. Navigation    - OnNavigatedTo/OnNavigatedFrom as user switches tabs
/// 6. Deactivate    - Deactivate() called when disabled
/// 7. Dispose       - Dispose() called on app shutdown
/// </remarks>
public interface IAppPlugin : IDisposable
{
    PluginMetadata Metadata { get; }
    NavigationItem? NavigationItem { get; }
    ICommandProvider? CommandProvider { get; }
    PluginState State { get; }

    /// <summary>
    /// Entity schemas this plugin registers with the generic entity engine.
    /// Implement to declare your entity types.
    /// </summary>
    IReadOnlyList<EntitySchema> EntitySchemas { get; }

    /// <summary>
    /// Initializes the plugin. Store the host for later use.
    /// </summary>
    Task<bool> InitializeAsync(IPluginHost host, CancellationToken cancellationToken = default);

    void Activate();
    void Deactivate();

    /// <summary>
    /// Creates or returns the cached ViewModel for this plugin's main view.
    /// </summary>
    ViewModelBase CreateViewModel();

    /// <summary>
    /// Clears the cached ViewModel (used during workspace switching).
    /// </summary>
    void ResetViewModel();

    Task OnNavigatedToAsync(CancellationToken cancellationToken = default);
    void OnNavigatedFrom();
}
