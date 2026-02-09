// ============================================================================
// File: InlineMarkdownParser.cs
// Description: Round-trips between inline markdown and TextDocument.
//              Parse: single-pass state machine tracking active marks.
//              Serialize: groups by style, wraps with markdown markers.
// ============================================================================

using System.Text;

namespace PrivStack.UI.Adaptive.Controls.RichTextEditor;

public static class InlineMarkdownParser
{
    private static readonly Dictionary<string, TextColor> ColorNameMap = new(StringComparer.OrdinalIgnoreCase)
    {
        ["default"] = TextColor.Default,
        ["gray"] = TextColor.Gray,
        ["brown"] = TextColor.Brown,
        ["orange"] = TextColor.Orange,
        ["yellow"] = TextColor.Yellow,
        ["green"] = TextColor.Green,
        ["blue"] = TextColor.Blue,
        ["purple"] = TextColor.Purple,
        ["pink"] = TextColor.Pink,
        ["red"] = TextColor.Red,
    };

    private static readonly Dictionary<TextColor, string> ColorToName = new()
    {
        [TextColor.Gray] = "gray",
        [TextColor.Brown] = "brown",
        [TextColor.Orange] = "orange",
        [TextColor.Yellow] = "yellow",
        [TextColor.Green] = "green",
        [TextColor.Blue] = "blue",
        [TextColor.Purple] = "purple",
        [TextColor.Pink] = "pink",
        [TextColor.Red] = "red",
    };

    /// <summary>
    /// Parse inline markdown into a TextDocument. Handles **, *, `, ~~, &lt;u&gt;, &lt;sup&gt;,
    /// &lt;span style="color:X"&gt;, &lt;mark style="background:X"&gt;.
    /// </summary>
    public static TextDocument Parse(string markdown)
    {
        if (string.IsNullOrEmpty(markdown))
            return new TextDocument();

        var text = new StringBuilder(markdown.Length);
        var styles = new List<InlineStyle>(markdown.Length);
        var fgColors = new List<TextColor>(markdown.Length);
        var bgColors = new List<TextColor>(markdown.Length);
        var linkUrls = new List<string?>(markdown.Length);
        var active = InlineStyle.None;
        var activeFg = TextColor.Default;
        var activeBg = TextColor.Default;

        // Stacks for nested color tags
        var fgStack = new Stack<TextColor>();
        var bgStack = new Stack<TextColor>();

        var i = 0;

        while (i < markdown.Length)
        {
            // ~~strikethrough~~
            if (i + 1 < markdown.Length && markdown[i] == '~' && markdown[i + 1] == '~')
            {
                active ^= InlineStyle.Strikethrough;
                i += 2;
                continue;
            }

            // ** or *** (bold, bold+italic)
            if (i + 1 < markdown.Length && markdown[i] == '*' && markdown[i + 1] == '*')
            {
                if (i + 2 < markdown.Length && markdown[i + 2] == '*')
                {
                    active ^= InlineStyle.Bold | InlineStyle.Italic;
                    i += 3;
                    continue;
                }
                active ^= InlineStyle.Bold;
                i += 2;
                continue;
            }

            // * italic (single asterisk, not followed by another)
            if (markdown[i] == '*')
            {
                active ^= InlineStyle.Italic;
                i += 1;
                continue;
            }

            // HTML-style tags
            if (markdown[i] == '<')
            {
                // <sup>
                if (TryMatch(markdown, i, "<sup>"))
                {
                    active |= InlineStyle.Superscript;
                    i += 5;
                    continue;
                }
                // </sup>
                if (TryMatch(markdown, i, "</sup>"))
                {
                    active &= ~InlineStyle.Superscript;
                    i += 6;
                    continue;
                }
                // <u>
                if (TryMatch(markdown, i, "<u>"))
                {
                    active ^= InlineStyle.Underline;
                    i += 3;
                    continue;
                }
                // </u>
                if (TryMatch(markdown, i, "</u>"))
                {
                    active ^= InlineStyle.Underline;
                    i += 4;
                    continue;
                }
                // <span style="color:X">
                if (TryParseColorTag(markdown, i, "span", "color", out var fgColor, out var tagLen))
                {
                    fgStack.Push(activeFg);
                    activeFg = fgColor;
                    i += tagLen;
                    continue;
                }
                // </span>
                if (TryMatch(markdown, i, "</span>"))
                {
                    activeFg = fgStack.Count > 0 ? fgStack.Pop() : TextColor.Default;
                    i += 7;
                    continue;
                }
                // <mark style="background:X">
                if (TryParseColorTag(markdown, i, "mark", "background", out var bgColor, out var bgTagLen))
                {
                    bgStack.Push(activeBg);
                    activeBg = bgColor;
                    i += bgTagLen;
                    continue;
                }
                // </mark>
                if (TryMatch(markdown, i, "</mark>"))
                {
                    activeBg = bgStack.Count > 0 ? bgStack.Pop() : TextColor.Default;
                    i += 7;
                    continue;
                }
            }

            // `code`
            if (markdown[i] == '`')
            {
                active ^= InlineStyle.Code;
                i += 1;
                continue;
            }

            // [text](url) — markdown link
            if (markdown[i] == '[')
            {
                var closeB = markdown.IndexOf(']', i + 1);
                if (closeB > i && closeB + 1 < markdown.Length && markdown[closeB + 1] == '(')
                {
                    var closeP = markdown.IndexOf(')', closeB + 2);
                    if (closeP > closeB + 1)
                    {
                        var linkText = markdown[(i + 1)..closeB];
                        var linkUrl = markdown[(closeB + 2)..closeP];
                        foreach (var ch in linkText)
                        {
                            text.Append(ch);
                            styles.Add(active | InlineStyle.Link);
                            fgColors.Add(activeFg);
                            bgColors.Add(activeBg);
                            linkUrls.Add(linkUrl);
                        }
                        i = closeP + 1;
                        continue;
                    }
                }
            }

            // Regular character
            text.Append(markdown[i]);
            styles.Add(active);
            fgColors.Add(activeFg);
            bgColors.Add(activeBg);
            linkUrls.Add(null);
            i++;
        }

        return new TextDocument(text.ToString(), styles.ToArray(), fgColors.ToArray(), bgColors.ToArray(), linkUrls.ToArray());
    }

