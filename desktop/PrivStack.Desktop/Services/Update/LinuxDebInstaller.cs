using System.Diagnostics;
using Serilog;

namespace PrivStack.Desktop.Services.Update;

/// <summary>
/// Applies an update for Linux .deb installs using pkexec + dpkg.
/// </summary>
public sealed class LinuxDebInstaller : IUpdateInstaller
{
    private static readonly ILogger Logger = Log.ForContext<LinuxDebInstaller>();

    public async Task<bool> ApplyAndRestartAsync(string filePath)
    {
        try
        {
            Logger.Information("Installing .deb package via pkexec: {Path}", filePath);

            // pkexec prompts the user for their password via a GUI dialog
            using var process = Process.Start(new ProcessStartInfo
            {
                FileName = "pkexec",
                Arguments = $"dpkg -i \"{filePath}\"",
                RedirectStandardOutput = true,
                RedirectStandardError = true,
                UseShellExecute = false,
                CreateNoWindow = true
            });

            if (process == null)
            {
                Logger.Error("Failed to start pkexec process");
                return false;
            }

            await process.WaitForExitAsync();

            if (process.ExitCode != 0)
            {
                var stderr = await process.StandardError.ReadToEndAsync();
                Logger.Error("dpkg install failed (exit {Code}): {Error}", process.ExitCode, stderr);
                return false;
            }

            // Restart from the standard install location
            Process.Start(new ProcessStartInfo
            {
                FileName = "/usr/bin/privstack",
                UseShellExecute = true
            });

            Environment.Exit(0);
            return true;
        }
        catch (Exception ex)
        {
            Logger.Error(ex, "Failed to apply .deb update");
            return false;
        }
    }

    public Task ApplyOnExitAsync(string filePath)
    {
        var stagingPath = Path.Combine(DataPaths.BaseDir, "updates", "pending-deb");
        File.WriteAllText(stagingPath, filePath);
        Logger.Information("Staged .deb update for next launch: {Path}", filePath);
        return Task.CompletedTask;
    }
}
