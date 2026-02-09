// ============================================================================
// File: TextLayoutEngine.cs
// Description: Measures styled text spans, performs word-wrapping, and provides
//              hit testing and caret/selection geometry for RichTextEditor.
// ============================================================================

using Avalonia;
using Avalonia.Media;

namespace PrivStack.UI.Adaptive.Controls.RichTextEditor;

/// <summary>
/// A single measured text run within a layout line.
/// </summary>
public sealed class GlyphRun
{
    public double X { get; set; }
    public double Width { get; set; }
    public int StartIndex { get; set; }
    public int Length { get; set; }
    public InlineStyle Style { get; set; }
    public FormattedText? FormattedText { get; set; }
    public Typeface Typeface { get; set; }
    public double RunFontSize { get; set; }
    public double YOffset { get; set; }
    public TextColor FgColor { get; set; }
    public TextColor BgColor { get; set; }
    public string? LinkUrl { get; set; }
}

/// <summary>
/// A horizontal line of glyph runs.
/// </summary>
public sealed class LayoutLine
{
    public double Y { get; set; }
    public double Height { get; set; }
    public double Baseline { get; set; }
    public List<GlyphRun> Runs { get; } = [];
}

/// <summary>
/// Word-wrapping layout engine. Call Layout() after setting properties,
/// then use hit testing and geometry queries.
/// </summary>
public sealed class TextLayoutEngine
{
    private readonly List<LayoutLine> _lines = [];

    public double MaxWidth { get; set; } = double.MaxValue;
    public FontFamily BaseFont { get; set; } = FontFamily.Default;
    public FontFamily MonoFont { get; set; } = FontFamily.Default;
    public double FontSize { get; set; } = 14;
    public FontWeight BaseFontWeight { get; set; } = FontWeight.Normal;
    public FontStyle BaseFontStyle { get; set; } = FontStyle.Normal;
    public double TotalHeight { get; private set; }
    public double ContentWidth { get; private set; }
    public IReadOnlyList<LayoutLine> Lines => _lines;

    private IReadOnlyList<StyledSpan> _spans = [];
    private string _fullText = "";

    public void SetContent(IReadOnlyList<StyledSpan> spans, string fullText)
    {
        _spans = spans;
        _fullText = fullText;
    }

    public void Layout()
    {
        _lines.Clear();
        TotalHeight = 0;
        ContentWidth = 0;

        if (_spans.Count == 0 || _fullText.Length == 0)
        {
            // Even empty documents need one line for caret placement
            var emptyLine = new LayoutLine { Y = 0, Height = FontSize * 1.6, Baseline = FontSize };
            _lines.Add(emptyLine);
            TotalHeight = emptyLine.Height;
            return;
        }

        // Build word-level chunks with style info
        var chunks = BuildChunks();
        var lineHeight = FontSize * 1.6;
        var baseline = FontSize;

        var currentLine = new LayoutLine { Y = 0, Height = lineHeight, Baseline = baseline };
        var x = 0.0;

        foreach (var chunk in chunks)
        {
            // Forced line break on newline characters
            if (chunk.Text == "\n")
            {
                // Add a zero-width run so the caret index is tracked
                currentLine.Runs.Add(new GlyphRun
                {
                    X = x,
                    Width = 0,
                    StartIndex = chunk.StartIndex,
                    Length = chunk.Length,
                    Style = chunk.Style,
                    FormattedText = null,
                });
                _lines.Add(currentLine);
                currentLine = new LayoutLine
                {
                    Y = currentLine.Y + lineHeight,
                    Height = lineHeight,
                    Baseline = baseline,
                };
                x = 0;
                continue;
            }

            var (ft, typeface) = CreateFormattedText(chunk.Text, chunk.Style);
            // FormattedText.Width trims trailing whitespace, so spaces measure as 0.
            // Measure space width by embedding between two characters to avoid
            // kerning artifacts and trailing-whitespace trimming across platforms.
            var w = ft.Width;
            if (IsWhitespace(chunk.Text) && chunk.Text.Length > 0)
            {
                var (withSpaces, _) = CreateFormattedText("M" + chunk.Text + "M", chunk.Style);
                var (noSpaces, _) = CreateFormattedText("MM", chunk.Style);
                w = withSpaces.Width - noSpaces.Width;
            }

            // Word wrap: if adding this chunk exceeds max width and line isn't empty
            if (x + w > MaxWidth && currentLine.Runs.Count > 0 && !IsWhitespace(chunk.Text))
            {
                _lines.Add(currentLine);
                currentLine = new LayoutLine
                {
                    Y = currentLine.Y + lineHeight,
                    Height = lineHeight,
                    Baseline = baseline,
                };
                x = 0;
            }

            // Skip leading whitespace on a new line
            if (x == 0 && currentLine.Runs.Count == 0 && IsWhitespace(chunk.Text))
            {
                // Still need to account for the character indices
                // but don't render leading space on wrapped lines
                if (_lines.Count > 0) // only skip on wrapped lines, not first line
                {
                    currentLine.Runs.Add(new GlyphRun
                    {
                        X = 0,
                        Width = 0,
                        StartIndex = chunk.StartIndex,
                        Length = chunk.Length,
                        Style = chunk.Style,
                        FormattedText = null,
                    });
                    continue;
                }
            }

            // Add right padding for emoji runs so they don't feel cramped
            if (IsEmojiOnly(chunk.Text))
                w += 2;

            var isSuperscript = chunk.Style.HasFlag(InlineStyle.Superscript);
            var runFontSize = isSuperscript ? FontSize * 0.7 : FontSize;
            var yOffset = isSuperscript ? -(FontSize * 0.3) : 0.0;

            currentLine.Runs.Add(new GlyphRun
            {
                X = x,
                Width = w,
                StartIndex = chunk.StartIndex,
                Length = chunk.Length,
                Style = chunk.Style,
                FormattedText = ft,
                Typeface = typeface,
                RunFontSize = runFontSize,
                YOffset = yOffset,
                FgColor = chunk.FgColor,
                BgColor = chunk.BgColor,
                LinkUrl = chunk.LinkUrl,
            });
            x += w;
        }

        _lines.Add(currentLine);
        TotalHeight = _lines.Count * lineHeight;
        ContentWidth = 0;
        foreach (var line in _lines)
        {
            var lineWidth = 0.0;
            foreach (var run in line.Runs)
                lineWidth = run.X + run.Width;
            if (lineWidth > ContentWidth)
                ContentWidth = lineWidth;
        }
    }

