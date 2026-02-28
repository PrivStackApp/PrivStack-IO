using System.Collections.ObjectModel;
using System.Reflection;
using System.Runtime.Loader;
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
/// Scans multiple directories for plugin assemblies, loads them in isolated AssemblyLoadContexts,
/// and initializes IAppPlugin implementations for SDK capabilities and API providers.
/// </summary>
internal sealed class HeadlessPluginRegistry : IPluginRegistry
{
    private static readonly ILogger _log = Serilog.Log.ForContext<HeadlessPluginRegistry>();

    private readonly List<IAppPlugin> _plugins = [];
    private readonly List<IAppPlugin> _activePlugins = [];
    private readonly List<NavigationItem> _navItems = [];
    private readonly ObservableCollection<NavigationItem> _navItemsObservable = [];
    private PluginHostFactory? _hostFactory;
    private object? _mainViewModel;

    private PluginHostFactory HostFactory => _hostFactory ??= new PluginHostFactory();

    public IReadOnlyList<IAppPlugin> Plugins => _plugins;
    public IReadOnlyList<IAppPlugin> ActivePlugins => _activePlugins;
    public IReadOnlyList<NavigationItem> NavigationItems => _navItems;
    public ObservableCollection<NavigationItem> NavigationItemsObservable => _navItemsObservable;

#pragma warning disable CS0067 // Required by IPluginRegistry interface; headless never fires UI state events
    public event EventHandler<PluginStateChangedEventArgs>? PluginStateChanged;
#pragma warning restore CS0067
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
        var brokerProviders = HostFactory.CapabilityBroker.GetProviders<TCapability>();
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

        // Collect plugin assemblies from all known directories
        var pluginTypes = new List<(Type Type, Assembly Assembly)>();
        var pluginDirs = GetPluginDirectories();

        foreach (var pluginDir in pluginDirs)
        {
            _log.Debug("Scanning plugin directory: {Dir}", pluginDir);

            // Scan subdirectories — each plugin lives in its own folder
            foreach (var subDir in Directory.GetDirectories(pluginDir))
            {
                // Prioritize .Headless.dll assemblies (guaranteed Avalonia-free)
                // then fall back to full plugin DLLs with partial type loading
                var dlls = Directory.GetFiles(subDir, "PrivStack.Plugin.*.dll");
                if (dlls.Length == 0) continue;

                // Sort: *.Headless.dll first, then others — headless assemblies load cleanly
                // and the deduplication by plugin ID prevents the UI assembly from double-loading
                var sorted = dlls
                    .OrderByDescending(d => d.EndsWith(".Headless.dll", StringComparison.OrdinalIgnoreCase))
                    .ToArray();

                foreach (var dll in sorted)
                {
                    try
                    {
                        var context = new PluginLoadContext(dll, AppContext.BaseDirectory);
                        var asm = context.LoadFromAssemblyPath(dll);
                        ScanAssemblyForPlugins(asm, pluginTypes);
                    }
                    catch (Exception ex)
                    {
                        _log.Warning(ex, "Failed to load plugin assembly: {Path}", dll);
                    }
                }
            }

            // Also check for loose DLLs directly in the plugin directory
            var looseDlls = Directory.GetFiles(pluginDir, "PrivStack.Plugin.*.dll")
                .OrderByDescending(d => d.EndsWith(".Headless.dll", StringComparison.OrdinalIgnoreCase));

            foreach (var dll in looseDlls)
            {
                try
                {
                    var context = new PluginLoadContext(dll, AppContext.BaseDirectory);
                    var asm = context.LoadFromAssemblyPath(dll);
                    ScanAssemblyForPlugins(asm, pluginTypes);
                }
                catch (Exception ex)
                {
                    _log.Warning(ex, "Failed to load plugin assembly: {Path}", dll);
                }
            }
        }

        _log.Information("Found {Count} plugin types across {DirCount} directories",
            pluginTypes.Count, pluginDirs.Count);

