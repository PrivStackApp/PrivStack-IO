using System.Diagnostics;
using Serilog;

namespace PrivStack.Desktop.Services.Update;

/// <summary>
/// Applies an update for macOS .dmg installs by mounting, copying .app, and restarting.
/// </summary>
public sealed class MacOsDmgInstaller : IUpdateInstaller
{
    private static readonly ILogger Logger = Log.ForContext<MacOsDmgInstaller>();

    public async Task<bool> ApplyAndRestartAsync(string filePath)
    {
        string? mountPoint = null;

        try
        {
            Logger.Information("Mounting DMG: {Path}", filePath);

            // Mount the DMG
            var mountResult = await RunProcessCaptureAsync("hdiutil", $"attach \"{filePath}\" -nobrowse -noverify -noautoopen");
            if (mountResult.ExitCode != 0)
            {
                Logger.Error("hdiutil attach failed: {Error}", mountResult.StdErr);
                return false;
            }

            // Parse mount point from output (last line, last column)
            mountPoint = ParseMountPoint(mountResult.StdOut);
            if (mountPoint == null)
            {
                Logger.Error("Could not determine DMG mount point from: {Output}", mountResult.StdOut);
                return false;
            }

            // Find the .app bundle in the mounted DMG
            var appBundles = Directory.GetDirectories(mountPoint, "*.app");
            if (appBundles.Length == 0)
            {
                Logger.Error("No .app bundle found in mounted DMG at {Mount}", mountPoint);
                return false;
            }

            var sourceApp = appBundles[0];
            var destApp = "/Applications/" + Path.GetFileName(sourceApp);

            Logger.Information("Copying {Source} to {Dest}", sourceApp, destApp);

            // Remove old version and copy new
            if (Directory.Exists(destApp))
                Directory.Delete(destApp, recursive: true);

            await RunProcessCaptureAsync("cp", $"-R \"{sourceApp}\" \"{destApp}\"");

            // Detach
            await RunProcessCaptureAsync("hdiutil", $"detach \"{mountPoint}\"");
            mountPoint = null;

            // Restart the app
            Process.Start(new ProcessStartInfo
            {
                FileName = "open",
                Arguments = $"\"{destApp}\"",
                UseShellExecute = true
            });

            Environment.Exit(0);
            return true;
        }
        catch (Exception ex)
        {
            Logger.Error(ex, "Failed to apply DMG update");
            return false;
        }
        finally
        {
            // Clean up mount if still attached
            if (mountPoint != null)
            {
                try { await RunProcessCaptureAsync("hdiutil", $"detach \"{mountPoint}\""); }
                catch { /* best effort */ }
            }
        }
    }

    public Task ApplyOnExitAsync(string filePath)
    {
        var stagingPath = Path.Combine(DataPaths.BaseDir, "updates", "pending-dmg");
        File.WriteAllText(stagingPath, filePath);
        Logger.Information("Staged DMG update for next launch: {Path}", filePath);
        return Task.CompletedTask;
    }

    private static string? ParseMountPoint(string output)
    {
        // hdiutil output: last line typically ends with the mount point path
        var lines = output.Split('\n', StringSplitOptions.RemoveEmptyEntries);
        if (lines.Length == 0) return null;

        var lastLine = lines[^1].Trim();
        // Format: "/dev/disk4s2  Apple_HFS  /Volumes/PrivStack"
        var parts = lastLine.Split('\t');
        return parts.Length >= 3 ? parts[^1].Trim() : lastLine.Split("  ", StringSplitOptions.RemoveEmptyEntries).LastOrDefault()?.Trim();
    }

    private static async Task<ProcessResult> RunProcessCaptureAsync(string fileName, string arguments)
    {
        using var process = new Process
        {
            StartInfo = new ProcessStartInfo
            {
                FileName = fileName,
                Arguments = arguments,
                RedirectStandardOutput = true,
                RedirectStandardError = true,
                UseShellExecute = false,
                CreateNoWindow = true
            }
        };

        process.Start();
        var stdout = await process.StandardOutput.ReadToEndAsync();
        var stderr = await process.StandardError.ReadToEndAsync();
        await process.WaitForExitAsync();

        return new ProcessResult(process.ExitCode, stdout, stderr);
    }

    private record ProcessResult(int ExitCode, string StdOut, string StdErr);
}
