using System.Collections.ObjectModel;
using System.Reflection;
using Microsoft.Extensions.DependencyInjection;
using PrivStack.Services;
using PrivStack.Services.Abstractions;
using PrivStack.Services.Plugin;
using PrivStack.Services.Sdk;
using PrivStack.Sdk;
using Serilog;

namespace PrivStack.Server;

/// <summary>
/// Headless plugin registry — discovers and initializes plugins without Avalonia dependencies.
/// Subset of the Desktop PluginRegistry focused on SDK capabilities and API providers.
/// </summary>
internal sealed class HeadlessPluginRegistry : IPluginRegistry
{
    private static readonly ILogger _log = Serilog.Log.ForContext<HeadlessPluginRegistry>();

    private readonly List<IAppPlugin> _plugins = [];
    private readonly List<IAppPlugin> _activePlugins = [];
    private readonly List<NavigationItem> _navItems = [];
    private readonly ObservableCollection<NavigationItem> _navItemsObservable = [];
    private readonly PluginHostFactory _hostFactory;
    private object? _mainViewModel;

    public HeadlessPluginRegistry()
    {
        _hostFactory = new PluginHostFactory();
    }

    public IReadOnlyList<IAppPlugin> Plugins => _plugins;
    public IReadOnlyList<IAppPlugin> ActivePlugins => _activePlugins;
    public IReadOnlyList<NavigationItem> NavigationItems => _navItems;
    public ObservableCollection<NavigationItem> NavigationItemsObservable => _navItemsObservable;

    public event EventHandler<PluginStateChangedEventArgs>? PluginStateChanged;
    public event EventHandler? NavigationItemsChanged;

    public IAppPlugin? GetPlugin(string pluginId)
        => _plugins.FirstOrDefault(p => p.Metadata.Id.Equals(pluginId, StringComparison.OrdinalIgnoreCase));

    public IAppPlugin? GetPluginForNavItem(string navItemId)
        => _activePlugins.FirstOrDefault(p => p.NavigationItem?.Id == navItemId);

    public IReadOnlyList<TCapability> GetCapabilityProviders<TCapability>() where TCapability : class
    {
        var providers = new List<TCapability>();

        foreach (var plugin in _activePlugins)
        {
            if (plugin is TCapability cap)
                providers.Add(cap);
        }

        // Also check capability broker
        var brokerProviders = _hostFactory.CapabilityBroker.GetProviders<TCapability>();
        if (brokerProviders != null)
            providers.AddRange(brokerProviders);

        return providers;
    }

    public TCapability? GetCapabilityProvider<TCapability>(
        string identifier,
        Func<TCapability, string> identifierSelector) where TCapability : class
    {
        return GetCapabilityProviders<TCapability>()
            .FirstOrDefault(p => identifierSelector(p) == identifier);
    }

    public void DiscoverAndInitialize()
    {
        DiscoverAndInitializeAsync().GetAwaiter().GetResult();
    }