    /// <summary>
    /// Serialize a TextDocument back to inline markdown.
    /// </summary>
    public static string Serialize(TextDocument doc)
    {
        var spans = doc.GetSpans();
        if (spans.Count == 0)
            return "";

        var sb = new StringBuilder();
        foreach (var span in spans)
        {
            var prefix = new StringBuilder();
            var suffix = new StringBuilder();

            // Link wraps outermost — emit [prefix...text...suffix](url)
            var isLink = span.Style.HasFlag(InlineStyle.Link) && !string.IsNullOrEmpty(span.LinkUrl);

            // Fg color wraps everything (inside link wrapper)
            if (span.FgColor != TextColor.Default && ColorToName.TryGetValue(span.FgColor, out var fgName))
            {
                prefix.Append($"<span style=\"color:{fgName}\">");
                suffix.Insert(0, "</span>");
            }

            // Bg color
            if (span.BgColor != TextColor.Default && ColorToName.TryGetValue(span.BgColor, out var bgName))
            {
                prefix.Append($"<mark style=\"background:{bgName}\">");
                suffix.Insert(0, "</mark>");
            }

            // Superscript
            if (span.Style.HasFlag(InlineStyle.Superscript))
            {
                prefix.Append("<sup>");
                suffix.Insert(0, "</sup>");
            }

            // Order: underline wraps strikethrough wraps bold wraps italic wraps code
            if (span.Style.HasFlag(InlineStyle.Underline))
            {
                prefix.Append("<u>");
                suffix.Insert(0, "</u>");
            }

            if (span.Style.HasFlag(InlineStyle.Strikethrough))
            {
                prefix.Append("~~");
                suffix.Insert(0, "~~");
            }

            var hasBold = span.Style.HasFlag(InlineStyle.Bold);
            var hasItalic = span.Style.HasFlag(InlineStyle.Italic);

            if (hasBold && hasItalic)
            {
                prefix.Append("***");
                suffix.Insert(0, "***");
            }
            else
            {
                if (hasBold)
                {
                    prefix.Append("**");
                    suffix.Insert(0, "**");
                }
                if (hasItalic)
                {
                    prefix.Append('*');
                    suffix.Insert(0, "*");
                }
            }

            if (span.Style.HasFlag(InlineStyle.Code))
            {
                prefix.Append('`');
                suffix.Insert(0, "`");
            }

            if (isLink)
            {
                sb.Append('[');
                sb.Append(prefix);
                sb.Append(span.Text);
                sb.Append(suffix);
                sb.Append("](");
                sb.Append(span.LinkUrl);
                sb.Append(')');
            }
            else
            {
                sb.Append(prefix);
                sb.Append(span.Text);
                sb.Append(suffix);
            }
        }

        return sb.ToString();
    }

    private static bool TryMatch(string s, int pos, string tag)
    {
        if (pos + tag.Length > s.Length) return false;
        return s.AsSpan(pos, tag.Length).Equals(tag.AsSpan(), StringComparison.OrdinalIgnoreCase);
    }

    /// <summary>
    /// Try to parse &lt;tagName style="cssProp:colorName"&gt; at position.
    /// </summary>
    private static bool TryParseColorTag(string s, int pos, string tagName, string cssProp,
        out TextColor color, out int length)
    {
        color = TextColor.Default;
        length = 0;

        // Build expected prefix: <tagName style="cssProp:
        var prefix = $"<{tagName} style=\"{cssProp}:";
        if (!TryMatch(s, pos, prefix)) return false;

        var valueStart = pos + prefix.Length;
        var closeQuote = s.IndexOf('"', valueStart);
        if (closeQuote < 0) return false;

        // Expect "> after the closing quote
        if (closeQuote + 1 >= s.Length || s[closeQuote + 1] != '>') return false;

        var colorName = s[valueStart..closeQuote].Trim();
        if (!ColorNameMap.TryGetValue(colorName, out color)) return false;

        length = closeQuote + 2 - pos;
        return true;
    }
}
