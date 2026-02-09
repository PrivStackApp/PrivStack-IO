using Avalonia;
using Avalonia.Controls;
using Avalonia.Media;

namespace PrivStack.Desktop.Controls.Shared;

/// <summary>
/// Unified footer bar for all plugin views.
/// Provides consistent layout: Stats | Status | Timestamp/Actions
/// </summary>
public partial class PluginFooter : UserControl
{
    public static readonly StyledProperty<string?> PrimaryLabelProperty =
        AvaloniaProperty.Register<PluginFooter, string?>(nameof(PrimaryLabel));

    public static readonly StyledProperty<string?> PrimaryValueProperty =
        AvaloniaProperty.Register<PluginFooter, string?>(nameof(PrimaryValue));

    public static readonly StyledProperty<string?> SecondaryLabelProperty =
        AvaloniaProperty.Register<PluginFooter, string?>(nameof(SecondaryLabel));

    public static readonly StyledProperty<string?> SecondaryValueProperty =
        AvaloniaProperty.Register<PluginFooter, string?>(nameof(SecondaryValue));

    public static readonly StyledProperty<object?> LeftContentProperty =
        AvaloniaProperty.Register<PluginFooter, object?>(nameof(LeftContent));

    public static readonly StyledProperty<string?> StatusTextProperty =
        AvaloniaProperty.Register<PluginFooter, string?>(nameof(StatusText));

    public static readonly StyledProperty<IBrush?> StatusColorProperty =
        AvaloniaProperty.Register<PluginFooter, IBrush?>(nameof(StatusColor));

    public static readonly StyledProperty<string?> TimestampProperty =
        AvaloniaProperty.Register<PluginFooter, string?>(nameof(Timestamp));

    public static readonly StyledProperty<object?> RightContentProperty =
        AvaloniaProperty.Register<PluginFooter, object?>(nameof(RightContent));

    /// <summary>
    /// Label for primary stat (e.g., "Items:", "Pages:")
    /// </summary>
    public string? PrimaryLabel
    {
        get => GetValue(PrimaryLabelProperty);
        set => SetValue(PrimaryLabelProperty, value);
    }

    /// <summary>
    /// Value for primary stat (e.g., "42", "12 blocks")
    /// </summary>
    public string? PrimaryValue
    {
        get => GetValue(PrimaryValueProperty);
        set => SetValue(PrimaryValueProperty, value);
    }

    /// <summary>
    /// Label for secondary stat
    /// </summary>
    public string? SecondaryLabel
    {
        get => GetValue(SecondaryLabelProperty);
        set => SetValue(SecondaryLabelProperty, value);
    }

    /// <summary>
    /// Value for secondary stat
    /// </summary>
    public string? SecondaryValue
    {
        get => GetValue(SecondaryValueProperty);
        set => SetValue(SecondaryValueProperty, value);
    }

    /// <summary>
    /// Custom content for left section
    /// </summary>
    public object? LeftContent
    {
        get => GetValue(LeftContentProperty);
        set => SetValue(LeftContentProperty, value);
    }

    /// <summary>
    /// Status text (e.g., "Saved", "Syncing...")
    /// </summary>
    public string? StatusText
    {
        get => GetValue(StatusTextProperty);
        set => SetValue(StatusTextProperty, value);
    }

    /// <summary>
    /// Status indicator color
    /// </summary>
    public IBrush? StatusColor
    {
        get => GetValue(StatusColorProperty);
        set => SetValue(StatusColorProperty, value);
    }

    /// <summary>
    /// Timestamp text (e.g., "Last modified: 5 min ago")
    /// </summary>
    public string? Timestamp
    {
        get => GetValue(TimestampProperty);
        set => SetValue(TimestampProperty, value);
    }

    /// <summary>
    /// Custom content for right section
    /// </summary>
    public object? RightContent
    {
        get => GetValue(RightContentProperty);
        set => SetValue(RightContentProperty, value);
    }

    public PluginFooter()
    {
        InitializeComponent();
    }
}
