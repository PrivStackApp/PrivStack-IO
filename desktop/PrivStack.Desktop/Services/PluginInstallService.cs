using System.IO.Compression;
using System.Security.Cryptography;
using System.Text.Json;
using PrivStack.Desktop.Models.PluginRegistry;
using PrivStack.Desktop.Services.Abstractions;
using PrivStack.Desktop.Services.Plugin;
using Serilog;

namespace PrivStack.Desktop.Services;

/// <summary>
/// Downloads, verifies, extracts, and hot-loads official plugin packages (.pspkg).
/// Packages are stored in ~/.privstack/plugins/{pluginId}/.
/// </summary>
public sealed class PluginInstallService : IPluginInstallService
{
    private static readonly ILogger _log = Log.ForContext<PluginInstallService>();

    private static readonly HttpClient Http = new()
    {
        Timeout = TimeSpan.FromMinutes(5)
    };

    private static readonly JsonSerializerOptions ManifestJsonOptions = new()
    {
        PropertyNamingPolicy = JsonNamingPolicy.SnakeCaseLower,
        PropertyNameCaseInsensitive = true
    };

    private readonly PrivStackApiClient _apiClient;
    private readonly IPluginRegistry _pluginRegistry;

    private static string UserPluginsDir => Path.Combine(
        Environment.GetFolderPath(Environment.SpecialFolder.UserProfile),
        ".privstack", "plugins");

    public PluginInstallService(PrivStackApiClient apiClient, IPluginRegistry pluginRegistry)
    {
        _apiClient = apiClient;
        _pluginRegistry = pluginRegistry;
    }

    public async Task<IReadOnlyList<OfficialPluginInfo>> GetAvailablePluginsAsync(CancellationToken ct = default)
    {
        return await _apiClient.GetOfficialPluginsAsync(ct);
    }

    public async Task<PluginInstallResult> InstallPluginAsync(
        OfficialPluginInfo plugin,
        IProgress<double>? progress = null,
        CancellationToken ct = default)
    {
        try
        {
            _log.Information("Installing plugin {PluginId} v{Version}", plugin.PluginId, plugin.Version);
            progress?.Report(0);

            // 1. Download .pspkg to temp
            var tempPath = Path.Combine(Path.GetTempPath(), $"{plugin.PluginId}-{plugin.Version}.pspkg");
            await DownloadFileAsync(plugin.PackageUrl, tempPath, plugin.PackageSizeBytes, progress, ct);
            progress?.Report(0.6);

            // 2. Verify SHA-256
            var actualChecksum = await ComputeSha256Async(tempPath, ct);
            if (!string.Equals(actualChecksum, plugin.ChecksumSha256, StringComparison.OrdinalIgnoreCase))
            {
                _log.Error("Checksum mismatch for {PluginId}: expected {Expected}, got {Actual}",
                    plugin.PluginId, plugin.ChecksumSha256, actualChecksum);
                TryDeleteFile(tempPath);
                return new PluginInstallResult(false, "Package integrity check failed. The download may be corrupted.");
            }
            progress?.Report(0.7);

            // 3. Extract to user plugins directory
            var pluginDir = Path.Combine(UserPluginsDir, plugin.PluginId);
            Directory.CreateDirectory(pluginDir);

            // Clear any existing files
            foreach (var file in Directory.GetFiles(pluginDir))
                File.Delete(file);

            ZipFile.ExtractToDirectory(tempPath, pluginDir, overwriteFiles: true);
            TryDeleteFile(tempPath);
            progress?.Report(0.85);

            // 4. Verify manifest + entry DLL exist
            var manifestPath = Path.Combine(pluginDir, "manifest.json");
            if (!File.Exists(manifestPath))
            {
                _log.Error("Missing manifest.json in package for {PluginId}", plugin.PluginId);
                Directory.Delete(pluginDir, recursive: true);
                return new PluginInstallResult(false, "Invalid plugin package: missing manifest.json");
            }

            var manifest = JsonSerializer.Deserialize<PluginManifest>(
                await File.ReadAllTextAsync(manifestPath, ct), ManifestJsonOptions);

            if (manifest?.EntryDll is null || !File.Exists(Path.Combine(pluginDir, manifest.EntryDll)))
            {
                _log.Error("Missing entry DLL in package for {PluginId}", plugin.PluginId);
                Directory.Delete(pluginDir, recursive: true);
                return new PluginInstallResult(false, "Invalid plugin package: missing entry DLL");
            }
            progress?.Report(0.9);

            // 5. Hot-load into the running app
            var loaded = await _pluginRegistry.LoadPluginFromDirectoryAsync(pluginDir, ct);
            if (!loaded)
            {
                _log.Warning("Plugin extracted but hot-load failed for {PluginId} — will load on next restart", plugin.PluginId);
            }

            progress?.Report(1.0);
            _log.Information("Plugin {PluginId} v{Version} installed successfully at {Path}",
                plugin.PluginId, plugin.Version, pluginDir);

            return new PluginInstallResult(true, InstalledPath: pluginDir);
        }
        catch (OperationCanceledException)
        {
            return new PluginInstallResult(false, "Installation was cancelled.");
        }
        catch (Exception ex)
        {
            _log.Error(ex, "Failed to install plugin {PluginId}", plugin.PluginId);
            return new PluginInstallResult(false, ex.Message);
        }
    }

