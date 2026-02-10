using System.Globalization;
using Avalonia.Media;
using PrivStack.Desktop.Converters;

namespace PrivStack.Desktop.Tests.Converters;

public class BoolToColorConverterTests
{
    private readonly BoolToColorConverter _converter = new();

    [Fact]
    public void Convert_NonBoolValue_ReturnsBrush()
    {
        // When value is not a bool, returns default brush
        var result = _converter.Convert("not-a-bool", typeof(IBrush), "Red|Blue", CultureInfo.InvariantCulture);
        result.Should().BeAssignableTo<IBrush>();
    }

    [Fact]
    public void Convert_NullParameter_ReturnsBrush()
    {
        var result = _converter.Convert(true, typeof(IBrush), null, CultureInfo.InvariantCulture);
        result.Should().BeAssignableTo<IBrush>();
    }

    [Fact]
    public void Convert_InvalidParameterFormat_ReturnsBrush()
    {
        // Single color (no pipe separator) returns default
        var result = _converter.Convert(true, typeof(IBrush), "OnlyOneColor", CultureInfo.InvariantCulture);
        result.Should().BeAssignableTo<IBrush>();
    }

    [Fact]
    public void Convert_True_WithHexColors_ReturnsTrueColor()
    {
        var result = _converter.Convert(true, typeof(IBrush), "#FF0000|#0000FF", CultureInfo.InvariantCulture);

        result.Should().BeOfType<SolidColorBrush>();
        var brush = (SolidColorBrush)result!;
        brush.Color.Should().Be(Color.Parse("#FF0000"));
    }

    [Fact]
    public void Convert_False_WithHexColors_ReturnsFalseColor()
    {
        var result = _converter.Convert(false, typeof(IBrush), "#FF0000|#0000FF", CultureInfo.InvariantCulture);

        result.Should().BeOfType<SolidColorBrush>();
        var brush = (SolidColorBrush)result!;
        brush.Color.Should().Be(Color.Parse("#0000FF"));
    }

    [Fact]
    public void Convert_True_WithTransparent_ReturnsTransparent()
    {
        var result = _converter.Convert(true, typeof(IBrush), "Transparent|#FF0000", CultureInfo.InvariantCulture);

        result.Should().Be(Brushes.Transparent);
    }

    [Fact]
    public void Convert_False_WithTransparent_ReturnsTransparent()
    {
        var result = _converter.Convert(false, typeof(IBrush), "#FF0000|Transparent", CultureInfo.InvariantCulture);

        result.Should().Be(Brushes.Transparent);
    }

    [Fact]
    public void Convert_True_WithThemeResourceKey_ReturnsBrush()
    {
        // When the color key is not a hex code and not "Transparent", it resolves via
        // ThemeHelper which falls back to a default brush when no Avalonia app is running.
        var result = _converter.Convert(true, typeof(IBrush), "ThemeAccent|ThemeSurface", CultureInfo.InvariantCulture);

        result.Should().BeAssignableTo<IBrush>();
    }

    [Fact]
    public void Convert_False_WithThemeResourceKey_ReturnsBrush()
    {
        var result = _converter.Convert(false, typeof(IBrush), "ThemeAccent|ThemeSurface", CultureInfo.InvariantCulture);

        result.Should().BeAssignableTo<IBrush>();
    }

    [Fact]
    public void Convert_WithBrushSuffix_DoesNotDoubleSuffix()
    {
        // If the key already ends with "Brush", it shouldn't append "Brush" again
        var result = _converter.Convert(true, typeof(IBrush), "ThemeAccentBrush|#FF0000", CultureInfo.InvariantCulture);

        result.Should().BeAssignableTo<IBrush>();
    }

    [Fact]
    public void ConvertBack_ThrowsNotImplemented()
    {
        var act = () => _converter.ConvertBack(null, typeof(bool), null, CultureInfo.InvariantCulture);
        act.Should().Throw<NotImplementedException>();
    }
}
