using PrivStack.Sdk;

namespace PrivStack.Desktop.Tests.Sdk;

public class PluginMetadataTests
{
    [Fact]
    public void RequiredProperties_ArePopulated()
    {
        var meta = new PluginMetadata
        {
            Id = "privstack.notes",
            Name = "Notes",
            Description = "A notes plugin",
            Version = new Version(2, 1, 0)
        };

        meta.Id.Should().Be("privstack.notes");
        meta.Name.Should().Be("Notes");
        meta.Description.Should().Be("A notes plugin");
        meta.Version.Should().Be(new Version(2, 1, 0));
    }

    [Fact]
    public void DefaultValues_AreCorrect()
    {
        var meta = new PluginMetadata
        {
            Id = "test",
            Name = "Test",
            Description = "Test",
            Version = new Version(1, 0)
        };

        meta.CanDisable.Should().BeTrue();
        meta.NavigationOrder.Should().Be(1000);
        meta.Author.Should().Be("PrivStack");
        meta.Category.Should().Be(PluginCategory.Utility);
        meta.Tags.Should().BeEmpty();
        meta.IsExperimental.Should().BeFalse();
        meta.IsHardLocked.Should().BeFalse();
        meta.Icon.Should().BeNull();
        meta.MinAppVersion.Should().BeNull();
        meta.WebsiteUrl.Should().BeNull();
        meta.HardLockedReason.Should().BeNull();
    }

    [Fact]
    public void ToString_ContainsNameIdAndVersion()
    {
        var meta = new PluginMetadata
        {
            Id = "privstack.notes",
            Name = "Notes",
            Description = "Test",
            Version = new Version(1, 2, 3)
        };

        var str = meta.ToString();

        str.Should().Be("Notes (privstack.notes) v1.2.3");
    }
}
