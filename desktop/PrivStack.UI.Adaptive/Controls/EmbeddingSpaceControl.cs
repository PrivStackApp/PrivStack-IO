// ============================================================================
// File: EmbeddingSpaceControl.cs
// Description: Renders a 3D point cloud of vector embeddings using perspective
//              projection onto Avalonia's DrawingContext. Depth-sorted with
//              entity-type coloring and similarity edges.
// ============================================================================

using Avalonia;
using Avalonia.Controls;
using Avalonia.Media;
using Avalonia.Threading;
using PrivStack.UI.Adaptive.Models;
using PrivStack.UI.Adaptive.Services;

namespace PrivStack.UI.Adaptive.Controls;

public sealed partial class EmbeddingSpaceControl : Control
{
    private EmbeddingSpaceData? _data;
    private DispatcherTimer? _timer;
    private int[]? _depthOrder;
    private bool _needsDepthSort = true;

    public PerspectiveCamera Camera { get; } = new();

    public event Action<int>? PointClicked;
    public event Action? PointDeselected;
    public event Action<int>? PointHovered;

    public void SetData(EmbeddingSpaceData data)
    {
        _data = data;
        _needsDepthSort = true;
        Camera.IsAutoRotating = true;

        // Center camera on the centroid of the point cloud
        if (data.Points.Count > 0)
        {
            double cx = 0, cy = 0, cz = 0;
            foreach (var p in data.Points) { cx += p.X; cy += p.Y; cz += p.Z; }
            var n = data.Points.Count;
            Camera.TargetX = cx / n;
            Camera.TargetY = cy / n;
            Camera.TargetZ = cz / n;

            // Set distance based on spread
            double maxDist = 0;
            foreach (var p in data.Points)
            {
                var d = Math.Sqrt(
                    Math.Pow(p.X - Camera.TargetX, 2) +
                    Math.Pow(p.Y - Camera.TargetY, 2) +
                    Math.Pow(p.Z - Camera.TargetZ, 2));
                maxDist = Math.Max(maxDist, d);
            }
            Camera.Distance = Math.Max(maxDist * 2.5, 1.0);
        }

        StartTimer();
        InvalidateVisual();
    }

    public void ClearData()
    {
        _data = null;
        _depthOrder = null;
        StopTimer();
        InvalidateVisual();
    }

    private void StartTimer()
    {
        if (_timer != null) return;
        _timer = new DispatcherTimer { Interval = TimeSpan.FromMilliseconds(16) };
        _timer.Tick += OnTick;
        _timer.Start();
    }

    private void StopTimer()
    {
        _timer?.Stop();
        _timer = null;
    }

    protected override void OnAttachedToVisualTree(VisualTreeAttachmentEventArgs e)
    {
        base.OnAttachedToVisualTree(e);
        if (_data != null) StartTimer();
    }

    protected override void OnDetachedFromVisualTree(VisualTreeAttachmentEventArgs e)
    {
        StopTimer();
        base.OnDetachedFromVisualTree(e);
    }

    private void OnTick(object? sender, EventArgs e)
    {
        Camera.Tick();
        _needsDepthSort = true;
        InvalidateVisual();
    }

    public override void Render(DrawingContext context)
    {
        var bounds = Bounds;
        if (bounds.Width < 1 || bounds.Height < 1) return;

        var centerX = bounds.Width / 2;
        var centerY = bounds.Height / 2;

        // Dark background
        context.DrawRectangle(
            GetBrush("ThemeSurfaceBrush", new SolidColorBrush(Color.Parse("#1a1a2e"))),
            null, new Rect(bounds.Size));

        if (_data == null || _data.Points.Count == 0)
        {
            DrawEmptyState(context, centerX, centerY);
            return;
        }

        // Project all points to screen space
        var points = _data.Points;
        for (int i = 0; i < points.Count; i++)
        {
            var p = points[i];
            var (sx, sy, depth) = Camera.WorldToScreen(p.X, p.Y, p.Z, centerX, centerY);
            p.ScreenX = sx;
            p.ScreenY = sy;
            p.ScreenRadius = depth > 0.1 ? Math.Clamp(Camera.Fov / depth * 0.04, 2, 20) : 0;
        }

        // Depth sort (painter's algorithm — far first)
        if (_needsDepthSort || _depthOrder == null)
        {
            _depthOrder = Enumerable.Range(0, points.Count).ToArray();
            var cam = Camera.GetPosition();
            Array.Sort(_depthOrder, (a, b) =>
            {
                var pa = points[a]; var pb = points[b];
                var da = DistSq(pa.X - cam.X, pa.Y - cam.Y, pa.Z - cam.Z);
                var db = DistSq(pb.X - cam.X, pb.Y - cam.Y, pb.Z - cam.Z);
                return db.CompareTo(da); // far first
            });
            _needsDepthSort = false;
        }

        // Draw edges
        DrawEdges(context, points);

        // Draw points (far to near)
        foreach (var idx in _depthOrder)
        {
            var p = points[idx];
            if (p.ScreenRadius <= 0) continue;

            var brush = GetEntityBrush(p.EntityType);
            var opacity = p.IsSelected ? 1.0 : p.IsHovered ? 0.9 : 0.7;

            if (brush is ISolidColorBrush scb)
            {
                var c = scb.Color;
                brush = new SolidColorBrush(Color.FromArgb((byte)(opacity * 255), c.R, c.G, c.B));
            }

            var center = new Point(p.ScreenX, p.ScreenY);
            context.DrawEllipse(brush, null, center, p.ScreenRadius, p.ScreenRadius);

            // Selected ring
            if (p.IsSelected)
            {
                var ringPen = new Pen(Brushes.White, 2.0);
                context.DrawEllipse(null, ringPen, center, p.ScreenRadius + 3, p.ScreenRadius + 3);
            }

            // Hover label
            if (p.IsHovered && !string.IsNullOrEmpty(p.Title))
            {
                DrawLabel(context, p.Title, p.ScreenX, p.ScreenY - p.ScreenRadius - 8);
            }
        }

        // Draw selected point label
        var selected = points.FirstOrDefault(p => p.IsSelected);
        if (selected != null && !selected.IsHovered)
        {
            DrawLabel(context, selected.Title, selected.ScreenX, selected.ScreenY - selected.ScreenRadius - 8);
        }
    }

