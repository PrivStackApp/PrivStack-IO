using Avalonia.Controls;
using Avalonia.Controls.Primitives;
using Avalonia.VisualTree;

namespace PrivStack.Desktop.Plugins.Dashboard;

public partial class DashboardOverviewTab : UserControl
{
    private const double MinCardWidth = 260;
    private const double ColumnSpacing = 10;

    public DashboardOverviewTab()
    {
        InitializeComponent();
    }

    private static void InitializeComponent()
    {
        throw new NotImplementedException();
    }

    private void OnPluginGridSizeChanged(object? sender, SizeChangedEventArgs e)
    {
        if (sender is not ItemsControl host) return;

        var available = e.NewSize.Width;
        if (available <= 0) return;

        var columns = Math.Max(1, (int)((available + ColumnSpacing) / (MinCardWidth + ColumnSpacing)));

        var grid = host.GetVisualDescendants()
            .OfType<UniformGrid>()
            .FirstOrDefault();

        if (grid != null && grid.Columns != columns)
            grid.Columns = columns;
    }
}
