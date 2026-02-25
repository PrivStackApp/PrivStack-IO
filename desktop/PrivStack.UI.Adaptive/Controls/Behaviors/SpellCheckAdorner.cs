using Avalonia;
using Avalonia.Controls;
using Avalonia.VisualTree;
using Avalonia.Media;
using Avalonia.Threading;
using PrivStack.Sdk;
using PrivStack.UI.Adaptive.Services.SpellCheck;

namespace PrivStack.UI.Adaptive.Controls.Behaviors;

/// <summary>
/// Adorner control that renders red wavy underlines beneath misspelled words
/// in a TextBox. Attached via <see cref="SpellCheckBehavior"/>.
/// </summary>
internal sealed class SpellCheckAdorner : Control
{
    private readonly TextBox _textBox;
    private readonly Dictionary<string, bool> _cache = new(StringComparer.OrdinalIgnoreCase);
    private readonly System.Timers.Timer _debounce;
    private List<(double X, double Width, double Y)> _underlines = [];
    private string _lastCheckedText = "";

    public SpellCheckAdorner(TextBox textBox)
    {
        _textBox = textBox;
        IsHitTestVisible = false;

        _debounce = new System.Timers.Timer(300) { AutoReset = false };
        _debounce.Elapsed += (_, _) => Dispatcher.UIThread.Post(RebuildUnderlines);

        _textBox.PropertyChanged += OnTextBoxPropertyChanged;

        var spellCheck = HostServices.SpellCheck;
        if (spellCheck != null)
            spellCheck.DictionaryChanged += OnDictionaryChanged;
    }

    private void OnTextBoxPropertyChanged(object? sender, AvaloniaPropertyChangedEventArgs e)
    {
        if (e.Property == TextBox.TextProperty ||
            e.Property == TextBox.BoundsProperty)
        {
            _debounce.Stop();
            _debounce.Start();
        }
    }

    private void OnDictionaryChanged()
    {
        _cache.Clear();
        Dispatcher.UIThread.Post(RebuildUnderlines);
    }

    private void RebuildUnderlines()
    {
        var text = _textBox.Text;
        if (string.IsNullOrEmpty(text))
        {
            if (_underlines.Count > 0)
            {
                _underlines = [];
                InvalidateVisual();
            }
            return;
        }

        var spellCheck = HostServices.SpellCheck;
        if (spellCheck == null || !spellCheck.IsLoaded) return;

        var newUnderlines = new List<(double X, double Width, double Y)>();

        // Tokenize into words
        var i = 0;
        while (i < text.Length)
        {
            // Skip non-word chars
            while (i < text.Length && !SpellCheckContextMenuHelper.IsWordChar(text[i]))
                i++;

            if (i >= text.Length) break;

            var wordStart = i;
            while (i < text.Length && SpellCheckContextMenuHelper.IsWordChar(text[i]))
                i++;

            var word = text[wordStart..i];

            // Skip pure digits or single chars
            if (word.Length <= 1 || word.All(char.IsDigit))
                continue;

            if (!CheckWord(spellCheck, word))
            {
                // Measure position using FormattedText matching TextBox font
                var rect = MeasureWordRect(text, wordStart, i);
                if (rect.Width > 0)
                    newUnderlines.Add((rect.X, rect.Width, rect.Y));
            }
        }

        _underlines = newUnderlines;
        _lastCheckedText = text;
        InvalidateVisual();
    }

    private bool CheckWord(ISpellCheckService spellCheck, string word)
    {
        if (_cache.TryGetValue(word, out var cached))
            return cached;

        var result = spellCheck.Check(word);
        _cache[word] = result;
        return result;
    }

    private (double X, double Width, double Y) MeasureWordRect(string text, int start, int end)
    {
        // Use the TextBox's presenter to get text positioning
        var presenter = _textBox.GetVisualDescendants()
            .OfType<Avalonia.Controls.Presenters.TextPresenter>()
            .FirstOrDefault();

        if (presenter == null)
            return (0, 0, 0);

        var textLayout = presenter.TextLayout;
        if (textLayout == null)
            return (0, 0, 0);

        var startHit = textLayout.HitTestTextPosition(start);
        var endHit = textLayout.HitTestTextPosition(end);

        // For multi-line, use the start line's Y + height
        var lineY = startHit.Top + presenter.Bounds.Top;
        var lineHeight = startHit.Bottom - startHit.Top;

        var x = startHit.Left + presenter.Bounds.Left;
        var width = endHit.Left - startHit.Left;

        // Underline Y is at the bottom of the text line
        var underlineY = lineY + lineHeight;

        return (x, Math.Max(width, 0), underlineY);
    }

    public override void Render(DrawingContext ctx)
    {
        base.Render(ctx);

        foreach (var (x, width, y) in _underlines)
        {
            SpellCheckRendering.DrawWavyUnderline(ctx, x, y, width);
        }
    }

    public void Detach()
    {
        _textBox.PropertyChanged -= OnTextBoxPropertyChanged;
        _debounce.Stop();
        _debounce.Dispose();

        var spellCheck = HostServices.SpellCheck;
        if (spellCheck != null)
            spellCheck.DictionaryChanged -= OnDictionaryChanged;
    }
}
