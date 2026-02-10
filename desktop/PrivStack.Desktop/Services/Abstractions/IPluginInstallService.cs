using PrivStack.Desktop.Models.PluginRegistry;

namespace PrivStack.Desktop.Services.Abstractions;

/// <summary>
/// Service for downloading, installing, updating, and uninstalling official plugins
/// from the PrivStack plugin registry.
/// </summary>
public interface IPluginInstallService
{
    /// <summary>
    /// Fetches the list of available official plugins from the server.
    /// </summary>
    Task<IReadOnlyList<OfficialPluginInfo>> GetAvailablePluginsAsync(CancellationToken ct = default);

    /// <summary>
    /// Downloads and installs a plugin package, then hot-loads it into the registry.
    /// </summary>
    Task<PluginInstallResult> InstallPluginAsync(
        OfficialPluginInfo plugin,
        IProgress<double>? progress = null,
        CancellationToken ct = default);

    /// <summary>
    /// Updates an installed plugin to a new version (backup → install → rollback on failure).
    /// </summary>
    Task<PluginInstallResult> UpdatePluginAsync(
        string pluginId,
        OfficialPluginInfo newVersion,
        IProgress<double>? progress = null,
        CancellationToken ct = default);

    /// <summary>
    /// Unloads and removes a plugin from disk.
    /// </summary>
    Task<bool> UninstallPluginAsync(string pluginId);

    /// <summary>
    /// Returns a map of installed plugin IDs to their versions (read from manifest.json).
    /// </summary>
    IReadOnlyDictionary<string, Version> GetInstalledVersions();

    /// <summary>
    /// Quick connectivity check against the plugin registry API.
    /// </summary>
    Task<bool> IsOnlineAsync(CancellationToken ct = default);
}
