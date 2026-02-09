using System.Runtime.InteropServices;

namespace PrivStack.Desktop.Services;

/// <summary>
/// Detects the current platform, architecture, and install format.
/// </summary>
public static class PlatformDetector
{
    public static string GetPlatform()
    {
        if (OperatingSystem.IsWindows()) return "windows";
        if (OperatingSystem.IsLinux()) return "linux";
        if (OperatingSystem.IsMacOS()) return "macos";
        return "unknown";
    }

    public static string GetArch() => RuntimeInformation.OSArchitecture switch
    {
        Architecture.X64 => "x64",
        Architecture.Arm64 => "arm64",
        _ => RuntimeInformation.OSArchitecture.ToString().ToLowerInvariant()
    };

    /// <summary>
    /// Attempts to detect how the app was installed based on process path and environment.
    /// </summary>
    public static string DetectCurrentInstallFormat()
    {
        var processPath = Environment.ProcessPath ?? "";
        var baseDir = AppContext.BaseDirectory;

        if (OperatingSystem.IsWindows())
        {
            // MSIX packages run from WindowsApps
            if (baseDir.Contains("WindowsApps", StringComparison.OrdinalIgnoreCase))
                return "msix";
            return "exe";
        }

        if (OperatingSystem.IsLinux())
        {
            // AppImage sets APPIMAGE env var and typically runs from /tmp/.mount_*
            if (!string.IsNullOrEmpty(Environment.GetEnvironmentVariable("APPIMAGE")))
                return "appimage";

            // deb installs go to /usr paths
            if (processPath.StartsWith("/usr/", StringComparison.Ordinal))
                return "deb";

            // Fallback — treat as AppImage if we can't determine
            return "appimage";
        }

        if (OperatingSystem.IsMacOS())
        {
            return "dmg";
        }

        return "unknown";
    }
}