    /// <summary>
    /// Returns the character index closest to the given point.
    /// </summary>
    public int HitTest(Point point)
    {
        if (_lines.Count == 0) return 0;

        // Find line by Y
        var line = _lines[^1];
        for (var i = 0; i < _lines.Count; i++)
        {
            if (point.Y < _lines[i].Y + _lines[i].Height)
            {
                line = _lines[i];
                break;
            }
        }

        // Find run by X
        if (line.Runs.Count == 0)
            return 0;

        foreach (var run in line.Runs)
        {
            if (run.Width == 0) continue;
            if (point.X < run.X + run.Width)
            {
                // Proportional estimation within the run
                var relX = point.X - run.X;
                var charWidth = run.Length > 0 ? run.Width / run.Length : run.Width;
                var charOffset = (int)Math.Round(relX / charWidth);
                return run.StartIndex + Math.Clamp(charOffset, 0, run.Length);
            }
        }

        // Past end of line → return end of last run
        var lastRun = line.Runs[^1];
        return lastRun.StartIndex + lastRun.Length;
    }

    /// <summary>
    /// Returns the caret rectangle for a given character index.
    /// </summary>
    public Rect GetCaretRect(int charIndex)
    {
        const double caretWidth = 1.5;

        if (_lines.Count == 0)
            return new Rect(0, 0, caretWidth, FontSize * 1.6);

        foreach (var line in _lines)
        {
            foreach (var run in line.Runs)
            {
                if (charIndex >= run.StartIndex && charIndex <= run.StartIndex + run.Length)
                {
                    var offsetInRun = charIndex - run.StartIndex;
                    double x;
                    if (run.FormattedText != null && offsetInRun > 0 && offsetInRun < run.Length)
                    {
                        // Measure substring for precise positioning.
                        // Embed between M chars to avoid trailing-whitespace trimming
                        // and kerning artifacts with sentinel characters.
                        var sub = _fullText.Substring(run.StartIndex, offsetInRun);
                        var (withSub, _) = CreateFormattedText("M" + sub + "M", run.Style);
                        var (noSub, _2) = CreateFormattedText("MM", run.Style);
                        x = run.X + (withSub.Width - noSub.Width);
                    }
                    else
                    {
                        var charWidth = run.Length > 0 ? run.Width / run.Length : 0;
                        x = run.X + offsetInRun * charWidth;
                    }
                    return new Rect(x, line.Y, caretWidth, line.Height);
                }
            }
        }

        // Fallback: end of last line
        var lastLine = _lines[^1];
        var lastX = 0.0;
        if (lastLine.Runs.Count > 0)
        {
            var lr = lastLine.Runs[^1];
            lastX = lr.X + lr.Width;
        }
        return new Rect(lastX, lastLine.Y, caretWidth, lastLine.Height);
    }