        // Initialize discovered plugins
        foreach (var (type, _) in pluginTypes)
        {
            try
            {
                if (Activator.CreateInstance(type) is not IAppPlugin plugin) continue;

                var pluginId = plugin.Metadata.Id;

                // Skip duplicates (same plugin found in multiple directories)
                if (_plugins.Any(p => p.Metadata.Id.Equals(pluginId, StringComparison.OrdinalIgnoreCase)))
                {
                    _log.Debug("Plugin {Id} already loaded, skipping duplicate", pluginId);
                    continue;
                }

                // Check if disabled
                if (wsConfig.DisabledPlugins.Contains(pluginId))
                {
                    _log.Debug("Plugin {Id} is disabled, skipping", pluginId);
                    continue;
                }

                // Initialize
                var host = HostFactory.CreateHost(pluginId);
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

    /// <summary>
    /// Returns all directories to scan for plugins, in priority order.
    /// </summary>
    private static List<string> GetPluginDirectories()
    {
        var dirs = new List<string>();

        // 1. Bundled plugins — next to the server binary
        var bundledDir = Path.Combine(AppContext.BaseDirectory, "plugins");
        if (Directory.Exists(bundledDir))
        {
            dirs.Add(bundledDir);
        }
        else
        {
            // Dev-time fallback: repo root is 5 levels up from bin/{Config}/net9.0/
            var devDir = Path.GetFullPath(Path.Combine(AppContext.BaseDirectory, "..", "..", "..", "..", "..", "plugins"));
            if (Directory.Exists(devDir))
                dirs.Add(devDir);
        }

        // 2. User-installed plugins (~/.privstack/plugins/)
        var userDir = Path.Combine(DataPaths.BaseDir, "plugins");
        if (Directory.Exists(userDir) && !dirs.Contains(userDir))
            dirs.Add(userDir);

        return dirs;
    }

    /// <summary>
    /// Scans an assembly for IAppPlugin implementations and adds them to the list.
    /// Uses partial type loading to handle plugins with Avalonia view types that
    /// can't be resolved in the headless context.
    /// </summary>
    private static void ScanAssemblyForPlugins(Assembly asm, List<(Type, Assembly)> pluginTypes)
    {
        Type?[] loadedTypes;
        try
        {
            loadedTypes = asm.GetExportedTypes();
        }
        catch (ReflectionTypeLoadException ex)
        {
            // Some types failed to load (e.g., Avalonia views) — use the ones that did
            loadedTypes = ex.Types;
            _log.Debug("Partial type load for {Name}: {Count} types loaded, some skipped (Avalonia views)",
                asm.GetName().Name, loadedTypes.Count(t => t != null));
        }
        catch (Exception ex)
        {
            _log.Warning("Failed to scan assembly {Name}: {Message}", asm.GetName().Name, ex.Message);
            return;
        }

        foreach (var type in loadedTypes)
        {
            if (type is not { IsClass: true, IsAbstract: false }) continue;

            try
            {
                if (typeof(IAppPlugin).IsAssignableFrom(type))
                    pluginTypes.Add((type, asm));
            }
            catch
            {
                // Type comparison can fail for types with unresolvable base types
            }
        }
    }

    /// <summary>
    /// Isolated AssemblyLoadContext for plugins. Delegates host-provided assemblies
    /// (SDK, Services, etc.) to the default context for type identity, and resolves
    /// plugin-specific dependencies from the plugin's directory.
    /// </summary>
    private sealed class PluginLoadContext : AssemblyLoadContext
    {
        private readonly AssemblyDependencyResolver _resolver;
        private readonly AssemblyDependencyResolver _hostResolver;

        public PluginLoadContext(string pluginPath, string hostPath) : base(isCollectible: false)
        {
            _resolver = new AssemblyDependencyResolver(pluginPath);
            _hostResolver = new AssemblyDependencyResolver(hostPath);
        }

        protected override Assembly? Load(AssemblyName assemblyName)
        {
            // If the host already has this assembly loaded, use it (type identity)
            foreach (var loaded in Default.Assemblies)
            {
                if (string.Equals(loaded.GetName().Name, assemblyName.Name, StringComparison.OrdinalIgnoreCase))
                    return null;
            }

            // If the host can provide this assembly, delegate to default context
            if (_hostResolver.ResolveAssemblyToPath(assemblyName) != null)
                return null;

            // Plugin-specific dependency: resolve from plugin directory
            var path = _resolver.ResolveAssemblyToPath(assemblyName);
            return path != null ? LoadFromAssemblyPath(path) : null;
        }

        protected override IntPtr LoadUnmanagedDll(string unmanagedDllName)
        {
            var path = _resolver.ResolveUnmanagedDllToPath(unmanagedDllName);
            return path != null ? LoadUnmanagedDllFromPath(path) : IntPtr.Zero;
        }
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
