namespace PrivStack.Desktop.Tests.Sdk;

using PrivStack.Sdk.Helpers;

public class SeedHelperTests
{
    [Fact]
    public void BuildDateParameters_returns_null_when_both_null()
    {
        var result = SeedHelper.BuildDateParameters(null, null);
        result.Should().BeNull();
    }

    [Fact]
    public void BuildDateParameters_with_created_at_only()
    {
        var date = new DateTimeOffset(2024, 6, 15, 12, 0, 0, TimeSpan.Zero);
        var result = SeedHelper.BuildDateParameters(date, null);

        result.Should().NotBeNull();
        result.Should().ContainKey("created_at");
        result.Should().NotContainKey("modified_at");
        result!["created_at"].Should().Be(date.ToUnixTimeMilliseconds().ToString());
    }

    [Fact]
    public void BuildDateParameters_with_modified_at_only()
    {
        var date = new DateTimeOffset(2024, 6, 15, 12, 0, 0, TimeSpan.Zero);
        var result = SeedHelper.BuildDateParameters(null, date);

        result.Should().NotBeNull();
        result.Should().NotContainKey("created_at");
        result.Should().ContainKey("modified_at");
        result!["modified_at"].Should().Be(date.ToUnixTimeMilliseconds().ToString());
    }

    [Fact]
    public void BuildDateParameters_with_both()
    {
        var created = new DateTimeOffset(2024, 1, 1, 0, 0, 0, TimeSpan.Zero);
        var modified = new DateTimeOffset(2024, 6, 15, 12, 0, 0, TimeSpan.Zero);
        var result = SeedHelper.BuildDateParameters(created, modified);

        result.Should().NotBeNull();
        result.Should().HaveCount(2);
        result.Should().ContainKey("created_at");
        result.Should().ContainKey("modified_at");
    }

    [Fact]
    public void BuildDateParameters_uses_unix_millis()
    {
        var epoch = DateTimeOffset.UnixEpoch;
        var result = SeedHelper.BuildDateParameters(epoch, null);

        result!["created_at"].Should().Be("0");
    }

    [Fact]
    public void BuildDateParameters_preserves_precision()
    {
        var date = new DateTimeOffset(2024, 6, 15, 14, 30, 45, 123, TimeSpan.Zero);
        var result = SeedHelper.BuildDateParameters(date, null);

        var millis = long.Parse(result!["created_at"]);
        var roundtrip = DateTimeOffset.FromUnixTimeMilliseconds(millis);
        roundtrip.Should().Be(date);
    }
}
