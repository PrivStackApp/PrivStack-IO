using Avalonia.Controls;

namespace PrivStack.Desktop.Services.Abstractions;

/// <summary>
/// Desktop-only extension for Avalonia Window-specific settings operations.
/// Implemented by <see cref="AppSettingsService"/> alongside <see cref="PrivStack.Services.Abstractions.IAppSettingsService"/>.
/// </summary>
public interface IWindowSettingsService
{
    void UpdateWindowBounds(Window window);
    void ApplyToWindow(Window window);
}
