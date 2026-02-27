// ============================================================================
// File: SplitPaneLayout.cs
// Description: Reusable horizontal split pane with a draggable resize handle.
//              Ratio expresses the fraction of space given to the right pane.
//              Uses a custom Panel for layout to avoid star-column min-width
//              issues that plague Grid-based split implementations.
// ============================================================================

using Avalonia;
using Avalonia.Controls;
using Avalonia.Data;
using Avalonia.Input;
using Avalonia.Layout;
using Avalonia.Media;

namespace PrivStack.UI.Adaptive.Controls;

/// <summary>
/// A horizontal split layout with two content panes and a draggable resize handle.
/// <see cref="Ratio"/> controls the fraction of width given to <see cref="Pane2"/> (right).
/// Bind <see cref="Ratio"/> OneWay and listen to <see cref="RatioChanged"/> to persist on release.
/// </summary>
public sealed class SplitPaneLayout : Border
{
    // ── Styled Properties ──────────────────────────────────────────

    public static readonly StyledProperty<double> RatioProperty =
        AvaloniaProperty.Register<SplitPaneLayout, double>(
            nameof(Ratio), defaultValue: 0.5, defaultBindingMode: BindingMode.OneWay);

    public static readonly StyledProperty<double> MinRatioProperty =
        AvaloniaProperty.Register<SplitPaneLayout, double>(
            nameof(MinRatio), defaultValue: 0.25);

    public static readonly StyledProperty<double> MaxRatioProperty =
        AvaloniaProperty.Register<SplitPaneLayout, double>(
            nameof(MaxRatio), defaultValue: 0.75);

    public static readonly StyledProperty<double> HandleSizeProperty =
        AvaloniaProperty.Register<SplitPaneLayout, double>(
            nameof(HandleSize), defaultValue: 5.0);

    public static readonly StyledProperty<Control?> Pane1Property =
        AvaloniaProperty.Register<SplitPaneLayout, Control?>(nameof(Pane1));

    public static readonly StyledProperty<Control?> Pane2Property =
        AvaloniaProperty.Register<SplitPaneLayout, Control?>(nameof(Pane2));

    // ── CLR Accessors ──────────────────────────────────────────────

    public double Ratio { get => GetValue(RatioProperty); set => SetValue(RatioProperty, value); }
    public double MinRatio { get => GetValue(MinRatioProperty); set => SetValue(MinRatioProperty, value); }
    public double MaxRatio { get => GetValue(MaxRatioProperty); set => SetValue(MaxRatioProperty, value); }
    public double HandleSize { get => GetValue(HandleSizeProperty); set => SetValue(HandleSizeProperty, value); }
    public Control? Pane1 { get => GetValue(Pane1Property); set => SetValue(Pane1Property, value); }
    public Control? Pane2 { get => GetValue(Pane2Property); set => SetValue(Pane2Property, value); }

    // ── Events ─────────────────────────────────────────────────────

    /// <summary>Fired on pointer release after a drag. Use this to persist the ratio.</summary>
    public event EventHandler<double>? RatioChanged;

    // ── Private State ──────────────────────────────────────────────

    private readonly Border _pane1Host;
    private readonly Border _pane2Host;
    private readonly Border _handle;
    private readonly SplitPanel _panel;

    private bool _isResizing;
    private Point _resizeStart;
    private double _resizeStartRatio;
    private double _dragRatio = double.NaN;

    // ── Constructor ────────────────────────────────────────────────

    public SplitPaneLayout()
    {
        ClipToBounds = true;

        _pane1Host = new Border { ClipToBounds = true };
        _pane2Host = new Border { ClipToBounds = true };

        var handlePill = new Border
        {
            Width = 3,
            Height = 28,
            CornerRadius = new CornerRadius(2),
            HorizontalAlignment = HorizontalAlignment.Center,
            VerticalAlignment = VerticalAlignment.Center,
        };
        handlePill.Bind(BackgroundProperty,
            handlePill.GetResourceObservable("ThemeBorderSubtleBrush"));

        _handle = new Border
        {
            Background = Brushes.Transparent,
            Cursor = new Cursor(StandardCursorType.SizeWestEast),
            Child = handlePill,
        };
        _handle.PointerPressed += OnResizePressed;
        _handle.PointerMoved += OnResizeMoved;
        _handle.PointerReleased += OnResizeReleased;

        _panel = new SplitPanel(this);
        _panel.Children.Add(_pane1Host);
        _panel.Children.Add(_handle);
        _panel.Children.Add(_pane2Host);

        Child = _panel;
    }

    // ── Property Changed ───────────────────────────────────────────