    public async Task<PluginInstallResult> UpdatePluginAsync(
        string pluginId,
        OfficialPluginInfo newVersion,
        IProgress<double>? progress = null,
        CancellationToken ct = default)
    {
        var pluginDir = Path.Combine(UserPluginsDir, pluginId);
        var backupDir = pluginDir + ".bak";

        try
        {
            _log.Information("Updating plugin {PluginId} to v{Version}", pluginId, newVersion.Version);

            // Unload the running plugin first
            _pluginRegistry.UnloadPlugin(pluginId);

            // Backup existing
            if (Directory.Exists(pluginDir))
            {
                if (Directory.Exists(backupDir))
                    Directory.Delete(backupDir, recursive: true);
                Directory.Move(pluginDir, backupDir);
            }

            // Install new version
            var result = await InstallPluginAsync(newVersion, progress, ct);

            if (result.Success)
            {
                // Clean up backup
                if (Directory.Exists(backupDir))
                    Directory.Delete(backupDir, recursive: true);
            }
            else
            {
                // Restore from backup
                if (Directory.Exists(backupDir))
                {
                    if (Directory.Exists(pluginDir))
                        Directory.Delete(pluginDir, recursive: true);
                    Directory.Move(backupDir, pluginDir);

                    // Re-load the old version
                    await _pluginRegistry.LoadPluginFromDirectoryAsync(pluginDir, ct);
                }
            }

            return result;
        }
        catch (Exception ex)
        {
            _log.Error(ex, "Failed to update plugin {PluginId}", pluginId);

            // Attempt restore
            if (Directory.Exists(backupDir))
            {
                try
                {
                    if (Directory.Exists(pluginDir))
                        Directory.Delete(pluginDir, recursive: true);
                    Directory.Move(backupDir, pluginDir);
                    await _pluginRegistry.LoadPluginFromDirectoryAsync(pluginDir, ct);
                }
                catch (Exception restoreEx)
                {
                    _log.Error(restoreEx, "Failed to restore backup for {PluginId}", pluginId);
                }
            }

            return new PluginInstallResult(false, ex.Message);
        }
    }

    public Task<bool> UninstallPluginAsync(string pluginId)
    {
        try
        {
            _log.Information("Uninstalling plugin {PluginId}", pluginId);

            // Unload from running app
            _pluginRegistry.UnloadPlugin(pluginId);

            // Delete from disk
            var pluginDir = Path.Combine(UserPluginsDir, pluginId);
            if (Directory.Exists(pluginDir))
            {
                Directory.Delete(pluginDir, recursive: true);
                _log.Information("Deleted plugin directory: {Path}", pluginDir);
            }

            return Task.FromResult(true);
        }
        catch (Exception ex)
        {
            _log.Error(ex, "Failed to uninstall plugin {PluginId}", pluginId);
            return Task.FromResult(false);
        }
    }

