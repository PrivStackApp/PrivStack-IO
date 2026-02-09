using PrivStack.Desktop.Models;

namespace PrivStack.Desktop.Services.Abstractions;

/// <summary>
/// Abstraction for checking, downloading, and applying application updates
/// via the PrivStack registry API.
/// </summary>
public interface IUpdateService
{
    /// <summary>
    /// Current application version string.
    /// </summary>
    string CurrentVersion { get; }

    /// <summary>
    /// Checks the registry for a newer release.
    /// Returns the release info if an update is available, null otherwise.
    /// </summary>
    Task<LatestReleaseInfo?> CheckForUpdatesAsync(CancellationToken ct = default);

    /// <summary>
    /// Downloads the update artifact matching the current platform.
    /// Reports progress as 0–100 integer percentage.
    /// Returns the local file path of the downloaded artifact, or null on failure.
    /// </summary>
    Task<string?> DownloadUpdateAsync(IProgress<int>? progress = null, CancellationToken ct = default);

    /// <summary>
    /// Applies the downloaded update and restarts the application.
    /// Returns false if the restart could not be performed (user should restart manually).
    /// </summary>
    Task<bool> ApplyUpdateAndRestartAsync(CancellationToken ct = default);

    /// <summary>
    /// Raised when an update check finds a newer version.
    /// </summary>
    event EventHandler<LatestReleaseInfo>? UpdateFound;

    /// <summary>
    /// Raised when an error occurs during any update operation.
    /// </summary>
    event EventHandler<Exception>? UpdateError;
}
