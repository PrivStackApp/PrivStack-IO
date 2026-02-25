namespace PrivStack.Desktop.Tests.Services;

using PrivStack.Desktop.Services;

public class WorkspaceMigrationTests
{
    [Fact]
    public void WorkspaceMigrationProgress_construction()
    {
        var progress = new WorkspaceMigrationProgress(
            BytesCopied: 1024,
            TotalBytes: 4096,
            FilesCopied: 3,
            TotalFiles: 10,
            CurrentFile: "data.db",
            Phase: MigrationPhase.Copying
        );
        progress.BytesCopied.Should().Be(1024);
        progress.TotalBytes.Should().Be(4096);
        progress.FilesCopied.Should().Be(3);
        progress.TotalFiles.Should().Be(10);
        progress.CurrentFile.Should().Be("data.db");
        progress.Phase.Should().Be(MigrationPhase.Copying);
    }

    [Fact]
    public void WorkspaceMigrationProgress_is_record_with_equality()
    {
        var a = new WorkspaceMigrationProgress(100, 200, 1, 2, "f.txt", MigrationPhase.Copying);
        var b = new WorkspaceMigrationProgress(100, 200, 1, 2, "f.txt", MigrationPhase.Copying);
        a.Should().Be(b);
    }

    [Theory]
    [InlineData(MigrationPhase.Calculating)]
    [InlineData(MigrationPhase.Copying)]
    [InlineData(MigrationPhase.Verifying)]
    [InlineData(MigrationPhase.Reloading)]
    [InlineData(MigrationPhase.CleaningUp)]
    [InlineData(MigrationPhase.Complete)]
    [InlineData(MigrationPhase.Failed)]
    public void MigrationPhase_all_values(MigrationPhase phase)
    {
        Enum.IsDefined(phase).Should().BeTrue();
    }
}
