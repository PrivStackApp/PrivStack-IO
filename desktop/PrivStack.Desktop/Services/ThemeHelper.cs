using Avalonia;
using Avalonia.Media;

namespace PrivStack.Desktop.Services;

/// <summary>
/// Helper class for retrieving theme colors from application resources.
/// </summary>
public static class ThemeHelper
{
    /// <summary>
    /// Gets a color from the current theme resources.
    /// </summary>
    public static Color GetColor(string resourceKey, Color fallback)
    {
        if (Application.Current?.TryGetResource(resourceKey, Application.Current.ActualThemeVariant, out var resource) == true)
        {
            if (resource is Color color)
                return color;
        }
        return fallback;
    }

    /// <summary>
    /// Gets a brush from the current theme resources.
    /// </summary>
    public static IBrush GetBrush(string resourceKey, IBrush fallback)
    {
        if (Application.Current?.TryGetResource(resourceKey, Application.Current.ActualThemeVariant, out var resource) == true)
        {
            if (resource is IBrush brush)
                return brush;
        }
        return fallback;
    }

    /// <summary>
    /// Creates a SolidColorBrush from a theme color resource.
    /// </summary>
    public static SolidColorBrush GetSolidColorBrush(string resourceKey, string fallbackHex)
    {
        var color = GetColor(resourceKey, Color.Parse(fallbackHex));
        return new SolidColorBrush(color);
    }

    // Common theme color accessors
    public static Color Primary => GetColor("ThemePrimary", Color.Parse("#00D4FF"));
    public static Color PrimaryHover => GetColor("ThemePrimaryHover", Color.Parse("#00A8CC"));
    public static Color Secondary => GetColor("ThemeSecondary", Color.Parse("#8B5CF6"));
    public static Color SecondaryHover => GetColor("ThemeSecondaryHover", Color.Parse("#7C3AED"));
    public static Color Success => GetColor("ThemeSuccess", Color.Parse("#10B981"));
    public static Color Warning => GetColor("ThemeWarning", Color.Parse("#FFB800"));
    public static Color Danger => GetColor("ThemeDanger", Color.Parse("#EF4444"));
    public static Color TextPrimary => GetColor("ThemeTextPrimary", Color.Parse("#FFFFFF"));
    public static Color TextSecondary => GetColor("ThemeTextSecondary", Color.Parse("#A0A0B0"));
    public static Color TextMuted => GetColor("ThemeTextMuted", Color.Parse("#6B6B7B"));
    public static Color Background => GetColor("ThemeBackground", Color.Parse("#0A0A0F"));
    public static Color Surface => GetColor("ThemeSurface", Color.Parse("#12121A"));
    public static Color SurfaceElevated => GetColor("ThemeSurfaceElevated", Color.Parse("#1A1A24"));
    public static Color Border => GetColor("ThemeBorder", Color.Parse("#1E1E2E"));
    public static Color Hover => GetColor("ThemeHover", Color.Parse("#252536"));
    public static Color Selected => GetColor("ThemeSelected", Color.Parse("#1E1E2E"));
}
