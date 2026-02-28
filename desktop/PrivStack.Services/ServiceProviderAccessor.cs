namespace PrivStack.Services;

/// <summary>
/// Static accessor for the DI container. Set by the Desktop app or headless server during startup.
/// Used as a service locator where constructor injection is impractical (e.g., cross-singleton
/// wiring that requires lazy resolution).
/// </summary>
public static class ServiceProviderAccessor
{
    private static IServiceProvider? _services;

    /// <summary>
    /// Gets the current service provider. Throws if not yet set.
    /// </summary>
    public static IServiceProvider Services
    {
        get => _services ?? throw new InvalidOperationException("ServiceProvider has not been configured. Call Set() during startup.");
        set => _services = value;
    }

    /// <summary>
    /// Gets the service provider, or null if not yet set.
    /// </summary>
    public static IServiceProvider? ServicesOrNull => _services;
}
