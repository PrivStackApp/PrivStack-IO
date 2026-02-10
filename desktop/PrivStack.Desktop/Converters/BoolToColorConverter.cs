using System.Collections.Generic;
using System.Globalization;
using Avalonia;
using Avalonia.Data.Converters;
using Avalonia.Media;
using PrivStack.Desktop.Services;

namespace PrivStack.Desktop.Converters;

/// <summary>
/// Converts a boolean to one of two brushes specified in the ConverterParameter.
/// Parameter format: "TrueResourceKey|FalseResourceKey" (e.g., "ThemeSelected|Transparent")
/// Also supports hex colors for backwards compatibility (e.g., "#1A1A24|#12121A")
/// Theme resource keys can be specified with or without "Brush" suffix.
/// </summary>
public class BoolToColorConverter : IValueConverter
{
    public object? Convert(object? value, Type targetType, object? parameter, CultureInfo culture)
    {
        if (value is not bool boolValue || parameter is not string colorParam)
            return ThemeHelper.GetBrush("ThemeSurfaceElevatedBrush", Brushes.DimGray);

        var colors = colorParam.Split('|');
        if (colors.Length != 2)
            return ThemeHelper.GetBrush("ThemeSurfaceElevatedBrush", Brushes.DimGray);

        var colorKey = boolValue ? colors[0] : colors[1];
        return ResolveBrush(colorKey);
    }

    private static IBrush ResolveBrush(string colorKey)
    {
        // Handle transparent explicitly
        if (string.IsNullOrEmpty(colorKey) || colorKey.Equals("Transparent", StringComparison.OrdinalIgnoreCase))
        {
            return Brushes.Transparent;
        }

        // If it starts with #, it's a hex color
        if (colorKey.StartsWith('#'))
        {
            return new SolidColorBrush(Color.Parse(colorKey));
        }

        // Append "Brush" if not already present for theme resource lookup
        var brushKey = colorKey.EndsWith("Brush", StringComparison.OrdinalIgnoreCase)
            ? colorKey
            : colorKey + "Brush";

        return ThemeHelper.GetBrush(brushKey, Brushes.DimGray);
    }

    public object? ConvertBack(object? value, Type targetType, object? parameter, CultureInfo culture)
    {
        throw new NotImplementedException();
    }
}

/// <summary>
/// Compares two values for equality. Returns true if they are equal.
/// Use with MultiBinding: first value is the item's value, second is the comparison value.
/// </summary>
public class EqualityConverter : IMultiValueConverter
{
    public static readonly EqualityConverter Instance = new();

    public object? Convert(IList<object?> values, Type targetType, object? parameter, CultureInfo culture)
    {
        if (values.Count < 2)
            return false;

        var first = values[0];
        var second = values[1];

        if (first == null && second == null)
            return true;
        if (first == null || second == null)
            return false;

        return first.Equals(second);
    }
}

/// <summary>
/// Converts a link type identifier to a color for the indicator dot.
/// Maps known link types to PrivStack theme colors.
/// </summary>
public class LinkTypeColorConverter : IValueConverter
{
    public object? Convert(object? value, Type targetType, object? parameter, CultureInfo culture)
    {
        var linkType = value as string ?? "";
        var color = linkType switch
        {
            "note" => ThemeHelper.Primary,      // ThemePrimary (cyan)
            "task" => ThemeHelper.Success,      // ThemeSuccess (green)
            "event" => ThemeHelper.Warning,     // ThemeWarning (amber)
            "journal" => ThemeHelper.Secondary, // ThemeSecondary (purple)
            "file" => ThemeHelper.Secondary,    // ThemeSecondary (purple)
            _ => ThemeHelper.TextMuted          // ThemeTextMuted (gray)
        };
        return new SolidColorBrush(color);
    }

    public object? ConvertBack(object? value, Type targetType, object? parameter, CultureInfo culture)
    {
        throw new NotImplementedException();
    }
}

/// <summary>
/// Converts UTC DateTimeOffset to local time for display.
/// Dates are stored as UTC and should be presented in local time.
/// </summary>
public class UtcToLocalConverter : IValueConverter
{
    public static readonly UtcToLocalConverter Instance = new();

    public object? Convert(object? value, Type targetType, object? parameter, CultureInfo culture)
    {
        return value switch
        {
            DateTimeOffset dto => dto.ToLocalTime(),
            DateTime dt when dt.Kind == DateTimeKind.Utc => dt.ToLocalTime(),
            DateTime dt => dt,
            _ => value
        };
    }

    public object? ConvertBack(object? value, Type targetType, object? parameter, CultureInfo culture)
    {
        throw new NotImplementedException();
    }
}
