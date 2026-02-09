using PrivStack.Sdk;

namespace PrivStack.Desktop.Services;

/// <summary>
/// Manages focus mode state. When active, the shell hides its navigation
/// sidebar and status bar so the current plugin can use the full window.
/// </summary>
public sealed class FocusModeService : IFocusModeService
{
    public bool IsFocusMode { get; private set; }
    public event Action<bool>? FocusModeChanged;

    public void SetFocusMode(bool enabled)
    {
        if (IsFocusMode == enabled) return;
        IsFocusMode = enabled;
        FocusModeChanged?.Invoke(enabled);
    }
}
