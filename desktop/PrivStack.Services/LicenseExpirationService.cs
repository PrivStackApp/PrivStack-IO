using System.Diagnostics;
using PrivStack.Services.Native;
using Serilog;

namespace PrivStack.Services;

/// <summary>
/// Tracks license expiration state and exposes it for the UI banner.
/// </summary>
public sealed class LicenseExpirationService
{
    private static readonly ILogger _log = Log.ForContext<LicenseExpirationService>();

    public bool IsExpired { get; private set; }

    /// <summary>
    /// Raised on the calling thread when the expiration state changes.
    /// Subscribers must dispatch to UI thread if needed.
    /// </summary>
    public event Action? ExpiredChanged;

    /// <summary>
    /// Checks the current license status and updates <see cref="IsExpired"/>.
    /// Call after native runtime initialization.
    /// </summary>
    public void CheckLicenseStatus(ILicensingService licensing)
    {
        var status = licensing.GetLicenseStatus();
        ApplyStatus(status);
    }

    /// <summary>
    /// Applies a license status received from a remote server (client mode).
    /// Parses the status string (e.g., "active", "expired", "readonly") to the enum.
    /// </summary>
    public void CheckLicenseStatusFromServer(string? statusString)
    {
        if (string.IsNullOrEmpty(statusString) ||
            !Enum.TryParse<LicenseStatus>(statusString, ignoreCase: true, out var status))
        {
            _log.Debug("Could not parse server license status '{Status}', assuming active", statusString);
            return;
        }

        ApplyStatus(status);
    }

    private void ApplyStatus(LicenseStatus status)
    {
        var wasExpired = IsExpired;
        IsExpired = status is LicenseStatus.ReadOnly or LicenseStatus.Expired or LicenseStatus.NotActivated;

        if (IsExpired)
            _log.Warning("License status is {Status} — app is in read-only mode", status);

        if (IsExpired != wasExpired)
            ExpiredChanged?.Invoke();
    }

    /// <summary>
    /// Called by SdkHost when a mutation is blocked by the Rust core.
    /// </summary>
    public void OnMutationBlocked()
    {
        if (IsExpired) return;

        IsExpired = true;
        _log.Warning("Mutation blocked by Rust core — switching to read-only mode");
        ExpiredChanged?.Invoke();
    }

    /// <summary>
    /// Opens the PrivStack pricing page in the default browser.
    /// </summary>
    public static void OpenPricingPage()
    {
        Process.Start(new ProcessStartInfo("https://privstack.io/pricing") { UseShellExecute = true });
    }
}