    public IReadOnlyDictionary<string, Version> GetInstalledVersions()
    {
        var versions = new Dictionary<string, Version>(StringComparer.OrdinalIgnoreCase);

        if (!Directory.Exists(UserPluginsDir))
            return versions;

        // Also check bundled plugins directory
        var bundledDir = Path.Combine(AppContext.BaseDirectory, "plugins");
        var dirs = Directory.Exists(bundledDir)
            ? Directory.GetDirectories(UserPluginsDir).Concat(Directory.GetDirectories(bundledDir))
            : Directory.GetDirectories(UserPluginsDir);

        foreach (var dir in dirs)
        {
            var manifestPath = Path.Combine(dir, "manifest.json");
            if (!File.Exists(manifestPath)) continue;

            try
            {
                var json = File.ReadAllText(manifestPath);
                var manifest = JsonSerializer.Deserialize<PluginManifest>(json, ManifestJsonOptions);
                if (manifest?.PluginId != null && manifest.Version != null &&
                    Version.TryParse(manifest.Version, out var ver))
                {
                    versions[manifest.PluginId] = ver;
                }
            }
            catch (Exception ex)
            {
                _log.Warning(ex, "Failed to read manifest from {Path}", manifestPath);
            }
        }

        return versions;
    }

    public async Task<bool> IsOnlineAsync(CancellationToken ct = default)
    {
        try
        {
            using var request = new HttpRequestMessage(HttpMethod.Get, $"{PrivStackApiClient.ApiBaseUrl}/api/health");
            using var response = await Http.SendAsync(request, HttpCompletionOption.ResponseHeadersRead, ct);
            return response.IsSuccessStatusCode;
        }
        catch
        {
            return false;
        }
    }

    // ── Private helpers ──────────────────────────────────────────────────

    private async Task DownloadFileAsync(
        string relativeUrl,
        string destPath,
        long? expectedSize,
        IProgress<double>? progress,
        CancellationToken ct)
    {
        var url = relativeUrl.StartsWith("http", StringComparison.OrdinalIgnoreCase)
            ? relativeUrl
            : $"{PrivStackApiClient.ApiBaseUrl}{relativeUrl}";

        using var response = await Http.GetAsync(url, HttpCompletionOption.ResponseHeadersRead, ct);
        response.EnsureSuccessStatusCode();

        var totalBytes = response.Content.Headers.ContentLength ?? expectedSize ?? 0;
        await using var contentStream = await response.Content.ReadAsStreamAsync(ct);
        await using var fileStream = new FileStream(destPath, FileMode.Create, FileAccess.Write, FileShare.None, 8192, true);

        var buffer = new byte[8192];
        long bytesRead = 0;
        int read;

        while ((read = await contentStream.ReadAsync(buffer, ct)) > 0)
        {
            await fileStream.WriteAsync(buffer.AsMemory(0, read), ct);
            bytesRead += read;

            if (totalBytes > 0)
            {
                // Scale download progress to 0–0.6 of total (rest is verify + extract + load)
                progress?.Report(0.6 * bytesRead / totalBytes);
            }
        }
    }

    private static async Task<string> ComputeSha256Async(string filePath, CancellationToken ct)
    {
        await using var stream = File.OpenRead(filePath);
        var hash = await SHA256.HashDataAsync(stream, ct);
        return Convert.ToHexStringLower(hash);
    }

    private static void TryDeleteFile(string path)
    {
        try { File.Delete(path); } catch { /* best-effort cleanup */ }
    }
}

/// <summary>
/// Deserialization target for the manifest.json inside a .pspkg package.
/// </summary>
internal sealed record PluginManifest
{
    public int FormatVersion { get; init; }
    public string? PluginId { get; init; }
    public string? Name { get; init; }
    public string? Version { get; init; }
    public string? MinAppVersion { get; init; }
    public string? Platform { get; init; }
    public string? Author { get; init; }
    public string? Description { get; init; }
    public string? Icon { get; init; }
    public string? Category { get; init; }
    public string? EntryDll { get; init; }
}
