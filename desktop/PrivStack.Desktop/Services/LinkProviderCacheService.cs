// ============================================================================
// File: LinkProviderCacheService.cs
// Description: Caches link provider metadata from plugins to avoid repeated
//              FFI calls. Only refreshes when plugins are loaded/unloaded.
// ============================================================================

using System.Runtime.InteropServices;
using System.Text.Json;
using Microsoft.Extensions.DependencyInjection;
using PrivStack.Desktop.Services.Plugin;
using PrivStack.Sdk.Capabilities;
using Serilog;
using NativeLib = PrivStack.Desktop.Native.NativeLibrary;

namespace PrivStack.Desktop.Services;

/// <summary>
/// Cached link provider info for fast lookups.
/// </summary>
public sealed record LinkProviderInfo(
    string PluginId,
    string LinkType,
    string DisplayName,
    string? Icon);

/// <summary>
/// Caches link provider metadata from plugins to avoid repeated FFI calls.
/// Also includes native C# plugins that implement ILinkableItemProvider.
/// The cache is loaded once at startup and only invalidated when plugins change.
/// </summary>
public sealed class LinkProviderCacheService
{
    private static readonly ILogger _log = Log.ForContext<LinkProviderCacheService>();

    private readonly object _lock = new();
    private Dictionary<string, LinkProviderInfo> _byLinkType = new();
    private Dictionary<string, LinkProviderInfo> _byPluginId = new();
    private bool _isLoaded;

    /// <summary>
    /// Loads link providers from the plugin host. Called once at startup.
    /// </summary>
    public void Load()
    {
        lock (_lock)
        {
            LoadInternal();
        }
    }

    /// <summary>
    /// Invalidates the cache and reloads from the plugin host.
    /// Call this when plugins are loaded or unloaded.
    /// </summary>
    public void Invalidate()
    {
        lock (_lock)
        {
            _log.Debug("LinkProviderCache: Invalidating cache, will reload on next access");
            _isLoaded = false;
            _byLinkType.Clear();
            _byPluginId.Clear();
        }
    }

    /// <summary>
    /// Gets the plugin ID for a given link type, or null if not found.
    /// </summary>
    public string? GetPluginIdForLinkType(string linkType)
    {
        EnsureLoaded();
        lock (_lock)
        {
            return _byLinkType.TryGetValue(linkType.ToLowerInvariant(), out var info)
                ? info.PluginId
                : null;
        }
    }

    /// <summary>
    /// Gets the display name for a given link type, or the link type itself if not found.
    /// </summary>
    public string GetDisplayNameForLinkType(string linkType)
    {
        EnsureLoaded();
        lock (_lock)
        {
            return _byLinkType.TryGetValue(linkType.ToLowerInvariant(), out var info)
                ? info.DisplayName
                : linkType;
        }
    }

    /// <summary>
    /// Gets the link type for a given plugin ID, or null if not found.
    /// </summary>
    public string? GetLinkTypeForPluginId(string pluginId)
    {
        EnsureLoaded();
        lock (_lock)
        {
            return _byPluginId.TryGetValue(pluginId, out var info)
                ? info.LinkType
                : null;
        }
    }

    /// <summary>
    /// Gets all display names as a dictionary (link_type -> display_name).
    /// </summary>
    public IReadOnlyDictionary<string, string> GetAllDisplayNames()
    {
        EnsureLoaded();
        lock (_lock)
        {
            return _byLinkType.ToDictionary(
                kvp => kvp.Key,
                kvp => kvp.Value.DisplayName);
        }
    }

    /// <summary>
    /// Gets all cached providers.
    /// </summary>
    public IReadOnlyList<LinkProviderInfo> GetAll()
    {
        EnsureLoaded();
        lock (_lock)
        {
            return _byLinkType.Values.ToList();
        }
    }

    private void EnsureLoaded()
    {
        lock (_lock)
        {
            if (!_isLoaded)
            {
                LoadInternal();
            }
        }
    }