    protected override void OnPropertyChanged(AvaloniaPropertyChangedEventArgs change)
    {
        base.OnPropertyChanged(change);

        if (change.Property == Pane1Property)
            _pane1Host.Child = Pane1;
        else if (change.Property == Pane2Property)
            _pane2Host.Child = Pane2;
        else if (change.Property == RatioProperty ||
                 change.Property == MinRatioProperty ||
                 change.Property == MaxRatioProperty ||
                 change.Property == HandleSizeProperty)
            _panel.InvalidateArrange();
    }

    // ── Resize Interaction ─────────────────────────────────────────

    private void OnResizePressed(object? sender, PointerPressedEventArgs e)
    {
        if (sender is not Border handle) return;
        _isResizing = true;
        _resizeStart = e.GetPosition(this);
        _resizeStartRatio = ActiveRatio;
        _dragRatio = _resizeStartRatio;
        e.Pointer.Capture(handle);
        e.Handled = true;
    }

    private void OnResizeMoved(object? sender, PointerEventArgs e)
    {
        if (!_isResizing) return;

        var totalWidth = Bounds.Width;
        if (totalWidth < 1) return;

        var current = e.GetPosition(this);
        var deltaX = current.X - _resizeStart.X;
        var availableWidth = totalWidth - HandleSize;
        if (availableWidth < 1) return;

        // Ratio is pane2's share. Dragging left → pane2 grows → ratio increases.
        var deltaRatio = -deltaX / availableWidth;
        var newRatio = _resizeStartRatio + deltaRatio;

        if (!double.IsFinite(newRatio)) return;

        _dragRatio = Math.Clamp(newRatio, MinRatio, MaxRatio);
        _panel.InvalidateArrange();
        e.Handled = true;
    }

    private void OnResizeReleased(object? sender, PointerReleasedEventArgs e)
    {
        if (!_isResizing) return;
        _isResizing = false;

        var finalRatio = _dragRatio;
        _dragRatio = double.NaN;

        if (double.IsFinite(finalRatio))
        {
            Ratio = finalRatio;
            RatioChanged?.Invoke(this, finalRatio);
        }

        e.Pointer.Capture(null);
        e.Handled = true;
    }

    // ── Active Ratio ───────────────────────────────────────────────

    internal double ActiveRatio
    {
        get
        {
            var r = _isResizing && double.IsFinite(_dragRatio) ? _dragRatio : Ratio;
            if (!double.IsFinite(r)) r = 0.5;
            return Math.Clamp(r, MinRatio, MaxRatio);
        }
    }

    // ── Custom Layout Panel ────────────────────────────────────────

    /// <summary>
    /// Inner panel that arranges [pane1Host, handle, pane2Host] using ratio-based
    /// math instead of Grid columns, avoiding star-column minimum width issues.
    /// </summary>
    private sealed class SplitPanel : Panel
    {
        private readonly SplitPaneLayout _owner;
        public SplitPanel(SplitPaneLayout owner) => _owner = owner;

        protected override Size MeasureOverride(Size availableSize)
        {
            var w = double.IsFinite(availableSize.Width) ? availableSize.Width : 0;
            var h = double.IsFinite(availableSize.Height) ? availableSize.Height : 0;

            if (Children.Count >= 3)
            {
                var handleWidth = _owner.HandleSize;
                var contentWidth = Math.Max(0, w - handleWidth);
                var ratio = _owner.ActiveRatio;
                var pane2W = contentWidth * ratio;
                var pane1W = contentWidth - pane2W;

                Children[0].Measure(new Size(pane1W, h));
                Children[1].Measure(new Size(handleWidth, h));
                Children[2].Measure(new Size(pane2W, h));
            }

            return new Size(w, h);
        }

        protected override Size ArrangeOverride(Size finalSize)
        {
            if (Children.Count < 3) return finalSize;

            var pane1 = Children[0];
            var handle = Children[1];
            var pane2 = Children[2];

            var handleWidth = _owner.HandleSize;
            var availableWidth = finalSize.Width - handleWidth;
            var ratio = _owner.ActiveRatio;

            var pane2Width = Math.Max(0, availableWidth * ratio);
            var pane1Width = Math.Max(0, availableWidth - pane2Width);

            pane1.Arrange(new Rect(0, 0, pane1Width, finalSize.Height));
            handle.Arrange(new Rect(pane1Width, 0, handleWidth, finalSize.Height));
            pane2.Arrange(new Rect(pane1Width + handleWidth, 0, pane2Width, finalSize.Height));

            return finalSize;
        }
    }
}
