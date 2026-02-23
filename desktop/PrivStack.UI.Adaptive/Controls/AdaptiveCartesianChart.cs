using Avalonia;
using Avalonia.Controls;
using LiveChartsCore;
using LiveChartsCore.Kernel.Sketches;
using LiveChartsCore.Measure;
using LiveChartsCore.SkiaSharpView.Avalonia;

namespace PrivStack.UI.Adaptive.Controls;

/// <summary>
/// Code-first wrapper around LiveCharts2 <see cref="CartesianChart"/>.
/// Plugins bind to <see cref="Series"/>, <see cref="XAxes"/>, and <see cref="YAxes"/>
/// via StyledProperties without referencing LiveCharts directly in their XAML,
/// avoiding source-generator issues in dynamically-loaded plugin assemblies.
/// </summary>
public sealed class AdaptiveCartesianChart : Border
{
    // -------------------------------------------------------------------------
    // Styled properties
    // -------------------------------------------------------------------------

    public static readonly StyledProperty<IEnumerable<ISeries>> SeriesProperty =
        AvaloniaProperty.Register<AdaptiveCartesianChart, IEnumerable<ISeries>>(
            nameof(Series), Array.Empty<ISeries>());

    public static readonly StyledProperty<IEnumerable<ICartesianAxis>> XAxesProperty =
        AvaloniaProperty.Register<AdaptiveCartesianChart, IEnumerable<ICartesianAxis>>(
            nameof(XAxes), Array.Empty<ICartesianAxis>());

    public static readonly StyledProperty<IEnumerable<ICartesianAxis>> YAxesProperty =
        AvaloniaProperty.Register<AdaptiveCartesianChart, IEnumerable<ICartesianAxis>>(
            nameof(YAxes), Array.Empty<ICartesianAxis>());

    public static readonly StyledProperty<LegendPosition> LegendPositionProperty =
        AvaloniaProperty.Register<AdaptiveCartesianChart, LegendPosition>(
            nameof(LegendPosition), LegendPosition.Hidden);

    // -------------------------------------------------------------------------
    // CLR accessors
    // -------------------------------------------------------------------------

    public IEnumerable<ISeries> Series
    {
        get => GetValue(SeriesProperty);
        set => SetValue(SeriesProperty, value);
    }

    public IEnumerable<ICartesianAxis> XAxes
    {
        get => GetValue(XAxesProperty);
        set => SetValue(XAxesProperty, value);
    }

    public IEnumerable<ICartesianAxis> YAxes
    {
        get => GetValue(YAxesProperty);
        set => SetValue(YAxesProperty, value);
    }

    public LegendPosition LegendPosition
    {
        get => GetValue(LegendPositionProperty);
        set => SetValue(LegendPositionProperty, value);
    }

    // -------------------------------------------------------------------------
    // Inner chart
    // -------------------------------------------------------------------------

    private readonly CartesianChart _chart;

    public AdaptiveCartesianChart()
    {
        _chart = new CartesianChart();
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
        else if (change.Property == XAxesProperty)
            _chart.XAxes = change.GetNewValue<IEnumerable<ICartesianAxis>>();
        else if (change.Property == YAxesProperty)
            _chart.YAxes = change.GetNewValue<IEnumerable<ICartesianAxis>>();
        else if (change.Property == LegendPositionProperty)
            _chart.LegendPosition = change.GetNewValue<LegendPosition>();
        else if (change.Property == WidthProperty)
            _chart.Width = Width;
        else if (change.Property == HeightProperty)
            _chart.Height = Height;
    }

}
