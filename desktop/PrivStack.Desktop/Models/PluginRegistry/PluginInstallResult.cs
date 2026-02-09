namespace PrivStack.Desktop.Models.PluginRegistry;

/// <summary>
/// Result of a plugin install or update operation.
/// </summary>
public sealed record PluginInstallResult(
    bool Success,
    string? ErrorMessage = null,
    string? InstalledPath = null);
