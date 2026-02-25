namespace PrivStack.Desktop.Tests.Services;

using PrivStack.Desktop.Services;

public class NetworkPathDetectorTests
{
    [Theory]
    [InlineData(null)]
    [InlineData("")]
    [InlineData("   ")]
    public void Null_or_empty_returns_false(string? path)
    {
        NetworkPathDetector.IsNetworkPath(path).Should().BeFalse();
    }

    [Fact]
    public void Local_home_path_returns_false()
    {
        // Local paths should never be detected as network
        NetworkPathDetector.IsNetworkPath("/Users/test/Documents").Should().BeFalse();
    }

    [Fact]
    public void Mnt_path_detected_on_unix()
    {
        if (OperatingSystem.IsWindows()) return;
        NetworkPathDetector.IsNetworkPath("/mnt/nas/share").Should().BeTrue();
    }

    [Fact]
    public void Net_path_detected_on_unix()
    {
        if (OperatingSystem.IsWindows()) return;
        NetworkPathDetector.IsNetworkPath("/net/server/share").Should().BeTrue();
    }

    [Fact]
    public void Volumes_path_detected_on_unix()
    {
        if (OperatingSystem.IsWindows()) return;
        NetworkPathDetector.IsNetworkPath("/Volumes/NAS-Share").Should().BeTrue();
    }

    [Fact]
    public void Volumes_macintosh_hd_excluded_on_unix()
    {
        if (OperatingSystem.IsWindows()) return;
        NetworkPathDetector.IsNetworkPath("/Volumes/Macintosh HD/Users").Should().BeFalse();
    }

    [Fact]
    public void Trailing_slash_normalized()
    {
        if (OperatingSystem.IsWindows()) return;
        NetworkPathDetector.IsNetworkPath("/mnt/share/").Should().BeTrue();
    }

    [Fact]
    public void Regular_root_path_not_network()
    {
        NetworkPathDetector.IsNetworkPath("/usr/local/bin").Should().BeFalse();
    }
}
