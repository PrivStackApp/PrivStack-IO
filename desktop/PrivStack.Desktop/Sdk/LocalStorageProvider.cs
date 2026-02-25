using System.Security.Cryptography;
using PrivStack.Sdk.Capabilities;
using Serilog;

namespace PrivStack.Desktop.Sdk;

/// <summary>
/// Default IStorageProvider that stores files on the local filesystem.
/// Always available, no plugin dependency required.
/// </summary>
internal sealed class LocalStorageProvider : IStorageProvider
{
    private static readonly ILogger _log = Log.ForContext<LocalStorageProvider>();

    private static readonly HashSet<string> ImageExtensions = new(StringComparer.OrdinalIgnoreCase)
    {
        ".jpg", ".jpeg", ".png", ".gif", ".bmp", ".webp", ".ico", ".tiff", ".tif", ".svg"
    };

    private string? _cachedStoragePath;

    /// <summary>
    /// Lazy storage path: workspace-scoped if active, legacy fallback otherwise.
    /// </summary>
    private string StoragePath
    {
        get
        {
            var wsDir = PrivStack.Desktop.Services.DataPaths.WorkspaceDataDir;
            var target = wsDir != null
                ? Path.Combine(wsDir, "files", "notes")
                : Path.Combine(PrivStack.Desktop.Services.DataPaths.BaseDir, "quill-images");

            if (_cachedStoragePath != target || !Directory.Exists(target))
            {
                _cachedStoragePath = target;
                Directory.CreateDirectory(target);
            }

            return target;
        }
    }

    public string ProviderId => "default";
    public string DisplayName => "Local Storage";

    public async Task<string> StoreFileAsync(string sourcePath, string fileName, CancellationToken ct = default)
    {
        ct.ThrowIfCancellationRequested();

        var ext = Path.GetExtension(fileName);
        var storagePath = StoragePath;

        // Run blocking file I/O off the UI thread. Cloud-mounted files (Google Drive,
        // iCloud) can stall or timeout on synchronous reads — this prevents the app
        // from hanging or crashing when dragging from those locations.
        return await Task.Run(async () =>
        {
            var hash = await HashFileAsync(sourcePath, ct);
            var destPath = Path.Combine(storagePath, hash + ext);

            if (File.Exists(destPath))
            {
                _log.Debug("LocalStorage: dedup hit for {FileName}, reusing {Id}", fileName, hash);
                return hash;
            }

            File.Copy(sourcePath, destPath, overwrite: false);
            _log.Debug("LocalStorage: stored {FileName} as {Id}", fileName, hash);

            return hash;
        }, ct);
    }

    private static async Task<string> HashFileAsync(string path, CancellationToken ct)
    {
        // Use async file stream with a 30-second timeout to handle cloud-mounted
        // files that may stall (Google Drive, iCloud, OneDrive).
        using var timeoutCts = CancellationTokenSource.CreateLinkedTokenSource(ct);
        timeoutCts.CancelAfter(TimeSpan.FromSeconds(30));

        await using var stream = new FileStream(
            path, FileMode.Open, FileAccess.Read, FileShare.Read,
            bufferSize: 81920, useAsync: true);

        using var sha = SHA256.Create();
        var hashBytes = await sha.ComputeHashAsync(stream, timeoutCts.Token);
        return Convert.ToHexString(hashBytes).ToLowerInvariant();
    }

    public Task<string?> RetrieveFileAsync(string fileId, string fileName, CancellationToken ct = default)
    {
        ct.ThrowIfCancellationRequested();

        // Try to find the file by ID prefix (ID stored without extension)
        var ext = Path.GetExtension(fileName);
        var exactPath = Path.Combine(StoragePath, fileId + ext);
        if (File.Exists(exactPath))
            return Task.FromResult<string?>(exactPath);

        // Fallback: scan for any file starting with the ID
        var match = Directory.EnumerateFiles(StoragePath, fileId + ".*").FirstOrDefault();
        return Task.FromResult(match);
    }

    public Task<bool> DeleteFileAsync(string fileId, string fileName, CancellationToken ct = default)
    {
        ct.ThrowIfCancellationRequested();

        if (!Directory.Exists(StoragePath))
        {
            _log.Debug("LocalStorage: storage path does not exist, nothing to delete for {FileId}", fileId);
            return Task.FromResult(false);
        }

        var ext = Path.GetExtension(fileName);
        var exactPath = Path.Combine(StoragePath, fileId + ext);
        if (File.Exists(exactPath))
        {
            File.Delete(exactPath);
            _log.Debug("LocalStorage: deleted {FileId}{Ext}", fileId, ext);
            return Task.FromResult(true);
        }

        // Fallback: scan for any file starting with the ID
        var match = Directory.EnumerateFiles(StoragePath, fileId + ".*").FirstOrDefault();
        if (match != null)
        {
            File.Delete(match);
            _log.Debug("LocalStorage: deleted {Path}", match);
            return Task.FromResult(true);
        }

        return Task.FromResult(false);
    }

    public Task<IReadOnlyList<StorageFileInfo>> SearchImagesAsync(string query, int maxResults = 50, CancellationToken ct = default)
    {
        ct.ThrowIfCancellationRequested();

        var files = Directory.EnumerateFiles(StoragePath)
            .Where(f => ImageExtensions.Contains(Path.GetExtension(f)))
            .Where(f => string.IsNullOrEmpty(query) ||
                        Path.GetFileName(f).Contains(query, StringComparison.OrdinalIgnoreCase))
            .Select(f =>
            {
                var info = new FileInfo(f);
                var id = Path.GetFileNameWithoutExtension(f);
                return new StorageFileInfo(id, info.Name, info.Length, info.LastWriteTimeUtc);
            })
            .OrderByDescending(f => f.ModifiedAtUtc)
            .Take(maxResults)
            .ToList();

        return Task.FromResult<IReadOnlyList<StorageFileInfo>>(files);
    }
}
