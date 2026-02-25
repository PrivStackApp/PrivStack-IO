// ============================================================================
// File: RichTextEditor.SpellCheck.cs
// Description: Spell check underlines, context menu, and autocorrect for
//              the RichTextEditor control.
// ============================================================================

using Avalonia;
using Avalonia.Controls;
using Avalonia.Media;
using Avalonia.Threading;
using PrivStack.Sdk;
using PrivStack.UI.Adaptive.Services.SpellCheck;

namespace PrivStack.UI.Adaptive.Controls.RichTextEditor;

public sealed partial class RichTextEditor
{
    private List<(double X, double Width, double Y)> _misspelledUnderlines = [];
    private readonly Dictionary<string, bool> _spellCache = new(StringComparer.OrdinalIgnoreCase);
    private System.Timers.Timer? _spellDebounce;
    private bool _spellCheckSubscribed;

    public static readonly StyledProperty<bool> IsSpellCheckEnabledProperty =
        AvaloniaProperty.Register<RichTextEditor, bool>(nameof(IsSpellCheckEnabled), true);

    public bool IsSpellCheckEnabled
    {
        get => GetValue(IsSpellCheckEnabledProperty);
        set => SetValue(IsSpellCheckEnabledProperty, value);
    }

    private void InitSpellCheck()
    {
        _spellDebounce = new System.Timers.Timer(500) { AutoReset = false };
        _spellDebounce.Elapsed += (_, _) => Dispatcher.UIThread.Post(RebuildSpellCheck);
    }

    private void SubscribeSpellCheck()
    {
        if (_spellCheckSubscribed) return;
        var spellCheck = HostServices.SpellCheck;
        if (spellCheck != null)
        {
            spellCheck.DictionaryChanged += OnSpellDictionaryChanged;
            _spellCheckSubscribed = true;
        }
    }

    private void UnsubscribeSpellCheck()
    {
        if (!_spellCheckSubscribed) return;
        var spellCheck = HostServices.SpellCheck;
        if (spellCheck != null)
            spellCheck.DictionaryChanged -= OnSpellDictionaryChanged;
        _spellCheckSubscribed = false;

        _spellDebounce?.Stop();
        _spellDebounce?.Dispose();
        _spellDebounce = null;
    }

    private void OnSpellDictionaryChanged()
    {
        _spellCache.Clear();
        Dispatcher.UIThread.Post(RebuildSpellCheck);
    }

    private void ScheduleSpellCheck()
    {
        if (!IsSpellCheckEnabled) return;
        _spellDebounce?.Stop();
        _spellDebounce?.Start();
    }

    private void RebuildSpellCheck()
    {
        if (!IsSpellCheckEnabled)
        {
            if (_misspelledUnderlines.Count > 0)
            {
                _misspelledUnderlines = [];
                InvalidateVisual();
            }
            return;
        }

        var spellCheck = HostServices.SpellCheck;
        if (spellCheck == null || !spellCheck.IsLoaded) return;

        var text = _doc.Text;
        if (string.IsNullOrEmpty(text))
        {
            _misspelledUnderlines = [];
            InvalidateVisual();
            return;
        }

        var newUnderlines = new List<(double X, double Width, double Y)>();

        // Tokenize into words
        var i = 0;
        while (i < text.Length)
        {
            while (i < text.Length && !SpellCheckContextMenuHelper.IsWordChar(text[i]))
                i++;

            if (i >= text.Length) break;
            var wordStart = i;

            while (i < text.Length && SpellCheckContextMenuHelper.IsWordChar(text[i]))
                i++;

            var word = text[wordStart..i];
            if (word.Length <= 1 || word.All(char.IsDigit))
                continue;

            // Skip words inside code spans
            if (wordStart < _doc.Length && _doc.GetStyleAt(wordStart).HasFlag(InlineStyle.Code))
                continue;

            if (!CheckWordCached(spellCheck, word))
            {
                // Use TextLayoutEngine to get the position
                var rects = _layout.GetSelectionRects(wordStart, i);
                foreach (var rect in rects)
                {
                    newUnderlines.Add((rect.X, rect.Width, rect.Y + rect.Height));
                }
            }
        }

        _misspelledUnderlines = newUnderlines;
        InvalidateVisual();
    }

    private bool CheckWordCached(ISpellCheckService spellCheck, string word)
    {
        if (_spellCache.TryGetValue(word, out var cached))
            return cached;
        var result = spellCheck.Check(word);
        _spellCache[word] = result;
        return result;
    }

    private void RenderSpellCheckUnderlines(DrawingContext ctx)
    {
        foreach (var (x, width, y) in _misspelledUnderlines)
        {
            SpellCheckRendering.DrawWavyUnderline(ctx, x, y, width);
        }
    }

    /// <summary>
    /// Handles right-click spell check context menu for non-link words.
    /// Returns true if a spell check menu was shown (caller should skip further handling).
    /// </summary>
    private bool TryShowSpellCheckContextMenu(int charIndex)
    {
        if (!IsSpellCheckEnabled) return false;

        var spellCheck = HostServices.SpellCheck;
        if (spellCheck == null || !spellCheck.IsLoaded) return false;

        var text = _doc.Text;
        var wordInfo = SpellCheckContextMenuHelper.GetWordAtCaret(text, charIndex);
        if (wordInfo == null) return false;

        // Don't show spell menu for code spans
        if (charIndex < _doc.Length && _doc.GetStyleAt(charIndex).HasFlag(InlineStyle.Code))
            return false;

        var menu = new ContextMenu();
        SpellCheckContextMenuHelper.PopulateContextMenu(
            menu,
            wordInfo,
            replacement =>
            {
                _doc.PushUndo();
                _doc.Delete(wordInfo.Start, wordInfo.End - wordInfo.Start);
                _doc.Insert(wordInfo.Start, replacement, InlineStyle.None);
                _caret.Position = wordInfo.Start + replacement.Length;
                _caret.ClearSelection();
                OnContentChanged();
            },
            () =>
            {
                spellCheck.AddToUserDictionary(wordInfo.Word);
            });

        menu.Open(this);
        return true;
    }

    /// <summary>
    /// Performs autocorrect on the word before the caret when a boundary char is typed.
    /// Call after inserting a space/punctuation character.
    /// </summary>
    private void TryAutoCorrect()
    {
        if (!AutoCorrectService.Instance.IsEnabled) return;

        var text = _doc.Text;
        var caret = _caret.Position;
        if (caret < 2) return;

        // The boundary character is at caret - 1
        var boundaryChar = text[caret - 1];
        if (!IsAutoCorrectBoundary(boundaryChar)) return;

        // Extract the word before the boundary
        var wordEnd = caret - 1;
        var wordStart = wordEnd;
        while (wordStart > 0 && SpellCheckContextMenuHelper.IsWordChar(text[wordStart - 1]))
            wordStart--;

        if (wordStart >= wordEnd) return;

        var word = text[wordStart..wordEnd];
        var correction = AutoCorrectService.Instance.GetCorrection(word);
        if (correction == null) return;

        // Replace in-place
        _doc.Delete(wordStart, word.Length);
        _doc.Insert(wordStart, correction, InlineStyle.None);

        // Adjust caret position
        var diff = correction.Length - word.Length;
        _caret.Position += diff;
    }

    private static bool IsAutoCorrectBoundary(char c) =>
        c == ' ' || c == '.' || c == ',' || c == ';' || c == ':' ||
        c == '!' || c == '?' || c == ')' || c == ']' || c == '\n';
}
