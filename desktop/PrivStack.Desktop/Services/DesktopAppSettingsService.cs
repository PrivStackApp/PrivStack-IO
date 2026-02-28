using Avalonia;
using Avalonia.Controls;
using PrivStack.Desktop.Services.Abstractions;

namespace PrivStack.Desktop.Services;

/// <summary>
/// Desktop-specific extension of <see cref="AppSettingsService"/> that adds
/// Avalonia Window position/size persistence.
/// </summary>
public class DesktopAppSettingsService : AppSettingsService, IWindowSettingsService
{
    public void ApplyToWindow(Window window)
    {
        if (Settings.WindowX.HasValue && Settings.WindowY.HasValue)
        {
            window.Position = new PixelPoint((int)Settings.WindowX.Value, (int)Settings.WindowY.Value);
        }

        window.Width = Settings.WindowWidth;
        window.Height = Settings.WindowHeight;

        if (Enum.TryParse<WindowState>(Settings.WindowState, out var state))
        {
            window.WindowState = state;
        }
    }

    public void UpdateWindowBounds(Window window)
    {
        if (window.WindowState == WindowState.Normal)
        {
            Settings.WindowX = window.Position.X;
            Settings.WindowY = window.Position.Y;
            Settings.WindowWidth = window.Width;
            Settings.WindowHeight = window.Height;
        }

        Settings.WindowState = window.WindowState.ToString();
        SaveDebounced();
    }
}
