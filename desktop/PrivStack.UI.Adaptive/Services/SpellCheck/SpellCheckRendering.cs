using Avalonia;
using Avalonia.Media;

namespace PrivStack.UI.Adaptive.Services.SpellCheck;

/// <summary>
/// Shared rendering utility for drawing red wavy (squiggly) underlines
/// beneath misspelled words. Used by both <c>SpellCheckAdorner</c> (TextBox)
/// and <c>RichTextEditor.SpellCheck</c>.
/// </summary>
public static class SpellCheckRendering
{
    private static readonly IPen WavyPen = new Pen(Brushes.Red, 1);

    private const double Amplitude = 1.5;
    private const double Wavelength = 4.0;

    /// <summary>
    /// Draws a red wavy underline from (<paramref name="x"/>, <paramref name="y"/>)
    /// spanning the given <paramref name="width"/>.
    /// </summary>
    public static void DrawWavyUnderline(DrawingContext ctx, double x, double y, double width)
    {
        if (width <= 0) return;

        var geometry = new StreamGeometry();
        using (var gc = geometry.Open())
        {
            gc.BeginFigure(new Point(x, y), false);

            var pos = 0.0;
            var up = true;
            while (pos < width)
            {
                var segWidth = Math.Min(Wavelength / 2, width - pos);
                var endX = x + pos + segWidth;
                var endY = up ? y - Amplitude : y + Amplitude;
                var cpX = x + pos + segWidth / 2;
                var cpY = endY;
                gc.QuadraticBezierTo(new Point(cpX, cpY), new Point(endX, up ? y : y));

                // Alternate to opposite direction for next half-wave
                pos += segWidth;
                up = !up;
            }

            gc.EndFigure(false);
        }

        ctx.DrawGeometry(null, WavyPen, geometry);
    }
}
