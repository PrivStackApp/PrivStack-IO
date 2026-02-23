using Avalonia;
using Avalonia.Controls;
using LiveChartsCore;
using LiveChartsCore.Measure;
using LiveChartsCore.SkiaSharpView.Avalonia;

namespace PrivStack.UI.Adaptive.Controls;

/// <summary>
/// Code-first wrapper around LiveCharts2 <see cref="PieChart"/>.
/// Plugins bind to <see cref="Series"/> via StyledProperty without referencing
/// LiveCharts directly in their XAML.
/// </summary>
public sealed class AdaptivePieChart : Border
{
    // -------------------------------------------------------------------------
    // Styled properties
    // -------------------------------------------------------------------------

    public static readonly StyledProperty<IEnumerable<ISeries>> SeriesProperty =
        AvaloniaProperty.Register<AdaptivePieChart, IEnumerable<ISeries>>(
            nameof(Series), Array.Empty<ISeries>());

    public static readonly StyledProperty<LegendPosition> LegendPositionProperty =
        AvaloniaProperty.Register<AdaptivePieChart, LegendPosition>(
            nameof(LegendPosition), LegendPosition.Hidden);

    // -------------------------------------------------------------------------
    // CLR accessors
    // -------------------------------------------------------------------------

    public IEnumerable<ISeries> Series
    {
        get => GetValue(SeriesProperty);
        set => SetValue(SeriesProperty, value);
    }

    public LegendPosition LegendPosition
    {
        get => GetValue(LegendPositionProperty);
        set => SetValue(LegendPositionProperty, value);
    }

    // -------------------------------------------------------------------------
    // Inner chart
    // -------------------------------------------------------------------------

    private readonly PieChart _chart;

    public AdaptivePieChart()
    {
        _chart = new PieChart();
        Child = _chart;
    }

    // -------------------------------------------------------------------------
    // Property reactions
    // -------------------------------------------------------------------------

    protected override void OnPropertyChanged(AvaloniaPropertyChangedEventArgs change)
    {
        base.OnPropertyChanged(change);

        if (change.Property == SeriesProperty)
            _chart.Series = change.GetNewValue<IEnumerable<ISeries>>();
        else if (change.Property == LegendPositionProperty)
            _chart.LegendPosition = change.GetNewValue<LegendPosition>();
        else if (change.Property == WidthProperty)
            _chart.Width = Width;
        else if (change.Property == HeightProperty)
            _chart.Height = Height;
    }

}
