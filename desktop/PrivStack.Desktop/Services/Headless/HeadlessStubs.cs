using System.ComponentModel;
using PrivStack.Desktop.Models;
using PrivStack.Desktop.Services.Abstractions;
using PrivStack.Sdk;
using IToastService = PrivStack.Sdk.IToastService;

#pragma warning disable CS0067 // Event is never used — required by interface contracts

namespace PrivStack.Desktop.Services.Headless;

/// <summary>
/// Executes actions synchronously on the calling thread. No UI dispatcher exists in headless mode.
/// </summary>
internal sealed class HeadlessUiDispatcher : IUiDispatcher
{
    public void Post(Action action) => action();

    public Task InvokeAsync(Action action)
    {
        action();
        return Task.CompletedTask;
    }

    public Task InvokeAsync(Func<Task> action) => action();
}

/// <summary>
/// Returns null/false for all dialog operations. No UI dialogs in headless mode.
/// </summary>
internal sealed class HeadlessDialogService : IDialogService
{
    public Avalonia.Controls.Window? Owner => null;
    public void SetOwner(Avalonia.Controls.Window owner) { }
    public Task<bool> ShowConfirmationAsync(string title, string message, string confirmButtonText = "Confirm") => Task.FromResult(false);
    public Task<string?> ShowPasswordConfirmationAsync(string title, string message, string confirmButtonText = "Confirm") => Task.FromResult<string?>(null);
    public Task<string?> ShowOpenFileDialogAsync(string title, (string Name, string Extension)[] filters) => Task.FromResult<string?>(null);
    public Task<string?> ShowSaveFileDialogAsync(string title, string defaultFileName, (string Name, string Extension)[] filters) => Task.FromResult<string?>(null);
    public Task<string?> ShowOpenFolderDialogAsync(string title) => Task.FromResult<string?>(null);
}

/// <summary>
/// Logs toast messages at Debug level. No visual toasts in headless mode.
/// </summary>
internal sealed class HeadlessToastService : IToastService
{
    private static readonly Serilog.ILogger _log = Serilog.Log.ForContext<HeadlessToastService>();

    public void Show(string message, ToastType type = ToastType.Info)
        => _log.Debug("[Toast:{Type}] {Message}", type, message);

    public void Show(string message, ToastType type, string actionLabel, Action action)
        => _log.Debug("[Toast:{Type}] {Message} (action: {Action})", type, message, actionLabel);
}

/// <summary>
/// Returns Dark theme defaults. No Avalonia resource dictionary in headless mode.
/// </summary>
internal sealed class HeadlessThemeService : IThemeService
{
    public AppTheme CurrentTheme { get => AppTheme.Dark; set { } }
    public bool IsDarkTheme => true;
    public bool IsLightTheme => false;
    public bool IsHighContrastTheme => false;
    public string? CurrentCustomThemeId => null;
    public void Initialize() { }
    public Dictionary<string, string> GetBuiltInThemeColors(AppTheme theme) => new();
    public void ApplyCustomTheme(CustomThemeDefinition theme) { }
    public void ApplyColorOverride(string key, string hex) { }
    public void SaveThemePreference(string themeString) { }
    public Dictionary<string, object> SnapshotCurrentColors() => new();
    public void RestoreSnapshot(Dictionary<string, object> snapshot) { }

    public event PropertyChangedEventHandler? PropertyChanged;
}

/// <summary>
/// Returns 1.0 scale. No font scaling in headless mode.
/// </summary>
internal sealed class HeadlessFontScaleService : IFontScaleService
{
    public double ScaleMultiplier { get => 1.0; set { } }
    public string ScaleDisplayText => "100%";
    public string CurrentFontFamily { get => "Inter"; set { } }
    public void Initialize() { }
    public void ReapplyScale() { }
    public double GetScaledSize(double baseSize) => baseSize;

    public event PropertyChangedEventHandler? PropertyChanged;
}

/// <summary>
/// Returns Desktop layout mode. No responsive layout in headless mode.
/// </summary>
internal sealed class HeadlessResponsiveLayoutService : IResponsiveLayoutService
{
    public LayoutMode CurrentMode => LayoutMode.Wide;
    public double ContentAreaWidth => 1920;
    public void UpdateContentAreaWidth(double width) { }
    public void Initialize() { }
    public void ReapplyLayout() { }

    public event PropertyChangedEventHandler? PropertyChanged;
}

/// <summary>
/// Returns false for all notification attempts. No system notifications in headless mode.
/// </summary>
internal sealed class HeadlessSystemNotificationService : ISystemNotificationService
{
    public Task<bool> SendNotificationAsync(string title, string body, string? subtitle = null, bool playSound = true)
        => Task.FromResult(false);
}

/// <summary>
/// Returns false for focus mode. No focus mode in headless mode.
/// </summary>
internal sealed class HeadlessFocusModeService : IFocusModeService
{
    public bool IsFocusMode => false;
    public event Action<bool>? FocusModeChanged;
    public void SetFocusMode(bool enabled) { }
}