    /// <summary>
    /// Returns rectangles covering the selected range for highlight rendering.
    /// </summary>
    public List<Rect> GetSelectionRects(int start, int end)
    {
        var rects = new List<Rect>();
        if (start >= end || _lines.Count == 0) return rects;

        foreach (var line in _lines)
        {
            if (line.Runs.Count == 0) continue;

            var lineStart = line.Runs[0].StartIndex;
            var lastRun = line.Runs[^1];
            var lineEnd = lastRun.StartIndex + lastRun.Length;

            if (end <= lineStart || start >= lineEnd) continue;

            var selStart = Math.Max(start, lineStart);
            var selEnd = Math.Min(end, lineEnd);

            // Compute X positions from runs directly to avoid caret ambiguity
            // at line boundaries (where lineEnd == nextLineStart).
            var startX = GetXInLine(line, selStart);
            var endX = GetXInLine(line, selEnd);

            // For fully-selected lines, extend to the full line width
            if (selEnd == lineEnd && selStart == lineStart)
                endX = Math.Max(endX, MaxWidth);
            else if (selEnd == lineEnd)
                endX = Math.Max(endX, lastRun.X + lastRun.Width);

            var width = Math.Max(endX - startX, 0);
            if (width > 0)
                rects.Add(new Rect(startX, line.Y, width, line.Height));
        }

        return rects;
    }

    /// <summary>Get the X position of a character index within a specific line's runs.</summary>
    private static double GetXInLine(LayoutLine line, int charIndex)
    {
        foreach (var run in line.Runs)
        {
            if (charIndex >= run.StartIndex && charIndex <= run.StartIndex + run.Length)
            {
                var offset = charIndex - run.StartIndex;
                var charWidth = run.Length > 0 ? run.Width / run.Length : 0;
                return run.X + offset * charWidth;
            }
        }
        // Past end of line
        if (line.Runs.Count > 0)
        {
            var lr = line.Runs[^1];
            return lr.X + lr.Width;
        }
        return 0;
    }

    /// <summary>
    /// Get the line index containing the given character index.
    /// </summary>
    public int GetLineIndex(int charIndex)
    {
        for (var i = 0; i < _lines.Count; i++)
        {
            var line = _lines[i];
            if (line.Runs.Count == 0) continue;
            var lineEnd = line.Runs[^1].StartIndex + line.Runs[^1].Length;
            if (charIndex <= lineEnd) return i;
        }
        return _lines.Count - 1;
    }

    /// <summary>
    /// Get the character index at the start of a given line.
    /// </summary>
    public int GetLineStart(int lineIndex)
    {
        if (lineIndex < 0 || lineIndex >= _lines.Count || _lines[lineIndex].Runs.Count == 0)
            return 0;
        return _lines[lineIndex].Runs[0].StartIndex;
    }

    /// <summary>
    /// Get the character index at the end of a given line.
    /// </summary>
    public int GetLineEnd(int lineIndex)
    {
        if (lineIndex < 0 || lineIndex >= _lines.Count || _lines[lineIndex].Runs.Count == 0)
            return 0;
        var lastRun = _lines[lineIndex].Runs[^1];
        return lastRun.StartIndex + lastRun.Length;
    }

#pragma warning disable CS0618 // FormattedText constructor is obsolete in some Avalonia versions
    /// <summary>
    /// Returns true if the text consists entirely of emoji / symbol codepoints
    /// (emoji modifiers, variation selectors, ZWJ sequences, etc.).
    /// </summary>
    private static bool IsEmojiOnly(string text)
    {
        if (string.IsNullOrEmpty(text)) return false;
        for (var i = 0; i < text.Length;)
        {
            // Guard against broken surrogate pairs (e.g. partial emoji delete)
            if (char.IsHighSurrogate(text[i]))
            {
                if (i + 1 >= text.Length || !char.IsLowSurrogate(text[i + 1]))
                    return false;
            }
            else if (char.IsLowSurrogate(text[i]))
                return false;

            var cp = char.ConvertToUtf32(text, i);
            i += char.IsHighSurrogate(text[i]) ? 2 : 1;
            // Allow ZWJ, variation selectors, combining marks, skin-tone modifiers
            if (cp == 0x200D || (cp >= 0xFE00 && cp <= 0xFE0F) || cp == 0x20E3
                || (cp >= 0x1F3FB && cp <= 0x1F3FF)) continue;
            // Common emoji ranges
            if (cp >= 0x1F600 && cp <= 0x1FAFF) continue; // emoticons, symbols, etc.
            if (cp >= 0x2600 && cp <= 0x27BF) continue;   // misc symbols, dingbats
            if (cp >= 0x2300 && cp <= 0x23FF) continue;   // misc technical
            if (cp >= 0x2B50 && cp <= 0x2B55) continue;   // stars, circles
            if (cp >= 0x1F900 && cp <= 0x1F9FF) continue; // supplemental symbols
            if (cp >= 0x1FA00 && cp <= 0x1FA6F) continue; // chess, extended-A
            if (cp >= 0x1FA70 && cp <= 0x1FAFF) continue; // extended-A cont.
            if (cp >= 0xE0020 && cp <= 0xE007F) continue; // tag sequences (flags)
            if (cp == 0xA9 || cp == 0xAE) continue;       // © ®
            if (cp >= 0x200B && cp <= 0x200F) continue;    // zero-width chars
            return false;
        }
        return text.Length > 0;
    }

