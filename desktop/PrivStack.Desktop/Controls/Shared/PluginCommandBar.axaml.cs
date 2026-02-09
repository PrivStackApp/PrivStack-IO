using Avalonia;
using Avalonia.Controls;

namespace PrivStack.Desktop.Controls.Shared;

/// <summary>
/// Unified command bar for all plugin views.
/// Provides consistent layout: Title | Center Content | Actions
/// </summary>
public partial class PluginCommandBar : UserControl
{
    public static readonly StyledProperty<string> TitleProperty =
        AvaloniaProperty.Register<PluginCommandBar, string>(nameof(Title), string.Empty);

    public static readonly StyledProperty<string?> SubtitleProperty =
        AvaloniaProperty.Register<PluginCommandBar, string?>(nameof(Subtitle));

    public static readonly StyledProperty<object?> BadgeProperty =
        AvaloniaProperty.Register<PluginCommandBar, object?>(nameof(Badge));

    public static readonly StyledProperty<object?> CenterContentProperty =
        AvaloniaProperty.Register<PluginCommandBar, object?>(nameof(CenterContent));

    public static readonly StyledProperty<object?> ActionsProperty =
        AvaloniaProperty.Register<PluginCommandBar, object?>(nameof(Actions));

    public string Title
    {
        get => GetValue(TitleProperty);
        set => SetValue(TitleProperty, value);
    }

    /// <summary>
    /// Subtitle shown below the title (e.g., "Capture your thoughts")
    /// </summary>
    public string? Subtitle
    {
        get => GetValue(SubtitleProperty);
        set => SetValue(SubtitleProperty, value);
    }

    /// <summary>
    /// Optional badge content (e.g., streak counter, item count)
    /// </summary>
    public object? Badge
    {
        get => GetValue(BadgeProperty);
        set => SetValue(BadgeProperty, value);
    }

    /// <summary>
    /// Center content area for view toggles, search, filters, etc.
    /// </summary>
    public object? CenterContent
    {
        get => GetValue(CenterContentProperty);
        set => SetValue(CenterContentProperty, value);
    }

    /// <summary>
    /// Right-aligned action buttons
    /// </summary>
    public object? Actions
    {
        get => GetValue(ActionsProperty);
        set => SetValue(ActionsProperty, value);
    }

    public PluginCommandBar()
    {
        InitializeComponent();
    }
}
