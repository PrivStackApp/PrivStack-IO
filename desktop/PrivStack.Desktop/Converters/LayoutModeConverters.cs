using System.Globalization;
using Avalonia.Controls;
using Avalonia.Data.Converters;
using PrivStack.Sdk;

namespace PrivStack.Desktop.Converters;

/// <summary>
/// Converts a <see cref="LayoutMode"/> to a <see cref="bool"/> visibility value.
/// Parameter specifies modes where the element is visible (comma-separated).
/// Example: ConverterParameter="Wide,Normal" → visible in Wide and Normal, hidden in Compact.
/// </summary>
public class LayoutModeToVisibilityConverter : IValueConverter
{
    public object Convert(object? value, Type targetType, object? parameter, CultureInfo culture)
    {
        if (value is not LayoutMode mode || parameter is not string visibleModes)
            return true;

        var modes = visibleModes.Split(',', StringSplitOptions.TrimEntries);
        return modes.Any(m => m.Equals(mode.ToString(), StringComparison.OrdinalIgnoreCase));
    }

    public object ConvertBack(object? value, Type targetType, object? parameter, CultureInfo culture)
        => throw new NotSupportedException();
}

/// <summary>
/// Converts a <see cref="LayoutMode"/> to a <see cref="GridLength"/> value.
/// Parameter format: "Compact=48,Normal=220,Wide=260" or "Compact=0,Normal=*,Wide=*".
/// Supports pixel values (plain numbers), star values ("*", "2*"), and "Auto".
/// </summary>
public class LayoutModeToGridLengthConverter : IValueConverter
{
    public object Convert(object? value, Type targetType, object? parameter, CultureInfo culture)
    {
        if (value is not LayoutMode mode || parameter is not string spec)
            return new GridLength(1, GridUnitType.Star);

        var pairs = spec.Split(',', StringSplitOptions.TrimEntries);
        foreach (var pair in pairs)
        {
            var kv = pair.Split('=', 2, StringSplitOptions.TrimEntries);
            if (kv.Length != 2) continue;

            if (!kv[0].Equals(mode.ToString(), StringComparison.OrdinalIgnoreCase))
                continue;

            return ParseGridLength(kv[1]);
        }

        return new GridLength(1, GridUnitType.Star);
    }

    private static GridLength ParseGridLength(string val)
    {
        if (val.Equals("Auto", StringComparison.OrdinalIgnoreCase))
            return GridLength.Auto;

        if (val.EndsWith('*'))
        {
            var numPart = val[..^1];
            var factor = string.IsNullOrEmpty(numPart) ? 1.0 : double.Parse(numPart, CultureInfo.InvariantCulture);
            return new GridLength(factor, GridUnitType.Star);
        }

        if (double.TryParse(val, CultureInfo.InvariantCulture, out var px))
            return new GridLength(px, GridUnitType.Pixel);

        return new GridLength(1, GridUnitType.Star);
    }

    public object ConvertBack(object? value, Type targetType, object? parameter, CultureInfo culture)
        => throw new NotSupportedException();
}

/// <summary>
/// Returns true when LayoutMode equals the specified mode in the parameter.
/// Parameter: "Compact", "Normal", or "Wide".
/// Useful for IsVisible bindings: IsVisible="{Binding CurrentMode, Converter={StaticResource LayoutModeEquals}, ConverterParameter=Compact}"
/// </summary>
public class LayoutModeEqualsConverter : IValueConverter
{
    public object Convert(object? value, Type targetType, object? parameter, CultureInfo culture)
    {
        if (value is not LayoutMode mode || parameter is not string target)
            return false;

        return mode.ToString().Equals(target, StringComparison.OrdinalIgnoreCase);
    }

    public object ConvertBack(object? value, Type targetType, object? parameter, CultureInfo culture)
        => throw new NotSupportedException();
}

/// <summary>
/// Returns true when LayoutMode does NOT equal the specified mode in the parameter.
/// Inverse of <see cref="LayoutModeEqualsConverter"/>.
/// </summary>
public class LayoutModeNotEqualsConverter : IValueConverter
{
    public object Convert(object? value, Type targetType, object? parameter, CultureInfo culture)
    {
        if (value is not LayoutMode mode || parameter is not string target)
            return true;

        return !mode.ToString().Equals(target, StringComparison.OrdinalIgnoreCase);
    }

    public object ConvertBack(object? value, Type targetType, object? parameter, CultureInfo culture)
        => throw new NotSupportedException();
}
