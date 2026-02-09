using System;
using Avalonia;
using Avalonia.Controls;
using Avalonia.Controls.Primitives;
using Avalonia.Input;

namespace PrivStack.Desktop.Controls;

public partial class RangeSlider : UserControl
{
    public static readonly StyledProperty<double> MinimumProperty =
        AvaloniaProperty.Register<RangeSlider, double>(nameof(Minimum), 0);

    public static readonly StyledProperty<double> MaximumProperty =
        AvaloniaProperty.Register<RangeSlider, double>(nameof(Maximum), 100);

    public static readonly StyledProperty<double> LowerValueProperty =
        AvaloniaProperty.Register<RangeSlider, double>(nameof(LowerValue), 0,
            defaultBindingMode: Avalonia.Data.BindingMode.TwoWay, coerce: CoerceLowerValue);

    public static readonly StyledProperty<double> UpperValueProperty =
        AvaloniaProperty.Register<RangeSlider, double>(nameof(UpperValue), 100,
            defaultBindingMode: Avalonia.Data.BindingMode.TwoWay, coerce: CoerceUpperValue);

    /// <summary>
    /// Raised when the user starts dragging a thumb.
    /// </summary>
    public event EventHandler? DragStarted;

    /// <summary>
    /// Raised when the user finishes dragging a thumb.
    /// </summary>
    public event EventHandler? DragCompleted;

    /// <summary>
    /// Whether the slider is currently being dragged.
    /// </summary>
    public bool IsDragging { get; private set; }

    public double Minimum
    {
        get => GetValue(MinimumProperty);
        set => SetValue(MinimumProperty, value);
    }

    public double Maximum
    {
        get => GetValue(MaximumProperty);
        set => SetValue(MaximumProperty, value);
    }

    public double LowerValue
    {
        get => GetValue(LowerValueProperty);
        set => SetValue(LowerValueProperty, value);
    }

    public double UpperValue
    {
        get => GetValue(UpperValueProperty);
        set => SetValue(UpperValueProperty, value);
    }

    private Thumb? _minThumb;
    private Thumb? _maxThumb;
    private Border? _rangeHighlight;
    private Canvas? _thumbCanvas;

    public RangeSlider()
    {
        InitializeComponent();
    }

    protected override void OnApplyTemplate(TemplateAppliedEventArgs e)
    {
        base.OnApplyTemplate(e);
        SetupThumbs();
    }

    protected override void OnLoaded(global::Avalonia.Interactivity.RoutedEventArgs e)
    {
        base.OnLoaded(e);
        SetupThumbs();
    }

    private void SetupThumbs()
    {
        _minThumb = this.FindControl<Thumb>("MinThumb");
        _maxThumb = this.FindControl<Thumb>("MaxThumb");
        _rangeHighlight = this.FindControl<Border>("RangeHighlight");
        _thumbCanvas = this.FindControl<Canvas>("ThumbCanvas");

        if (_minThumb != null)
        {
            _minThumb.DragStarted += OnThumbDragStarted;
            _minThumb.DragDelta += OnMinThumbDrag;
            _minThumb.DragCompleted += OnThumbDragCompleted;
        }

        if (_maxThumb != null)
        {
            _maxThumb.DragStarted += OnThumbDragStarted;
            _maxThumb.DragDelta += OnMaxThumbDrag;
            _maxThumb.DragCompleted += OnThumbDragCompleted;
        }

        UpdateThumbPositions();
    }

    private void OnThumbDragStarted(object? sender, VectorEventArgs e)
    {
        IsDragging = true;
        DragStarted?.Invoke(this, EventArgs.Empty);
    }

    private void OnThumbDragCompleted(object? sender, VectorEventArgs e)
    {
        IsDragging = false;
        // Sync thumb positions to match the final values
        UpdateThumbPositions();
        DragCompleted?.Invoke(this, EventArgs.Empty);
    }

    private static double CoerceLowerValue(AvaloniaObject obj, double value)
    {
        var slider = (RangeSlider)obj;
        return Math.Clamp(value, slider.Minimum, slider.UpperValue);
    }

    private static double CoerceUpperValue(AvaloniaObject obj, double value)
    {
        var slider = (RangeSlider)obj;
        return Math.Clamp(value, slider.LowerValue, slider.Maximum);
    }

