using System.Diagnostics;
using Serilog;

namespace PrivStack.Desktop.Services.Update;

/// <summary>
/// Applies an update for Linux AppImage installs by replacing the current binary.
/// </summary>
public sealed class LinuxAppImageInstaller : IUpdateInstaller
{
    private static readonly ILogger Logger = Log.ForContext<LinuxAppImageInstaller>();

    public async Task<bool> ApplyAndRestartAsync(string filePath)
    {
        var currentAppImage = Environment.GetEnvironmentVariable("APPIMAGE");
        if (string.IsNullOrEmpty(currentAppImage))
        {
            Logger.Warning("APPIMAGE env var not set — cannot determine current binary location");
            return false;
        }

        try
        {
            // Copy new AppImage over the current one
            Logger.Information("Replacing AppImage: {Current} with {New}", currentAppImage, filePath);
            File.Copy(filePath, currentAppImage, overwrite: true);

            // Make executable
            await RunProcessAsync("chmod", $"+x \"{currentAppImage}\"");

            // Launch the new version
            Process.Start(new ProcessStartInfo
            {
                FileName = currentAppImage,
                UseShellExecute = true
            });

            // Exit current process
            Environment.Exit(0);
            return true;
        }
        catch (Exception ex)
        {
            Logger.Error(ex, "Failed to apply AppImage update");
            return false;
        }
    }

    public Task ApplyOnExitAsync(string filePath)
    {
        // For AppImage, stage by noting the path — actual replace happens on next launch
        var stagingPath = Path.Combine(DataPaths.BaseDir, "updates", "pending-appimage");
        File.WriteAllText(stagingPath, filePath);
        Logger.Information("Staged AppImage update for next launch: {Path}", filePath);
        return Task.CompletedTask;
    }

    private static async Task RunProcessAsync(string fileName, string arguments)
    {
        using var process = Process.Start(new ProcessStartInfo
        {
            FileName = fileName,
            Arguments = arguments,
            RedirectStandardOutput = true,
            RedirectStandardError = true,
            UseShellExecute = false,
            CreateNoWindow = true
        });

        if (process != null)
            await process.WaitForExitAsync();
    }
}
