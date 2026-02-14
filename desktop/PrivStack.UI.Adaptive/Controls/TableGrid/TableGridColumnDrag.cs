// ============================================================================
// File: TableGridColumnDrag.cs
// Description: Column drag reorder for the TableGrid. Attaches drag behavior
//              to header cells with pointer capture, drop indicator, and
//              slot-based drop position calculation.
// ============================================================================

using Avalonia;
using Avalonia.Controls;
using Avalonia.Input;
using Avalonia.Layout;
using PrivStack.UI.Adaptive.Models;

namespace PrivStack.UI.Adaptive.Controls;

internal sealed class TableGridColumnDrag
{
    private bool _isDragging;
    private int _dragColIndex = -1;
    private Point _dragStartPoint;
    private Border? _dropIndicator;
    private const double DragThreshold = 10;

    private readonly Func<Grid> _getGrid;
    private readonly Func<int> _getColumnCount;
    private readonly Func<ITableGridDataSource?> _getSource;
    private readonly Action _onReorder;
    private readonly Func<Control> _getThemeSource;

    public TableGridColumnDrag(
        Func<Grid> getGrid,
        Func<int> getColumnCount,
        Func<ITableGridDataSource?> getSource,
        Action onReorder,
        Func<Control> getThemeSource)
    {
        _getGrid = getGrid;
        _getColumnCount = getColumnCount;
        _getSource = getSource;
        _onReorder = onReorder;
        _getThemeSource = getThemeSource;
    }

    /// <summary>
    /// Attaches drag behavior to a header cell border. Captures pointer on
    /// drag start (after threshold), shows drop indicator during drag, and
    /// fires reorder on release.
    /// </summary>
    public void AttachToHeader(Border headerBorder, int colIndex, Control relativeTo)
    {
        headerBorder.PointerPressed += (_, e) =>
        {
            if (e.GetCurrentPoint(relativeTo).Properties.IsLeftButtonPressed)
            {
                _isDragging = false;
                _dragColIndex = colIndex;
                _dragStartPoint = e.GetPosition(relativeTo);
            }
        };

        headerBorder.PointerMoved += (_, e) =>
        {
            if (_dragColIndex < 0) return;

            var pos = e.GetPosition(relativeTo);

            if (!_isDragging)
            {
                if (Math.Abs(pos.X - _dragStartPoint.X) > DragThreshold)
                {
                    _isDragging = true;
                    e.Pointer.Capture(headerBorder);
                }
                else
                    return;
            }

            var grid = _getGrid();
            var dropSlot = GetDropColumnSlot(pos.X, grid);
            ShowDropIndicator(dropSlot, grid);
            e.Handled = true;
        };

        headerBorder.PointerReleased += (_, e) =>
        {
            if (_dragColIndex < 0) return;

            var wasDragging = _isDragging;
            var dragFrom = _dragColIndex;

            _isDragging = false;
            _dragColIndex = -1;

            if (wasDragging)
            {
                e.Pointer.Capture(null);
                var grid = _getGrid();
                RemoveDropIndicator(grid);

                var pos = e.GetPosition(relativeTo);
                var dropSlot = GetDropColumnSlot(pos.X, grid);

                if (dropSlot >= 0 && dropSlot != dragFrom && dropSlot != dragFrom + 1)
                {
                    var source = _getSource();
                    if (source != null)
                    {
                        var toIndex = dropSlot > dragFrom ? dropSlot - 1 : dropSlot;
                        _ = source.OnColumnReorderedAsync(dragFrom, toIndex);
                        _onReorder();
                    }
                }

                e.Handled = true;
            }
        };
    }

    /// <summary>
    /// Returns the drop slot (0 through colCount) for the given X position.
    /// Slot N means "insert before column N" (slot colCount = after last column).
    /// </summary>
    private int GetDropColumnSlot(double x, Grid grid)
    {
        var colCount = _getColumnCount();

        // Start after drag handle column (column 0)
        double cumulativeWidth = grid.ColumnDefinitions.Count > 0
            ? grid.ColumnDefinitions[0].ActualWidth : 0;

        for (var c = 0; c < colCount; c++)
        {
            // Add grip width before this column (grip after previous column)
            if (c > 0)
            {
                var gripCol = (c - 1) * 2 + 2;
                if (gripCol < grid.ColumnDefinitions.Count)
                    cumulativeWidth += grid.ColumnDefinitions[gripCol].ActualWidth;
            }

            var gridCol = c * 2 + 1;
            var colWidth = gridCol < grid.ColumnDefinitions.Count
                ? grid.ColumnDefinitions[gridCol].ActualWidth : 100;

            if (x < cumulativeWidth + colWidth / 2)
                return c;

            cumulativeWidth += colWidth;
        }

        return colCount;
    }

    private void ShowDropIndicator(int dropSlot, Grid grid)
    {
        RemoveDropIndicator(grid);

        var colCount = _getColumnCount();
        var themeSource = _getThemeSource();

        _dropIndicator = new Border
        {
            Width = 2,
            Background = TableGridCellFactory.GetBrush(themeSource, "ThemePrimaryBrush"),
            VerticalAlignment = VerticalAlignment.Stretch,
            IsHitTestVisible = false,
            ZIndex = 10
        };

        int gridCol;
        HorizontalAlignment hAlign;

        if (dropSlot <= 0)
        {
            gridCol = 1; // first data column
            hAlign = HorizontalAlignment.Left;
        }
        else if (dropSlot >= colCount)
        {
            gridCol = (colCount - 1) * 2 + 1; // last data column
            hAlign = HorizontalAlignment.Right;
        }
        else
        {
            // Place on the grip column between dropSlot-1 and dropSlot
            gridCol = (dropSlot - 1) * 2 + 2;
            hAlign = HorizontalAlignment.Center;
        }

        Grid.SetRow(_dropIndicator, 0);
        Grid.SetColumn(_dropIndicator, gridCol);
        Grid.SetRowSpan(_dropIndicator, grid.RowDefinitions.Count);
        _dropIndicator.HorizontalAlignment = hAlign;

        grid.Children.Add(_dropIndicator);
    }

    private void RemoveDropIndicator(Grid grid)
    {
        if (_dropIndicator != null)
        {
            grid.Children.Remove(_dropIndicator);
            _dropIndicator = null;
        }
    }
}
