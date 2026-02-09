using Avalonia;
using Avalonia.Controls;
using Avalonia.Media;
using PrivStack.Desktop.Services;

namespace PrivStack.Desktop.Controls;

/// <summary>
/// Custom control that renders the PrivStack logo with theme-aware colors.
/// The logo consists of a rounded rectangle border with a checkmark inside.
/// </summary>
public class LogoControl : Control
{
    /// <summary>
    /// Whether to show the background fill (for app icon) or just the border/checkmark (for inline use).
    /// </summary>
    public static readonly StyledProperty<bool> ShowBackgroundProperty =
        AvaloniaProperty.Register<LogoControl, bool>(nameof(ShowBackground), false);

    public bool ShowBackground
    {
        get => GetValue(ShowBackgroundProperty);
        set => SetValue(ShowBackgroundProperty, value);
    }

    /// <summary>
    /// The accent color to use for the border and checkmark. If null, uses theme primary color.
    /// </summary>
    public static readonly StyledProperty<Color?> AccentColorProperty =
        AvaloniaProperty.Register<LogoControl, Color?>(nameof(AccentColor));

    public Color? AccentColor
    {
        get => GetValue(AccentColorProperty);
        set => SetValue(AccentColorProperty, value);
    }

    private IDisposable? _primaryColorSubscription;

    public LogoControl()
    {
        ActualThemeVariantChanged += (_, _) => InvalidateVisual();
    }

    protected override void OnAttachedToVisualTree(VisualTreeAttachmentEventArgs e)
    {
        base.OnAttachedToVisualTree(e);
        // Re-render when ThemePrimary resource changes (covers PrivStack theme switches)
        _primaryColorSubscription = this.GetResourceObservable("ThemePrimary")
            .Subscribe(new ResourceChangeObserver(this));
    }

    protected override void OnDetachedFromVisualTree(VisualTreeAttachmentEventArgs e)
    {
        _primaryColorSubscription?.Dispose();
        _primaryColorSubscription = null;
        base.OnDetachedFromVisualTree(e);
    }

    private sealed class ResourceChangeObserver(LogoControl control) : IObserver<object?>
    {
        public void OnCompleted() { }
        public void OnError(Exception error) { }
        public void OnNext(object? value) => control.InvalidateVisual();
    }

    public override void Render(DrawingContext context)
    {
        base.Render(context);

        var bounds = Bounds;
        var size = Math.Min(bounds.Width, bounds.Height);
        if (size <= 0) return;

        // Scale factor to map from 512x512 SVG to actual size
        var scale = size / 512.0;

        // Center the logo
        var offsetX = (bounds.Width - size) / 2;
        var offsetY = (bounds.Height - size) / 2;

        using (context.PushTransform(Matrix.CreateTranslation(offsetX, offsetY)))
        using (context.PushTransform(Matrix.CreateScale(scale, scale)))
        {
            DrawLogo(context);
        }
    }

    private void DrawLogo(DrawingContext context)
    {
        // Get colors
        var accentColor = AccentColor ?? ThemeHelper.Primary;
        var backgroundColor = ThemeHelper.Background;

        var accentBrush = new SolidColorBrush(accentColor);
        var backgroundBrush = new SolidColorBrush(backgroundColor);

        // Stroke width scaled for the logo
        var borderStrokeWidth = 48.0;
        var checkmarkStrokeWidth = 44.0;

        var borderPen = new Pen(accentBrush, borderStrokeWidth);
        var checkmarkPen = new Pen(accentBrush, checkmarkStrokeWidth)
        {
            LineCap = PenLineCap.Round,
            LineJoin = PenLineJoin.Round
        };

        // Background rounded rectangle (if enabled)
        if (ShowBackground)
        {
            var backgroundRect = new RoundedRect(new Rect(32, 32, 448, 448), 64);
            context.DrawRectangle(backgroundBrush, null, backgroundRect);
        }

        // Border rounded rectangle
        // Inset by half stroke width to keep the stroke inside the bounds
        var borderRect = new RoundedRect(new Rect(32, 32, 448, 448), 64);
        context.DrawRectangle(null, borderPen, borderRect);

        // Checkmark path: M160 256 L224 320 L352 192
        var checkmarkGeometry = new PathGeometry();
        var figure = new PathFigure
        {
            StartPoint = new Point(160, 256),
            IsClosed = false,
            IsFilled = false
        };
        figure.Segments!.Add(new LineSegment { Point = new Point(224, 320) });
        figure.Segments.Add(new LineSegment { Point = new Point(352, 192) });
        checkmarkGeometry.Figures!.Add(figure);

        context.DrawGeometry(null, checkmarkPen, checkmarkGeometry);
    }

    protected override Size MeasureOverride(Size availableSize)
    {
        // Default size if not constrained
        var size = Math.Min(
            double.IsInfinity(availableSize.Width) ? 32 : availableSize.Width,
            double.IsInfinity(availableSize.Height) ? 32 : availableSize.Height
        );
        return new Size(size, size);
    }
}
