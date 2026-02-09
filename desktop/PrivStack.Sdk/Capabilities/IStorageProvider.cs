namespace PrivStack.Sdk.Capabilities;

/// <summary>
/// Capability interface for plugins that provide file storage.
/// The host always registers a default local filesystem provider.
/// Plugins like Files can register additional providers (e.g., encrypted vault).
/// </summary>
public interface IStorageProvider
{
    /// <summary>
    /// Unique identifier for this provider (e.g., "default", "files-vault").
    /// </summary>
    string ProviderId { get; }

    /// <summary>
    /// Human-readable name for settings UI.
    /// </summary>
    string DisplayName { get; }

    /// <summary>
    /// Imports a file into storage and returns a stable file identifier.
    /// </summary>
    Task<string> StoreFileAsync(string sourcePath, string fileName, CancellationToken ct = default);

    /// <summary>
    /// Retrieves a stored file to a local path for display. Returns the local path, or null if not found.
    /// </summary>
    Task<string?> RetrieveFileAsync(string fileId, string fileName, CancellationToken ct = default);

    /// <summary>
    /// Deletes a stored file by its identifier. Returns true if the file was found and deleted.
    /// </summary>
    Task<bool> DeleteFileAsync(string fileId, string fileName, CancellationToken ct = default);

    /// <summary>
    /// Searches stored images by query string.
    /// </summary>
    Task<IReadOnlyList<StorageFileInfo>> SearchImagesAsync(string query, int maxResults = 50, CancellationToken ct = default);
}

/// <summary>
/// Lightweight file metadata returned by <see cref="IStorageProvider.SearchImagesAsync"/>.
/// </summary>
public record StorageFileInfo(string Id, string FileName, long SizeBytes, DateTime ModifiedAtUtc);
