// ============================================================================
// File: TextDocument.cs
// Description: Editable document model for a single block of inline-formatted
//              text. Stores flat text + a parallel style array. Supports insert,
//              delete, and style toggling. Round-trips to/from inline markdown.
// ============================================================================

using System.Text;

namespace PrivStack.UI.Adaptive.Controls.RichTextEditor;

[Flags]
public enum InlineStyle : byte
{
    None = 0,
    Bold = 1,
    Italic = 2,
    Code = 4,
    Strikethrough = 8,
    Underline = 16,
    Superscript = 32,
    Link = 64,
}

public enum TextColor : byte
{
    Default = 0, Gray, Brown, Orange, Yellow, Green, Blue, Purple, Pink, Red
}

/// <summary>
/// A contiguous run of text sharing the same inline style.
/// </summary>
public readonly record struct StyledSpan(string Text, InlineStyle Style, TextColor FgColor = TextColor.Default, TextColor BgColor = TextColor.Default, string? LinkUrl = null);

/// <summary>
/// Flat text buffer with a parallel style array. Every char has an InlineStyle.
/// No newlines — each TextDocument represents a single block.
/// </summary>
public sealed class TextDocument
{
    private readonly StringBuilder _text;
    private readonly List<InlineStyle> _styles;
    private readonly List<TextColor> _fgColors;
    private readonly List<TextColor> _bgColors;
    private readonly List<string?> _linkUrls;

    public TextDocument() : this("", [], [], [], null) { }

    public TextDocument(string text, InlineStyle[] styles,
        TextColor[]? fgColors = null, TextColor[]? bgColors = null,
        string?[]? linkUrls = null)
    {
        _text = new StringBuilder(text);
        if (styles.Length == text.Length)
        {
            _styles = [.. styles];
        }
        else
        {
            _styles = new List<InlineStyle>(text.Length);
            for (var i = 0; i < text.Length; i++)
                _styles.Add(i < styles.Length ? styles[i] : InlineStyle.None);
        }

        _fgColors = new List<TextColor>(text.Length);
        _bgColors = new List<TextColor>(text.Length);
        _linkUrls = new List<string?>(text.Length);
        for (var i = 0; i < text.Length; i++)
        {
            _fgColors.Add(fgColors != null && i < fgColors.Length ? fgColors[i] : TextColor.Default);
            _bgColors.Add(bgColors != null && i < bgColors.Length ? bgColors[i] : TextColor.Default);
            _linkUrls.Add(linkUrls != null && i < linkUrls.Length ? linkUrls[i] : null);
        }
    }

    public int Length => _text.Length;
    public string Text => _text.ToString();

    public InlineStyle GetStyleAt(int index) =>
        index >= 0 && index < _styles.Count ? _styles[index] : InlineStyle.None;

    public TextColor GetFgColorAt(int index) =>
        index >= 0 && index < _fgColors.Count ? _fgColors[index] : TextColor.Default;

    public TextColor GetBgColorAt(int index) =>
        index >= 0 && index < _bgColors.Count ? _bgColors[index] : TextColor.Default;

    public string? GetLinkUrlAt(int index) =>
        index >= 0 && index < _linkUrls.Count ? _linkUrls[index] : null;

    /// <summary>Returns the set of distinct non-null link URLs in this document.</summary>
    public HashSet<string> GetAllLinkUrls()
    {
        var urls = new HashSet<string>();
        foreach (var url in _linkUrls)
        {
            if (url != null)
                urls.Add(url);
        }
        return urls;
    }

    public void SetLinkUrl(int start, int count, string? url)
    {
        if (count <= 0) return;
        start = Math.Max(0, start);
        var end = Math.Min(start + count, _linkUrls.Count);
        for (var i = start; i < end; i++)
        {
            _linkUrls[i] = url;
            if (url != null)
                _styles[i] |= InlineStyle.Link;
            else
                _styles[i] &= ~InlineStyle.Link;
        }
    }

    public void Insert(int position, string text, InlineStyle style,
        TextColor fgColor = TextColor.Default, TextColor bgColor = TextColor.Default,
        string? linkUrl = null)
    {
        position = Math.Clamp(position, 0, _text.Length);
        _text.Insert(position, text);
        for (var i = 0; i < text.Length; i++)
        {
            _styles.Insert(position + i, style);
            _fgColors.Insert(position + i, fgColor);
            _bgColors.Insert(position + i, bgColor);
            _linkUrls.Insert(position + i, linkUrl);
        }
    }

    public void Delete(int start, int count)
    {
        if (count <= 0 || start < 0 || start >= _text.Length) return;
        count = Math.Min(count, _text.Length - start);
        _text.Remove(start, count);
        _styles.RemoveRange(start, count);
        _fgColors.RemoveRange(start, count);
        _bgColors.RemoveRange(start, count);
        _linkUrls.RemoveRange(start, count);
    }

    public void SetFgColor(int start, int count, TextColor color)
    {
        if (count <= 0) return;
        start = Math.Max(0, start);
        var end = Math.Min(start + count, _fgColors.Count);
        for (var i = start; i < end; i++)
            _fgColors[i] = color;
    }

    public void SetBgColor(int start, int count, TextColor color)
    {
        if (count <= 0) return;
        start = Math.Max(0, start);
        var end = Math.Min(start + count, _bgColors.Count);
        for (var i = start; i < end; i++)
            _bgColors[i] = color;
    }

