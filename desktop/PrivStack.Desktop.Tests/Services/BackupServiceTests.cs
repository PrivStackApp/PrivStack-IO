using PrivStack.Desktop.Services;

namespace PrivStack.Desktop.Tests.Services;

public class BackupServiceTests
{
    [Theory]
    [InlineData(500, "500 B")]
    [InlineData(1024, "1.0 KB")]
    [InlineData(1536, "1.5 KB")]
    [InlineData(1048576, "1.0 MB")]
    [InlineData(1572864, "1.5 MB")]
    [InlineData(1073741824, "1.0 GB")]
    [InlineData(1610612736, "1.5 GB")]
    public void BackupInfo_FormattedSize_ReturnsCorrectFormat(long bytes, string expected)
    {
        var info = new BackupInfo("/path/backup.zip", DateTime.UtcNow, bytes);
        info.FormattedSize.Should().Be(expected);
    }

    [Fact]
    public void BackupInfo_ZeroBytes_FormatsCorrectly()
    {
        var info = new BackupInfo("/path/backup.zip", DateTime.UtcNow, 0);
        info.FormattedSize.Should().Be("0 B");
    }

    [Fact]
    public void BackupCompletedEventArgs_SuccessProperties()
    {
        var args = new BackupCompletedEventArgs(true, "/path/backup.zip", null);

        args.Success.Should().BeTrue();
        args.BackupPath.Should().Be("/path/backup.zip");
        args.ErrorMessage.Should().BeNull();
    }

    [Fact]
    public void BackupCompletedEventArgs_FailureProperties()
    {
        var args = new BackupCompletedEventArgs(false, null, "Disk full");

        args.Success.Should().BeFalse();
        args.BackupPath.Should().BeNull();
        args.ErrorMessage.Should().Be("Disk full");
    }

    [Fact]
    public void BackupInfo_RecordEquality()
    {
        var time = DateTime.UtcNow;
        var a = new BackupInfo("/path/a.zip", time, 1024);
        var b = new BackupInfo("/path/a.zip", time, 1024);

        a.Should().Be(b);
    }

    [Fact]
    public void BackupInfo_RecordInequality()
    {
        var time = DateTime.UtcNow;
        var a = new BackupInfo("/path/a.zip", time, 1024);
        var b = new BackupInfo("/path/b.zip", time, 1024);

        a.Should().NotBe(b);
    }
}
