// ============================================================================
// File: EmbeddingSpaceControl.Input.cs
// Description: Input handling (orbit drag, zoom, hit testing) for the
//              3D embedding space visualization control.
// ============================================================================

using Avalonia;
using Avalonia.Input;

namespace PrivStack.UI.Adaptive.Controls;

public sealed partial class EmbeddingSpaceControl
{
    private bool _isDragging;
    private Point _lastDragPoint;
    private int _hoveredIndex = -1;

    protected override void OnPointerPressed(PointerPressedEventArgs e)
    {
        base.OnPointerPressed(e);
        var pos = e.GetPosition(this);
        var props = e.GetCurrentPoint(this).Properties;

        if (props.IsLeftButtonPressed)
        {
            // Check for hit on a point first
            var hitIdx = HitTest(pos);
            if (hitIdx >= 0)
            {
                SelectPoint(hitIdx);
                e.Handled = true;
                return;
            }

            // Start orbit drag
            _isDragging = true;
            Camera.IsAutoRotating = false;
            _lastDragPoint = pos;
            e.Handled = true;
        }
    }

    protected override void OnPointerMoved(PointerEventArgs e)
    {
        base.OnPointerMoved(e);
        var pos = e.GetPosition(this);

        if (_isDragging)
        {
            var dx = pos.X - _lastDragPoint.X;
            var dy = pos.Y - _lastDragPoint.Y;
            Camera.Orbit(dx * 0.005, -dy * 0.005);
            _lastDragPoint = pos;
            e.Handled = true;
            return;
        }

        // Hover detection
        var hitIdx = HitTest(pos);
        if (hitIdx != _hoveredIndex)
        {
            if (_hoveredIndex >= 0 && _data != null && _hoveredIndex < _data.Points.Count)
                _data.Points[_hoveredIndex].IsHovered = false;

            _hoveredIndex = hitIdx;

            if (_hoveredIndex >= 0 && _data != null && _hoveredIndex < _data.Points.Count)
            {
                _data.Points[_hoveredIndex].IsHovered = true;
                PointHovered?.Invoke(_hoveredIndex);
            }
            InvalidateVisual();
        }
    }

    protected override void OnPointerReleased(PointerReleasedEventArgs e)
    {
        base.OnPointerReleased(e);
        _isDragging = false;
    }

    protected override void OnPointerWheelChanged(PointerWheelEventArgs e)
    {
        base.OnPointerWheelChanged(e);
        Camera.Zoom(e.Delta.Y);
        Camera.IsAutoRotating = false;
        _needsDepthSort = true;
        InvalidateVisual();
        e.Handled = true;
    }

    protected override void OnKeyDown(KeyEventArgs e)
    {
        base.OnKeyDown(e);
        if (e.Key == Key.Escape)
        {
            DeselectAll();
            PointDeselected?.Invoke();
            e.Handled = true;
        }
        else if (e.Key == Key.Space)
        {
            Camera.IsAutoRotating = !Camera.IsAutoRotating;
            e.Handled = true;
        }
    }

    private int HitTest(Point pos)
    {
        if (_data == null || _depthOrder == null) return -1;

        // Check from nearest to farthest (reverse depth order)
        for (int i = _depthOrder.Length - 1; i >= 0; i--)
        {
            var idx = _depthOrder[i];
            var p = _data.Points[idx];
            if (p.ScreenRadius <= 0) continue;

            var dx = pos.X - p.ScreenX;
            var dy = pos.Y - p.ScreenY;
            var hitRadius = Math.Max(p.ScreenRadius + 3, 6); // minimum hit area
            if (dx * dx + dy * dy <= hitRadius * hitRadius)
                return idx;
        }

        return -1;
    }

    private void SelectPoint(int index)
    {
        if (_data == null) return;

        DeselectAll();
        _data.Points[index].IsSelected = true;
        PointClicked?.Invoke(index);
        InvalidateVisual();
    }

    private void DeselectAll()
    {
        if (_data == null) return;
        foreach (var p in _data.Points) p.IsSelected = false;
    }
}
