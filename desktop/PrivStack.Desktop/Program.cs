using Avalonia;
using Avalonia.WebView.Desktop;
using PrivStack.Desktop.Services;
using System;
using System.IO;

namespace PrivStack.Desktop;

sealed class Program
{
    // Initialization code. Don't use any Avalonia, third-party APIs or any
    // SynchronizationContext-reliant code before AppMain is called: things aren't initialized
    // yet and stuff might break.
    [STAThread]
    public static void Main(string[] args)
    {
        // Load .env file for local development (won't overwrite existing env vars)
        LoadDotEnv();

        // Initialize logging first
        Log.Initialize();

        try
        {
            Log.Information("Application starting with args: {Args}", string.Join(", ", args));

            BuildAvaloniaApp().StartWithClassicDesktopLifetime(args);
        }
        catch (Exception ex)
        {
            Log.Fatal(ex, "Application crashed with unhandled exception");
            throw;
        }
        finally
        {
            Log.Shutdown();
        }
    }

    /// <summary>
    /// Loads KEY=VALUE pairs from a .env file into the process environment.
    /// Walks up from the executable directory to find the nearest .env file.
    /// Does not overwrite variables that are already set.
    /// </summary>
    private static void LoadDotEnv()
    {
        var dir = AppContext.BaseDirectory;
        string? envPath = null;

        // Walk up to find .env (handles bin/Debug/net9.0/ nesting)
        for (var d = new DirectoryInfo(dir); d != null; d = d.Parent)
        {
            var candidate = Path.Combine(d.FullName, ".env");
            if (File.Exists(candidate))
            {
                envPath = candidate;
                break;
            }
        }

        if (envPath == null) return;

        foreach (var line in File.ReadLines(envPath))
        {
            var trimmed = line.Trim();
            if (trimmed.Length == 0 || trimmed.StartsWith('#')) continue;

            var eqIndex = trimmed.IndexOf('=');
            if (eqIndex <= 0) continue;

            var key = trimmed[..eqIndex].Trim();
            var value = trimmed[(eqIndex + 1)..].Trim();

            // Don't overwrite — explicit env vars take precedence
            if (Environment.GetEnvironmentVariable(key) == null)
                Environment.SetEnvironmentVariable(key, value);
        }
    }

    // Avalonia configuration, don't remove; also used by visual designer.
    public static AppBuilder BuildAvaloniaApp()
    {
        var builder = AppBuilder.Configure<App>()
            .UsePlatformDetect()
            .WithInterFont()
            .LogToTrace();

        // Only enable WebView on Windows - macOS has .NET 9 compatibility issues
        // with the MacCatalyst bindings in WebView.Avalonia
        if (OperatingSystem.IsWindows())
        {
            builder = builder.UseDesktopWebView();
        }

        return builder;
    }
}
