using System.ComponentModel;
using Avalonia;
using Avalonia.Controls;
using PrivStack.UI.Adaptive.Services.SpellCheck;

namespace PrivStack.UI.Adaptive.Controls.Behaviors;

/// <summary>
/// Attached behavior that enables spell check and thesaurus context menu on TextBox controls.
/// </summary>
/// <example>
/// <code>
/// xmlns:behaviors="using:PrivStack.UI.Adaptive.Controls.Behaviors"
///
/// &lt;TextBox behaviors:SpellCheckBehavior.IsEnabled="True" /&gt;
/// </code>
/// </example>
public static class SpellCheckBehavior
{
    /// <summary>
    /// Attached property to enable/disable spell check context menu on a TextBox.
    /// </summary>
    public static readonly AttachedProperty<bool> IsEnabledProperty =
        AvaloniaProperty.RegisterAttached<TextBox, bool>(
            "IsEnabled",
            typeof(SpellCheckBehavior),
            defaultValue: false);

    static SpellCheckBehavior()
    {
        IsEnabledProperty.Changed.AddClassHandler<TextBox>(OnIsEnabledChanged);
    }

    /// <summary>
    /// Gets the IsEnabled value for a TextBox.
    /// </summary>
    public static bool GetIsEnabled(TextBox textBox)
    {
        return textBox.GetValue(IsEnabledProperty);
    }

    /// <summary>
    /// Sets the IsEnabled value for a TextBox.
    /// </summary>
    public static void SetIsEnabled(TextBox textBox, bool value)
    {
        textBox.SetValue(IsEnabledProperty, value);
    }

    private static void OnIsEnabledChanged(TextBox textBox, AvaloniaPropertyChangedEventArgs e)
    {
        if (e.NewValue is true)
        {
            // Enable spell check context menu
            textBox.ContextMenu ??= new ContextMenu();
            textBox.ContextMenu.Opening += OnContextMenuOpening;
        }
        else
        {
            // Disable spell check context menu
            if (textBox.ContextMenu != null)
            {
                textBox.ContextMenu.Opening -= OnContextMenuOpening;
            }
        }
    }

    private static void OnContextMenuOpening(object? sender, CancelEventArgs e)
    {
        if (sender is not ContextMenu menu)
            return;

        // Find the TextBox that owns this context menu
        var textBox = menu.PlacementTarget as TextBox;
        if (textBox == null)
            return;

        SpellCheckContextMenuHelper.HandleContextMenuOpening(textBox, e, menu);
    }
}
