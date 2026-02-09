using Avalonia;
using Avalonia.Controls;

namespace PrivStack.Desktop.Controls.Shared;

/// <summary>
/// Unified view toggle group for switching between different view modes.
/// Wrap buttons in this control for consistent styling.
/// </summary>
public partial class ViewToggleGroup : UserControl
{
    public static readonly StyledProperty<object?> TogglesProperty =
        AvaloniaProperty.Register<ViewToggleGroup, object?>(nameof(Toggles));

    /// <summary>
    /// The toggle buttons content (typically a StackPanel with Buttons)
    /// </summary>
    public object? Toggles
    {
        get => GetValue(TogglesProperty);
        set => SetValue(TogglesProperty, value);
    }

    public ViewToggleGroup()
    {
        InitializeComponent();
    }
}