    protected override void OnPropertyChanged(AvaloniaPropertyChangedEventArgs change)
    {
        base.OnPropertyChanged(change);

        if (change.Property == LowerValueProperty ||
            change.Property == UpperValueProperty ||
            change.Property == MinimumProperty ||
            change.Property == MaximumProperty ||
            change.Property == BoundsProperty)
        {
            // During drag, only update the highlight bar - let Thumb handle its own position
            // This prevents a feedback loop that causes flickering
            if (IsDragging)
                UpdateRangeHighlight();
            else
                UpdateThumbPositions();
        }
    }

    private void OnMinThumbDrag(object? sender, VectorEventArgs e)
    {
        if (_thumbCanvas == null || _minThumb == null) return;

        var trackWidth = _thumbCanvas.Bounds.Width - 32; // Account for thumb width
        if (trackWidth <= 0) return;

        // Calculate new position and clamp it
        var currentX = Canvas.GetLeft(_minThumb) + e.Vector.X;
        var maxThumbX = Canvas.GetLeft(_maxThumb!) - 16; // Max position is just before the max thumb
        currentX = Math.Clamp(currentX, 0, maxThumbX - 2); // Min gap of 2px

        // Move the thumb directly - don't wait for value change to trigger UpdateThumbPositions
        Canvas.SetLeft(_minThumb, currentX);

        // Calculate and set the value for data binding
        var ratio = currentX / trackWidth;
        var newValue = Minimum + ratio * (Maximum - Minimum);
        LowerValue = newValue;

        // Update highlight based on actual thumb positions
        UpdateRangeHighlightFromThumbs();
    }

    private void OnMaxThumbDrag(object? sender, VectorEventArgs e)
    {
        if (_thumbCanvas == null || _maxThumb == null) return;

        var trackWidth = _thumbCanvas.Bounds.Width - 32;
        if (trackWidth <= 0) return;

        // Calculate new position and clamp it
        var currentX = Canvas.GetLeft(_maxThumb) + e.Vector.X;
        var minThumbX = Canvas.GetLeft(_minThumb!); // Min position is just after the min thumb
        currentX = Math.Clamp(currentX, minThumbX + 16 + 2, trackWidth + 16); // Min gap of 2px

        // Move the thumb directly - don't wait for value change to trigger UpdateThumbPositions
        Canvas.SetLeft(_maxThumb, currentX);

        // Calculate and set the value for data binding (remove the 16px offset for calculation)
        var ratio = (currentX - 16) / trackWidth;
        var newValue = Minimum + ratio * (Maximum - Minimum);
        UpperValue = newValue;

        // Update highlight based on actual thumb positions
        UpdateRangeHighlightFromThumbs();
    }

    private void UpdateThumbPositions()
    {
        if (_minThumb == null || _maxThumb == null || _rangeHighlight == null || _thumbCanvas == null)
            return;

        var trackWidth = _thumbCanvas.Bounds.Width - 32;
        if (trackWidth <= 0 || Maximum <= Minimum) return;

        var range = Maximum - Minimum;
        var minRatio = (LowerValue - Minimum) / range;
        var maxRatio = (UpperValue - Minimum) / range;

        var minX = minRatio * trackWidth;
        var maxX = maxRatio * trackWidth;

        Canvas.SetLeft(_minThumb, minX);
        Canvas.SetLeft(_maxThumb, maxX + 16); // Offset by thumb width

        // Update highlight
        _rangeHighlight.Margin = new Thickness(minX + 8, 0, 0, 0);
        _rangeHighlight.Width = Math.Max(0, maxX - minX + 16);
    }

    private void UpdateRangeHighlight()
    {
        if (_rangeHighlight == null || _thumbCanvas == null)
            return;

        var trackWidth = _thumbCanvas.Bounds.Width - 32;
        if (trackWidth <= 0 || Maximum <= Minimum) return;

        var range = Maximum - Minimum;
        var minRatio = (LowerValue - Minimum) / range;
        var maxRatio = (UpperValue - Minimum) / range;

        var minX = minRatio * trackWidth;
        var maxX = maxRatio * trackWidth;

        _rangeHighlight.Margin = new Thickness(minX + 8, 0, 0, 0);
        _rangeHighlight.Width = Math.Max(0, maxX - minX + 16);
    }

    private void UpdateRangeHighlightFromThumbs()
    {
        if (_minThumb == null || _maxThumb == null || _rangeHighlight == null)
            return;

        var minX = Canvas.GetLeft(_minThumb);
        var maxX = Canvas.GetLeft(_maxThumb) - 16; // Remove the offset

        _rangeHighlight.Margin = new Thickness(minX + 8, 0, 0, 0);
        _rangeHighlight.Width = Math.Max(0, maxX - minX + 16);
    }
}
