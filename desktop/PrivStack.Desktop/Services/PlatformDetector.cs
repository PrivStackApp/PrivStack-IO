using System.Runtime.InteropServices;

namespace PrivStack.Desktop.Services;

/// <summary>
/// Detects the current platform, architecture, install format, and hardware capabilities.
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
            return "deb";
        }

        if (OperatingSystem.IsMacOS())
        {
            return "dmg";
        }

        return "unknown";
    }

    /// <summary>
    /// Returns total physical memory in bytes, or 0 if detection fails.
    /// Uses GC memory info on all platforms (no P/Invoke needed).
    /// </summary>
    public static long GetTotalPhysicalMemoryBytes()
    {
        try
        {
            var gcInfo = GC.GetGCMemoryInfo();
            return gcInfo.TotalAvailableMemoryBytes;
        }
        catch
        {
            return 0;
        }
    }

    /// <summary>
    /// Returns total physical memory in GB (rounded).
    /// </summary>
    public static int GetTotalPhysicalMemoryGb()
    {
        var bytes = GetTotalPhysicalMemoryBytes();
        return bytes > 0 ? (int)Math.Round(bytes / 1_073_741_824.0) : 0;
    }

    /// <summary>
    /// Recommends a local LLM model based on available system RAM.
    /// </summary>
    public static (string ModelId, string Reason) RecommendLocalModel()
    {
        var ramGb = GetTotalPhysicalMemoryGb();

        if (ramGb >= 32)
            return ("qwen-2.5-32b", $"Your system has {ramGb} GB RAM — Qwen 2.5 32B provides the best local quality.");

        if (ramGb >= 16)
            return ("qwen-2.5-14b", $"Your system has {ramGb} GB RAM — Qwen 2.5 14B offers high quality responses.");

        if (ramGb >= 12)
            return ("qwen-2.5-7b", $"Your system has {ramGb} GB RAM — Qwen 2.5 7B offers a good balance of quality and speed.");

        return ("llama-3.2-3b", $"Your system has {ramGb} GB RAM — Llama 3.2 3B is lightweight and runs well on smaller systems.");
    }
}
