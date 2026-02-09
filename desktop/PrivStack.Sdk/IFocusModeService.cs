namespace PrivStack.Sdk;

/// <summary>
/// Allows plugins to request a distraction-free "focus mode" that hides
/// the host shell chrome (navigation sidebar, status bar, etc.).
/// </summary>
public interface IFocusModeService
{
    bool IsFocusMode { get; }
    event Action<bool>? FocusModeChanged;
    void SetFocusMode(bool enabled);
}