    private void DrawEdges(DrawingContext context, List<EmbeddingPoint> points)
    {
        if (_data?.Edges == null) return;

        var edgeBrush = new SolidColorBrush(Color.FromArgb(40, 255, 255, 255));
        var edgePen = new Pen(edgeBrush, 0.5);

        foreach (var edge in _data.Edges)
        {
            if (edge.SourceIndex >= points.Count || edge.TargetIndex >= points.Count) continue;
            var s = points[edge.SourceIndex];
            var t = points[edge.TargetIndex];
            if (s.ScreenRadius <= 0 || t.ScreenRadius <= 0) continue;
            context.DrawLine(edgePen, new Point(s.ScreenX, s.ScreenY), new Point(t.ScreenX, t.ScreenY));
        }
    }

    private void DrawLabel(DrawingContext context, string text, double x, double y)
    {
        var labelBrush = GetBrush("ThemeTextPrimaryBrush", Brushes.White);
        var truncated = text.Length > 30 ? text[..27] + "..." : text;
        var ft = new FormattedText(truncated, System.Globalization.CultureInfo.CurrentCulture,
            FlowDirection.LeftToRight,
            new Typeface("Inter", FontStyle.Normal, FontWeight.Normal),
            11, labelBrush);
        context.DrawText(ft, new Point(x - ft.Width / 2, y - ft.Height));
    }

    private void DrawEmptyState(DrawingContext context, double cx, double cy)
    {
        var brush = GetBrush("ThemeTextMutedBrush", Brushes.Gray);
        var ft = new FormattedText("No embedding data", System.Globalization.CultureInfo.CurrentCulture,
            FlowDirection.LeftToRight,
            new Typeface("Inter", FontStyle.Normal, FontWeight.Normal),
            18, brush);
        context.DrawText(ft, new Point(cx - ft.Width / 2, cy - ft.Height / 2));
    }

    private static double DistSq(double dx, double dy, double dz) => dx * dx + dy * dy + dz * dz;

    private static IBrush GetEntityBrush(string entityType) => entityType switch
    {
        "note" or "page" or "sticky_note" => GetBrush("ThemeSecondaryBrush", Brushes.MediumPurple),
        "task" => GetBrush("ThemeSuccessBrush", Brushes.Green),
        "contact" => GetBrush("ThemeWarningBrush", Brushes.Orange),
        "event" or "calendar" => GetBrush("ThemePrimaryBrush", Brushes.DodgerBlue),
        "journal" => GetBrush("ThemeDangerBrush", Brushes.IndianRed),
        "snippet" => new SolidColorBrush(Color.Parse("#06B6D4")),
        "rss" => new SolidColorBrush(Color.Parse("#FB923C")),
        "file" => new SolidColorBrush(Color.Parse("#64748B")),
        _ => GetBrush("ThemeTextMutedBrush", Brushes.Gray),
    };

    private static IBrush GetBrush(string key, IBrush fallback)
    {
        var app = Avalonia.Application.Current;
        if (app is null) return fallback;
        if (app.Resources.TryGetResource(key, app.ActualThemeVariant, out var v) && v is IBrush b)
            return b;
        return app.FindResource(key) as IBrush ?? fallback;
    }
}
