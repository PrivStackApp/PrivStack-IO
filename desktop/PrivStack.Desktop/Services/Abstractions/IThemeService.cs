using System.ComponentModel;

namespace PrivStack.Desktop.Services.Abstractions;

/// <summary>
/// Abstraction over theme management.
/// </summary>
public interface IThemeService : INotifyPropertyChanged
{
    AppTheme CurrentTheme { get; set; }
    bool IsDarkTheme { get; }
    bool IsLightTheme { get; }
    bool IsHighContrastTheme { get; }
    void Initialize();
}
