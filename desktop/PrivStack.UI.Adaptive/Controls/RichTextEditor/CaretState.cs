// ============================================================================
// File: CaretState.cs
// Description: Tracks caret position, selection range, and blink timer
//              for RichTextEditor.
// ============================================================================

using Avalonia.Threading;

namespace PrivStack.UI.Adaptive.Controls.RichTextEditor;

public sealed class CaretState : IDisposable
{
    private DispatcherTimer? _blinkTimer;

    /// <summary>
    /// Character index of the caret. 0 = before first char.
    /// </summary>
    public int Position { get; set; }

    /// <summary>
    /// Anchor for selection. null = no selection active.
    /// Selection range is [min(Position, Anchor), max(Position, Anchor)).
    /// </summary>
    public int? SelectionAnchor { get; set; }

    /// <summary>
    /// Whether the caret is currently visible (toggled by blink timer).
    /// </summary>
    public bool IsVisible { get; private set; } = true;

    public bool HasSelection => SelectionAnchor.HasValue && SelectionAnchor.Value != Position;

    public (int Start, int End) SelectionRange
    {
        get
        {
            if (!HasSelection) return (Position, Position);
            var anchor = SelectionAnchor!.Value;
            return anchor < Position ? (anchor, Position) : (Position, anchor);
        }
    }

    /// <summary>
    /// Fired when blink state changes and the control should repaint.
    /// </summary>
    public event Action? BlinkChanged;

    public void StartBlinking()
    {
        IsVisible = true;
        _blinkTimer?.Stop();
        _blinkTimer = new DispatcherTimer { Interval = TimeSpan.FromMilliseconds(530) };
        _blinkTimer.Tick += (_, _) =>
        {
            IsVisible = !IsVisible;
            BlinkChanged?.Invoke();
        };
        _blinkTimer.Start();
    }

    public void StopBlinking()
    {
        _blinkTimer?.Stop();
        _blinkTimer = null;
        IsVisible = false;
    }

    /// <summary>
    /// Reset blink to visible (call after any user input).
    /// </summary>
    public void ResetBlink()
    {
        IsVisible = true;
        _blinkTimer?.Stop();
        _blinkTimer?.Start();
    }

    public void ClearSelection() => SelectionAnchor = null;

    public void SelectAll(int documentLength)
    {
        SelectionAnchor = 0;
        Position = documentLength;
    }

    public void Dispose()
    {
        _blinkTimer?.Stop();
        _blinkTimer = null;
    }
}
