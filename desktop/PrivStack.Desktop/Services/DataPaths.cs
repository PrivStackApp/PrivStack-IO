namespace PrivStack.Desktop.Services;

/// <summary>
/// Central data directory resolution. Supports PRIVSTACK_DATA_DIR env var override
/// for isolated testing (e.g., build.sh --with-plugins).
/// </summary>
public static class DataPaths
{
    private static string? _cached;

    /// <summary>
    /// The base PrivStack data directory. Checks PRIVSTACK_DATA_DIR env var first,
    /// then falls back to the platform default (LocalApplicationData/PrivStack).
    /// </summary>
    public static string BaseDir => _cached ??= ResolveBaseDir();

    private static string ResolveBaseDir()
    {
        var envOverride = Environment.GetEnvironmentVariable("PRIVSTACK_DATA_DIR");
        if (!string.IsNullOrEmpty(envOverride))
            return envOverride;

        var appData = Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData);
        return Path.Combine(appData, "PrivStack");
    }
}