    public void ToggleStyle(int start, int count, InlineStyle flag)
    {
        if (count <= 0) return;
        start = Math.Max(0, start);
        var end = Math.Min(start + count, _styles.Count);

        // If all chars in range have the flag, remove it; otherwise add it
        var allHave = true;
        for (var i = start; i < end; i++)
        {
            if ((_styles[i] & flag) == 0) { allHave = false; break; }
        }

        for (var i = start; i < end; i++)
        {
            _styles[i] = allHave ? (_styles[i] & ~flag) : (_styles[i] | flag);
        }
    }

    /// <summary>
    /// Groups consecutive chars with the same style into spans.
    /// </summary>
    public IReadOnlyList<StyledSpan> GetSpans()
    {
        if (_text.Length == 0)
            return [];

        var spans = new List<StyledSpan>();
        var start = 0;
        var currentStyle = _styles[0];
        var currentFg = _fgColors[0];
        var currentBg = _bgColors[0];
        var currentLink = _linkUrls[0];

        for (var i = 1; i < _text.Length; i++)
        {
            if (_styles[i] != currentStyle || _fgColors[i] != currentFg || _bgColors[i] != currentBg || _linkUrls[i] != currentLink)
            {
                spans.Add(new StyledSpan(_text.ToString(start, i - start), currentStyle, currentFg, currentBg, currentLink));
                start = i;
                currentStyle = _styles[i];
                currentFg = _fgColors[i];
                currentBg = _bgColors[i];
                currentLink = _linkUrls[i];
            }
        }

        spans.Add(new StyledSpan(_text.ToString(start, _text.Length - start), currentStyle, currentFg, currentBg, currentLink));
        return spans;
    }

    /// <summary>
    /// Append the contents of another document to the end of this one.
    /// </summary>
    public void Append(TextDocument other)
    {
        _text.Append(other._text);
        _styles.AddRange(other._styles);
        _fgColors.AddRange(other._fgColors);
        _bgColors.AddRange(other._bgColors);
        _linkUrls.AddRange(other._linkUrls);
    }

    /// <summary>
    /// Split this document at position, returning a new document containing
    /// everything from position onward. This document is truncated.
    /// </summary>
    public TextDocument Split(int position)
    {
        position = Math.Clamp(position, 0, _text.Length);
        var afterText = _text.ToString(position, _text.Length - position);
        var afterStyles = _styles.GetRange(position, _styles.Count - position).ToArray();
        var afterFg = _fgColors.GetRange(position, _fgColors.Count - position).ToArray();
        var afterBg = _bgColors.GetRange(position, _bgColors.Count - position).ToArray();
        var afterLinks = _linkUrls.GetRange(position, _linkUrls.Count - position).ToArray();

        _text.Remove(position, _text.Length - position);
        _styles.RemoveRange(position, _styles.Count - position);
        _fgColors.RemoveRange(position, _fgColors.Count - position);
        _bgColors.RemoveRange(position, _bgColors.Count - position);
        _linkUrls.RemoveRange(position, _linkUrls.Count - position);

        return new TextDocument(afterText, afterStyles, afterFg, afterBg, afterLinks);
    }

    // ---- Undo / Redo ----

    private readonly record struct Snapshot(string Text, InlineStyle[] Styles, TextColor[] FgColors, TextColor[] BgColors, string?[] LinkUrls);

    private readonly Stack<Snapshot> _undoStack = new();
    private readonly Stack<Snapshot> _redoStack = new();
    private const int MaxUndoHistory = 100;

    /// <summary>Save current state to undo stack. Call before mutations.</summary>
    public void PushUndo()
    {
        if (_undoStack.Count >= MaxUndoHistory)
        {
            // Convert to array, drop oldest, rebuild
            var arr = _undoStack.ToArray();
            _undoStack.Clear();
            for (var i = Math.Min(arr.Length - 1, MaxUndoHistory - 2); i >= 0; i--)
                _undoStack.Push(arr[i]);
        }
        _undoStack.Push(new Snapshot(
            _text.ToString(),
            [.. _styles],
            [.. _fgColors],
            [.. _bgColors],
            [.. _linkUrls]));
        _redoStack.Clear();
    }

    /// <summary>Undo to previous state. Returns true if state changed.</summary>
    public bool Undo()
    {
        if (_undoStack.Count == 0) return false;

        // Save current state to redo
        _redoStack.Push(new Snapshot(
            _text.ToString(),
            [.. _styles],
            [.. _fgColors],
            [.. _bgColors],
            [.. _linkUrls]));

        var snap = _undoStack.Pop();
        RestoreSnapshot(snap);
        return true;
    }

    /// <summary>Redo previously undone state. Returns true if state changed.</summary>
    public bool Redo()
    {
        if (_redoStack.Count == 0) return false;

        // Save current state to undo (without clearing redo)
        _undoStack.Push(new Snapshot(
            _text.ToString(),
            [.. _styles],
            [.. _fgColors],
            [.. _bgColors],
            [.. _linkUrls]));

        var snap = _redoStack.Pop();
        RestoreSnapshot(snap);
        return true;
    }

    private void RestoreSnapshot(Snapshot snap)
    {
        _text.Clear();
        _text.Append(snap.Text);
        _styles.Clear();
        _styles.AddRange(snap.Styles);
        _fgColors.Clear();
        _fgColors.AddRange(snap.FgColors);
        _bgColors.Clear();
        _bgColors.AddRange(snap.BgColors);
        _linkUrls.Clear();
        _linkUrls.AddRange(snap.LinkUrls);
    }

    public static TextDocument FromMarkdown(string markdown) =>
        InlineMarkdownParser.Parse(markdown);

    public string ToMarkdown() =>
        InlineMarkdownParser.Serialize(this);
}
