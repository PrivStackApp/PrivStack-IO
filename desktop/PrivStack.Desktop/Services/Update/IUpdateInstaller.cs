namespace PrivStack.Desktop.Services.Update;

/// <summary>
/// Platform-specific strategy for applying a downloaded update artifact.
/// </summary>
public interface IUpdateInstaller
{
    /// <summary>
    /// Applies the update and restarts the application.
    /// Returns false if the restart could not be performed automatically.
    /// </summary>
    Task<bool> ApplyAndRestartAsync(string filePath);

    /// <summary>
    /// Stages the update to be applied when the app exits.
    /// </summary>
    Task ApplyOnExitAsync(string filePath);
}
