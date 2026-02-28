namespace PrivStack.Sdk;

/// <summary>
/// Base class for headless plugin registrations. Implements <see cref="IAppPlugin"/> with
/// no-op defaults for UI methods (CreateViewModel, navigation, etc.) so headless-loadable
/// assemblies don't need Avalonia or CommunityToolkit view model infrastructure.
/// <para>
/// Used by <c>PrivStack.Plugin.*.Headless</c> assemblies that provide services, capabilities,
/// and API routes for the headless server without dragging in Avalonia dependencies.
/// </para>
/// </summary>
public abstract class HeadlessPluginBase : IAppPlugin
{
    private PluginState _state = PluginState.Discovered;
    private bool _disposed;

    /// <summary>
    /// The host services provided during initialization. Guaranteed non-null after
    /// <see cref="InitializeAsync"/> completes successfully.
    /// </summary>
    protected IPluginHost? Host { get; private set; }

    // ── Required overrides ──────────────────────────────────────────────

    public abstract PluginMetadata Metadata { get; }

    // ── Optional overrides ──────────────────────────────────────────────

    public virtual NavigationItem? NavigationItem => null;
    public virtual ICommandProvider? CommandProvider => null;
    public virtual IReadOnlyList<EntitySchema> EntitySchemas => [];

    /// <summary>
    /// Called after <see cref="Host"/> is set. Perform service creation and capability registration here.
    /// </summary>
    protected virtual Task<bool> OnInitializeAsync(CancellationToken cancellationToken)
        => Task.FromResult(true);

    /// <summary>Called during <see cref="Dispose"/>. Release resources here.</summary>
    protected virtual void OnDispose() { }

    // ── IAppPlugin implementation ───────────────────────────────────────

    public PluginState State
    {
        get => _state;
        private set => _state = value;
    }

    public async Task<bool> InitializeAsync(IPluginHost host, CancellationToken cancellationToken = default)
    {
        Host = host ?? throw new ArgumentNullException(nameof(host));
        State = PluginState.Initializing;
        try
        {
            var result = await OnInitializeAsync(cancellationToken);
            State = result ? PluginState.Initialized : PluginState.Failed;
            return result;
        }
        catch
        {
            State = PluginState.Failed;
            throw;
        }
    }

    public void Activate() => State = PluginState.Active;

    public void Deactivate()
    {
        State = PluginState.Deactivated;
        Host?.Capabilities.UnregisterAll(this);
    }

    /// <summary>
    /// Headless plugins do not create ViewModels. This throws <see cref="NotSupportedException"/>
    /// if called — headless registries should never call this method.
    /// </summary>
    public ViewModelBase CreateViewModel()
        => throw new NotSupportedException("Headless plugins do not create ViewModels.");

    public void ResetViewModel() { }
    public Task OnNavigatedToAsync(CancellationToken cancellationToken = default) => Task.CompletedTask;
    public void OnNavigatedFrom() { }

    public void Dispose()
    {
        if (_disposed) return;
        _disposed = true;
        Host?.Capabilities.UnregisterAll(this);
        OnDispose();
        State = PluginState.Disposed;
        GC.SuppressFinalize(this);
    }
}
