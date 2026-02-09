using System.Diagnostics;
using Serilog;

namespace PrivStack.Desktop.Services.Update;

/// <summary>
/// Applies an update for Windows .exe installs by launching the installer.
/// </summary>
public sealed class WindowsExeInstaller : IUpdateInstaller
{
    private static readonly ILogger Logger = Log.ForContext<WindowsExeInstaller>();

    public Task<bool> ApplyAndRestartAsync(string filePath)
    {
        try
        {
            Logger.Information("Launching Windows installer: {Path}", filePath);

            // Launch the downloaded installer with silent flag
            Process.Start(new ProcessStartInfo
            {
                FileName = filePath,
                Arguments = "/S",
                UseShellExecute = true
            });

            // Exit current process — the installer will handle the rest
            Environment.Exit(0);
            return Task.FromResult(true);
        }
        catch (Exception ex)
        {
            Logger.Error(ex, "Failed to launch Windows installer");
            return Task.FromResult(false);
        }
    }

    public Task ApplyOnExitAsync(string filePath)
    {
        var stagingPath = Path.Combine(DataPaths.BaseDir, "updates", "pending-exe");
        File.WriteAllText(stagingPath, filePath);
        Logger.Information("Staged Windows update for next launch: {Path}", filePath);
        return Task.CompletedTask;
    }
}
