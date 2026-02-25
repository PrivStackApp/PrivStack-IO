namespace PrivStack.Desktop.Tests.Sdk;

using System.Text.Json;
using PrivStack.Sdk.Json;

public class UnixTimestampConverterTests
{
    private static readonly JsonSerializerOptions Options = new()
    {
        Converters = { new UnixTimestampConverter() }
    };

    private sealed record TestRecord(DateTimeOffset Timestamp);

    [Fact]
    public void Read_number_token()
    {
        var json = """{"Timestamp": 1700000000000}""";
        var result = JsonSerializer.Deserialize<TestRecord>(json, Options);
        result.Should().NotBeNull();
        result!.Timestamp.Year.Should().Be(2023);
    }

    [Fact]
    public void Read_string_iso_format()
    {
        var json = """{"Timestamp": "2024-06-15T12:00:00Z"}""";
        var result = JsonSerializer.Deserialize<TestRecord>(json, Options);
        result.Should().NotBeNull();
        result!.Timestamp.Year.Should().Be(2024);
        result.Timestamp.Month.Should().Be(6);
    }

    [Fact]
    public void Read_string_millis_as_string()
    {
        var json = """{"Timestamp": "1700000000000"}""";
        var result = JsonSerializer.Deserialize<TestRecord>(json, Options);
        result.Should().NotBeNull();
        result!.Timestamp.Year.Should().Be(2023);
    }

    [Fact]
    public void Write_outputs_millis()
    {
        var timestamp = DateTimeOffset.FromUnixTimeMilliseconds(1700000000000);
        var record = new TestRecord(timestamp);
        var json = JsonSerializer.Serialize(record, Options);
        json.Should().Contain("1700000000000");
    }

    [Fact]
    public void Roundtrip_preserves_value()
    {
        var original = DateTimeOffset.UtcNow;
        // Truncate to millisecond precision (Unix timestamps don't store sub-ms)
        var truncated = DateTimeOffset.FromUnixTimeMilliseconds(original.ToUnixTimeMilliseconds());
        var record = new TestRecord(truncated);

        var json = JsonSerializer.Serialize(record, Options);
        var deserialized = JsonSerializer.Deserialize<TestRecord>(json, Options);

        deserialized!.Timestamp.Should().Be(truncated);
    }

    [Fact]
    public void Read_epoch_zero()
    {
        var json = """{"Timestamp": 0}""";
        var result = JsonSerializer.Deserialize<TestRecord>(json, Options);
        result!.Timestamp.Should().Be(DateTimeOffset.UnixEpoch);
    }

    [Fact]
    public void Read_negative_millis()
    {
        // Before epoch — 1969-12-31
        var json = """{"Timestamp": -86400000}""";
        var result = JsonSerializer.Deserialize<TestRecord>(json, Options);
        result!.Timestamp.Year.Should().Be(1969);
    }

    [Fact]
    public void Read_invalid_token_throws()
    {
        var json = """{"Timestamp": true}""";
        var act = () => JsonSerializer.Deserialize<TestRecord>(json, Options);
        act.Should().Throw<JsonException>();
    }
}
