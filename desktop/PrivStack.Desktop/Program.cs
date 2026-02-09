using Avalonia;
using Avalonia.WebView.Desktop;
using PrivStack.Desktop.Services;
using System;

namespace PrivStack.Desktop;

sealed class Program
{
    // Initialization code. Don't use any Avalonia, third-party APIs or any
    // SynchronizationContext-reliant code before AppMain is called: things aren't initialized
    // yet and stuff might break.
    [STAThread]
    public static void Main(string[] args)
    {
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
