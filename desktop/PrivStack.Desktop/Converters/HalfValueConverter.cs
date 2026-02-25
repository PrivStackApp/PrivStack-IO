using System.Globalization;
using Avalonia.Data.Converters;

namespace PrivStack.Desktop.Converters;

public class HalfValueConverter : IValueConverter
{
    public object Convert(object? value, Type targetType, object? parameter, CultureInfo culture)
    {
        if (value is double d)
            return d / 2.0;
        return double.PositiveInfinity;
    }

    public object ConvertBack(object? value, Type targetType, object? parameter, CultureInfo culture)
        => throw new NotSupportedException();
}
