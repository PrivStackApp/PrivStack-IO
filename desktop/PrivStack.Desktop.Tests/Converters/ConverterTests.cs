using System.Globalization;
using PrivStack.Desktop.Converters;

namespace PrivStack.Desktop.Tests.Converters;

public class EqualityConverterTests
{
    private readonly EqualityConverter _converter = EqualityConverter.Instance;

    [Fact]
    public void Convert_EqualIntegers_ReturnsTrue()
    {
        var values = new List<object?> { 42, 42 };

        var result = _converter.Convert(values, typeof(bool), null, CultureInfo.InvariantCulture);

        result.Should().Be(true);
    }

    [Fact]
    public void Convert_DifferentIntegers_ReturnsFalse()
    {
        var values = new List<object?> { 42, 43 };

        var result = _converter.Convert(values, typeof(bool), null, CultureInfo.InvariantCulture);

        result.Should().Be(false);
    }

    [Fact]
    public void Convert_EqualStrings_ReturnsTrue()
    {
        var values = new List<object?> { "hello", "hello" };

        var result = _converter.Convert(values, typeof(bool), null, CultureInfo.InvariantCulture);

        result.Should().Be(true);
    }

    [Fact]
    public void Convert_DifferentStrings_ReturnsFalse()
    {
        var values = new List<object?> { "hello", "world" };

        var result = _converter.Convert(values, typeof(bool), null, CultureInfo.InvariantCulture);

        result.Should().Be(false);
    }

    [Fact]
    public void Convert_BothNull_ReturnsTrue()
    {
        var values = new List<object?> { null, null };

        var result = _converter.Convert(values, typeof(bool), null, CultureInfo.InvariantCulture);

        result.Should().Be(true);
    }

    [Fact]
    public void Convert_FirstNull_ReturnsFalse()
    {
        var values = new List<object?> { null, "value" };

        var result = _converter.Convert(values, typeof(bool), null, CultureInfo.InvariantCulture);

        result.Should().Be(false);
    }

    [Fact]
    public void Convert_SecondNull_ReturnsFalse()
    {
        var values = new List<object?> { "value", null };

        var result = _converter.Convert(values, typeof(bool), null, CultureInfo.InvariantCulture);

        result.Should().Be(false);
    }

    [Fact]
    public void Convert_EmptyList_ReturnsFalse()
    {
        var values = new List<object?>();

        var result = _converter.Convert(values, typeof(bool), null, CultureInfo.InvariantCulture);

        result.Should().Be(false);
    }

    [Fact]
    public void Convert_SingleValue_ReturnsFalse()
    {
        var values = new List<object?> { "only_one" };

        var result = _converter.Convert(values, typeof(bool), null, CultureInfo.InvariantCulture);

        result.Should().Be(false);
    }

    [Fact]
    public void Convert_EqualEnums_ReturnsTrue()
    {
        var values = new List<object?> { DayOfWeek.Monday, DayOfWeek.Monday };

        var result = _converter.Convert(values, typeof(bool), null, CultureInfo.InvariantCulture);

        result.Should().Be(true);
    }

    [Fact]
    public void Convert_DifferentEnums_ReturnsFalse()
    {
        var values = new List<object?> { DayOfWeek.Monday, DayOfWeek.Tuesday };

        var result = _converter.Convert(values, typeof(bool), null, CultureInfo.InvariantCulture);

        result.Should().Be(false);
    }

    [Fact]
    public void Convert_EqualObjects_ReturnsTrue()
    {
        var obj = new object();
        var values = new List<object?> { obj, obj };

        var result = _converter.Convert(values, typeof(bool), null, CultureInfo.InvariantCulture);

        result.Should().Be(true);
    }

    [Fact]
    public void Convert_DifferentObjectsSameContent_UsesEquals()
    {
        // Two different string instances with same content
        var str1 = new string("test".ToCharArray());
        var str2 = new string("test".ToCharArray());
        var values = new List<object?> { str1, str2 };

        var result = _converter.Convert(values, typeof(bool), null, CultureInfo.InvariantCulture);

        // String.Equals compares content, not reference
        result.Should().Be(true);
    }
}