    private void LoadInternal()
    {
        _log.Information("LinkProviderCache: LoadInternal() starting...");
        _byLinkType.Clear();
        _byPluginId.Clear();

        // Load from FFI (WASM plugins)
        LoadFromFfi();

        // Load from native C# plugins
        LoadFromNativePlugins();

        _log.Information("LinkProviderCache: LoadInternal() complete - {Count} providers total: [{Types}]",
            _byLinkType.Count, string.Join(", ", _byLinkType.Keys));
        _isLoaded = true;
    }

    private void LoadFromFfi()
    {
        var providersPtr = NativeLib.PluginGetLinkProviders();
        if (providersPtr == nint.Zero)
        {
            _log.Debug("LinkProviderCache: PluginGetLinkProviders returned null (no WASM providers)");
            return;
        }

        try
        {
            var json = Marshal.PtrToStringUTF8(providersPtr);
            NativeLib.FreeString(providersPtr);

            if (string.IsNullOrEmpty(json))
            {
                _log.Debug("LinkProviderCache: Empty JSON from PluginGetLinkProviders");
                return;
            }

            using var doc = JsonDocument.Parse(json);
            foreach (var p in doc.RootElement.EnumerateArray())
            {
                var pluginId = p.GetProperty("plugin_id").GetString() ?? "";
                var linkType = p.GetProperty("link_type").GetString() ?? "";
                var displayName = p.GetProperty("display_name").GetString() ?? linkType;
                var icon = p.TryGetProperty("icon", out var iconProp) ? iconProp.GetString() : null;

                var info = new LinkProviderInfo(pluginId, linkType, displayName, icon);
                _byLinkType[linkType.ToLowerInvariant()] = info;
                _byPluginId[pluginId] = info;
            }

            _log.Debug("LinkProviderCache: Loaded {Count} WASM providers", _byLinkType.Count);
        }
        catch (Exception ex)
        {
            _log.Error(ex, "LinkProviderCache: Failed to parse link providers JSON from FFI");
        }
    }

    private void LoadFromNativePlugins()
    {
        _log.Information("LinkProviderCache: Loading native plugins...");

        // Get plugin registry at runtime to avoid DI timing issues
        var pluginRegistry = App.Services.GetService<IPluginRegistry>();
        if (pluginRegistry == null)
        {
            _log.Warning("LinkProviderCache: IPluginRegistry not available from DI, skipping native plugins");
            return;
        }

        try
        {
            var nativeProviders = pluginRegistry.GetCapabilityProviders<ILinkableItemProvider>();
            _log.Information("LinkProviderCache: Found {Count} ILinkableItemProvider implementations", nativeProviders.Count);

            var countBefore = _byLinkType.Count;

            foreach (var provider in nativeProviders)
            {
                var linkType = provider.LinkType;
                _log.Debug("LinkProviderCache: Processing provider with LinkType={LinkType}", linkType);

                if (string.IsNullOrEmpty(linkType))
                {
                    _log.Warning("LinkProviderCache: Skipping provider with empty LinkType");
                    continue;
                }

                // Skip if already loaded from FFI
                if (_byLinkType.ContainsKey(linkType.ToLowerInvariant()))
                {
                    _log.Debug("LinkProviderCache: Skipping {LinkType} (already loaded from FFI)", linkType);
                    continue;
                }

                // Find the plugin ID for this provider
                var plugin = pluginRegistry.ActivePlugins.FirstOrDefault(p => p is ILinkableItemProvider lp && lp.LinkType == linkType);
                var pluginId = plugin?.Metadata.Id ?? linkType;

                var info = new LinkProviderInfo(
                    pluginId,
                    linkType,
                    provider.LinkTypeDisplayName,
                    provider.LinkTypeIcon);

                _byLinkType[linkType.ToLowerInvariant()] = info;
                _byPluginId[pluginId] = info;

                _log.Information("LinkProviderCache: Added native provider: {LinkType} ({DisplayName}) with icon {Icon}",
                    linkType, provider.LinkTypeDisplayName, provider.LinkTypeIcon);
            }

            var nativeCount = _byLinkType.Count - countBefore;
            _log.Information("LinkProviderCache: Loaded {Count} native C# providers total", nativeCount);
        }
        catch (Exception ex)
        {
            _log.Error(ex, "LinkProviderCache: Failed to load native plugin providers");
        }
    }
}