    public async Task DiscoverAndInitializeAsync(CancellationToken ct = default)
    {
        _log.Information("Discovering plugins for headless mode");

        var appSettings = ServiceProviderAccessor.Services.GetRequiredService<IAppSettingsService>();
        var wsConfig = appSettings.GetWorkspacePluginConfig();

        // Scan plugin assemblies from plugins directory
        var pluginDir = Path.Combine(AppContext.BaseDirectory, "plugins");
        var assemblies = new List<Assembly>();

        // Add current assembly's referenced assemblies (built-in plugins)
        var entryAssembly = Assembly.GetEntryAssembly();
        if (entryAssembly != null)
        {
            foreach (var refName in entryAssembly.GetReferencedAssemblies())
            {
                try
                {
                    var asm = Assembly.Load(refName);
                    assemblies.Add(asm);
                }
                catch { /* skip unloadable */ }
            }
        }

        // Also scan loose plugin DLLs
        if (Directory.Exists(pluginDir))
        {
            foreach (var dll in Directory.GetFiles(pluginDir, "PrivStack.Plugin.*.dll"))
            {
                try
                {
                    var asm = Assembly.LoadFrom(dll);
                    assemblies.Add(asm);
                }
                catch (Exception ex)
                {
                    _log.Warning(ex, "Failed to load plugin assembly: {Path}", dll);
                }
            }
        }

        // Find IAppPlugin implementations
        foreach (var asm in assemblies)
        {
            foreach (var type in asm.GetExportedTypes().Where(t =>
                t is { IsClass: true, IsAbstract: false } && typeof(IAppPlugin).IsAssignableFrom(t)))
            {
                try
                {
                    if (Activator.CreateInstance(type) is not IAppPlugin plugin) continue;

                    var pluginId = plugin.Metadata.Id;

                    // Check if disabled
                    if (wsConfig.DisabledPlugins.Contains(pluginId))
                    {
                        _log.Debug("Plugin {Id} is disabled, skipping", pluginId);
                        continue;
                    }

                    // Initialize
                    var host = _hostFactory.CreateHost(pluginId);
                    await plugin.InitializeAsync(host);

                    _plugins.Add(plugin);
                    _activePlugins.Add(plugin);

                    _log.Information("Loaded plugin: {Name} ({Id})", plugin.Metadata.Name, pluginId);
                }
                catch (Exception ex)
                {
                    _log.Error(ex, "Failed to initialize plugin type: {Type}", type.FullName);
                }
            }
        }

        // Build nav items
        _navItems.Clear();
        foreach (var plugin in _activePlugins.OrderBy(p => p.Metadata.NavigationOrder))
        {
            if (plugin.NavigationItem is { } navItem)
            {
                _navItems.Add(navItem);
                _navItemsObservable.Add(navItem);
            }
        }

        _log.Information("Headless plugin discovery complete: {Count} plugins active", _activePlugins.Count);
        NavigationItemsChanged?.Invoke(this, EventArgs.Empty);
    }

    public void Reinitialize()
    {
        _plugins.Clear();
        _activePlugins.Clear();
        _navItems.Clear();
        _navItemsObservable.Clear();
        DiscoverAndInitialize();
    }

    public Task ReinitializeAsync()
    {
        Reinitialize();
        return Task.CompletedTask;
    }

    public void SetMainViewModel(object? mainViewModel) => _mainViewModel = mainViewModel;
    public object? GetMainViewModel() => _mainViewModel;
    public void UpdateSelectedNavItem(string navItemId) { }
    public void MoveNavigationItem(int fromIndex, int toIndex) { }

    public bool IsPluginEnabled(string pluginId)
    {
        var config = ServiceProviderAccessor.Services.GetRequiredService<IAppSettingsService>().GetWorkspacePluginConfig();
        return !config.DisabledPlugins.Contains(pluginId);
    }

    public bool EnablePlugin(string pluginId)
    {
        var config = ServiceProviderAccessor.Services.GetRequiredService<IAppSettingsService>().GetWorkspacePluginConfig();
        config.DisabledPlugins.Remove(pluginId);
        ServiceProviderAccessor.Services.GetRequiredService<IAppSettingsService>().Save();
        return true;
    }

    public bool DisablePlugin(string pluginId)
    {
        var config = ServiceProviderAccessor.Services.GetRequiredService<IAppSettingsService>().GetWorkspacePluginConfig();
        config.DisabledPlugins.Add(pluginId);
        ServiceProviderAccessor.Services.GetRequiredService<IAppSettingsService>().Save();
        return true;
    }

    public bool TogglePlugin(string pluginId) => IsPluginEnabled(pluginId) ? !DisablePlugin(pluginId) : EnablePlugin(pluginId);
    public void SetExperimentalPluginsEnabled(bool enabled) { }
    public Task<bool> LoadPluginFromDirectoryAsync(string pluginDirectory, CancellationToken ct = default) => Task.FromResult(false);
    public bool UnloadPlugin(string pluginId) => false;
}
