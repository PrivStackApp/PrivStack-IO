using System.ComponentModel;
using Avalonia;
using Avalonia.Controls;
using Avalonia.Controls.Primitives;
using Avalonia.VisualTree;
using PrivStack.UI.Adaptive.Services.SpellCheck;

namespace PrivStack.UI.Adaptive.Controls.Behaviors;

/// <summary>
/// Attached behavior that enables spell check context menu, red wavy underlines,
/// and autocorrect on TextBox controls.
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
    /// Attached property to enable/disable spell check on a TextBox.
    /// </summary>
    public static readonly AttachedProperty<bool> IsEnabledProperty =
        AvaloniaProperty.RegisterAttached<TextBox, bool>(
            "IsEnabled",
            typeof(SpellCheckBehavior),
            defaultValue: false);

    /// <summary>
    /// Internal attached property to store the adorner reference.
    /// </summary>
    private static readonly AttachedProperty<SpellCheckAdorner?> AdornerProperty =
        AvaloniaProperty.RegisterAttached<TextBox, SpellCheckAdorner?>(
            "SpellCheckAdorner",
            typeof(SpellCheckBehavior));

    static SpellCheckBehavior()
    {
        IsEnabledProperty.Changed.AddClassHandler<TextBox>(OnIsEnabledChanged);
    }

    public static bool GetIsEnabled(TextBox textBox) => textBox.GetValue(IsEnabledProperty);
    public static void SetIsEnabled(TextBox textBox, bool value) => textBox.SetValue(IsEnabledProperty, value);

    private static void OnIsEnabledChanged(TextBox textBox, AvaloniaPropertyChangedEventArgs e)
    {
        if (e.NewValue is true)
        {
            textBox.ContextMenu ??= new ContextMenu();
            textBox.ContextMenu.Opening += OnContextMenuOpening;

            // Attach adorner when TextBox is loaded into the visual tree
            textBox.AttachedToVisualTree += OnAttachedToVisualTree;
            textBox.DetachedFromVisualTree += OnDetachedFromVisualTree;

            // If already in the tree, attach now
            if (textBox.GetVisualRoot() != null)
                AttachAdorner(textBox);

            // Autocorrect: listen for text changes
            textBox.TextChanged += OnTextChangedForAutoCorrect;
        }
        else
        {
            if (textBox.ContextMenu != null)
                textBox.ContextMenu.Opening -= OnContextMenuOpening;

            textBox.AttachedToVisualTree -= OnAttachedToVisualTree;
            textBox.DetachedFromVisualTree -= OnDetachedFromVisualTree;
            textBox.TextChanged -= OnTextChangedForAutoCorrect;

            DetachAdorner(textBox);
        }
    }

    private static void OnAttachedToVisualTree(object? sender, VisualTreeAttachmentEventArgs e)
    {
        if (sender is TextBox textBox)
            AttachAdorner(textBox);
    }

    private static void OnDetachedFromVisualTree(object? sender, VisualTreeAttachmentEventArgs e)
    {
        if (sender is TextBox textBox)
            DetachAdorner(textBox);
    }

    private static void AttachAdorner(TextBox textBox)
    {
        if (textBox.GetValue(AdornerProperty) != null)
            return; // already attached

        var adornerLayer = AdornerLayer.GetAdornerLayer(textBox);
        if (adornerLayer == null) return;

        var adorner = new SpellCheckAdorner(textBox);
        AdornerLayer.SetAdornedElement(adorner, textBox);
        adornerLayer.Children.Add(adorner);
        textBox.SetValue(AdornerProperty, adorner);
    }

    private static void DetachAdorner(TextBox textBox)
    {
        var adorner = textBox.GetValue(AdornerProperty);
        if (adorner == null) return;

        adorner.Detach();

        var adornerLayer = AdornerLayer.GetAdornerLayer(textBox);
        adornerLayer?.Children.Remove(adorner);

        textBox.SetValue(AdornerProperty, null);
    }

    private static void OnContextMenuOpening(object? sender, CancelEventArgs e)
    {
        if (sender is not ContextMenu menu) return;
        if (menu.PlacementTarget is not TextBox textBox) return;

        SpellCheckContextMenuHelper.HandleContextMenuOpening(textBox, e, menu);
    }

    private static string _prevAutoCorrectText = "";

    private static void OnTextChangedForAutoCorrect(object? sender, TextChangedEventArgs e)
    {
        if (sender is not TextBox textBox) return;
        if (!AutoCorrectService.Instance.IsEnabled) return;

        var text = textBox.Text;
        if (string.IsNullOrEmpty(text)) { _prevAutoCorrectText = ""; return; }

        var caret = textBox.CaretIndex;
        if (caret < 2) { _prevAutoCorrectText = text; return; }

        // Only trigger on a boundary character being typed
        var lastChar = text[caret - 1];
        if (!IsWordBoundary(lastChar)) { _prevAutoCorrectText = text; return; }

        // Extract previous word
        var wordEnd = caret - 1;
        var wordStart = wordEnd;
        while (wordStart > 0 && SpellCheckContextMenuHelper.IsWordChar(text[wordStart - 1]))
            wordStart--;

        if (wordStart >= wordEnd) { _prevAutoCorrectText = text; return; }

        var word = text[wordStart..wordEnd];
        var correction = AutoCorrectService.Instance.GetCorrection(word);
        if (correction == null) { _prevAutoCorrectText = text; return; }

        // Replace the word
        var newText = text[..wordStart] + correction + text[wordEnd..];
        textBox.Text = newText;
        textBox.CaretIndex = wordStart + correction.Length + 1; // +1 for boundary char

        _prevAutoCorrectText = newText;
    }

    private static bool IsWordBoundary(char c) =>
        c == ' ' || c == '.' || c == ',' || c == ';' || c == ':' ||
        c == '!' || c == '?' || c == ')' || c == ']' || c == '\n';
}