    private (FormattedText Ft, Typeface Typeface) CreateFormattedText(string text, InlineStyle style)
    {
        var isCode = style.HasFlag(InlineStyle.Code);
        var isBold = style.HasFlag(InlineStyle.Bold);
        var isItalic = style.HasFlag(InlineStyle.Italic);
        var isSuperscript = style.HasFlag(InlineStyle.Superscript);
        var isEmoji = IsEmojiOnly(text);

        // Emojis always render with normal weight/style — no bold/italic/superscript
        var weight = isEmoji ? FontWeight.Normal
            : isBold ? FontWeight.Bold
            : BaseFontWeight;
        var fontStyle = isEmoji ? FontStyle.Normal
            : isItalic ? FontStyle.Italic : BaseFontStyle;
        var family = isCode ? MonoFont : BaseFont;
        var fontSize = (isSuperscript && !isEmoji) ? FontSize * 0.7 : FontSize;

        var typeface = new Typeface(family, fontStyle, weight);

        var ft = new FormattedText(
            text,
            System.Globalization.CultureInfo.CurrentCulture,
            FlowDirection.LeftToRight,
            typeface,
            fontSize,
            Brushes.White); // Brush doesn't matter for measurement
        return (ft, typeface);
    }
#pragma warning restore CS0618

    // ---- Chunking ----

    private readonly record struct TextChunk(string Text, InlineStyle Style, int StartIndex, int Length,
        TextColor FgColor = TextColor.Default, TextColor BgColor = TextColor.Default, string? LinkUrl = null);

    /// <summary>
    /// Splits spans into word-level chunks at whitespace boundaries.
    /// Newline characters are emitted as individual chunks for forced line breaks.
    /// </summary>
    private List<TextChunk> BuildChunks()
    {
        var chunks = new List<TextChunk>();
        var globalIndex = 0;

        foreach (var span in _spans)
        {
            var start = 0;
            for (var i = 0; i <= span.Text.Length; i++)
            {
                var atEnd = i == span.Text.Length;

                // Force-split on newline characters
                if (!atEnd && span.Text[i] == '\n')
                {
                    if (i > start)
                    {
                        var pre = span.Text[start..i];
                        chunks.Add(new TextChunk(pre, span.Style, globalIndex + start, pre.Length, span.FgColor, span.BgColor, span.LinkUrl));
                    }
                    chunks.Add(new TextChunk("\n", span.Style, globalIndex + i, 1, span.FgColor, span.BgColor, span.LinkUrl));
                    start = i + 1;
                    continue;
                }

                // Split at whitespace boundaries
                var isSpace = !atEnd && char.IsWhiteSpace(span.Text[i]);
                var wasSpace = i > start && start < span.Text.Length && char.IsWhiteSpace(span.Text[start]);

                if (atEnd || (isSpace != wasSpace && i > start))
                {
                    var chunkText = span.Text[start..i];
                    chunks.Add(new TextChunk(chunkText, span.Style, globalIndex + start, chunkText.Length, span.FgColor, span.BgColor, span.LinkUrl));
                    start = i;
                }
            }
            globalIndex += span.Text.Length;
        }

        return chunks;
    }

    private static bool IsWhitespace(string text) =>
        text.Length > 0 && text.All(char.IsWhiteSpace);
}
