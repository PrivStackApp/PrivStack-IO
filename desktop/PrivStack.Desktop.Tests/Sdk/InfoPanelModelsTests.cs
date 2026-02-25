namespace PrivStack.Desktop.Tests.Sdk;

using PrivStack.Sdk;

public class InfoPanelModelsTests
{
    [Fact]
    public void InfoPanelDetailField_construction()
    {
        var field = new InfoPanelDetailField("Priority", "High", "#FF0000");
        field.Label.Should().Be("Priority");
        field.Value.Should().Be("High");
        field.Color.Should().Be("#FF0000");
    }

    [Fact]
    public void InfoPanelDetailField_color_defaults_to_null()
    {
        var field = new InfoPanelDetailField("Status", "Active");
        field.Color.Should().BeNull();
    }

    [Fact]
    public void InfoPanelDetailField_is_record_with_equality()
    {
        var a = new InfoPanelDetailField("Key", "Value");
        var b = new InfoPanelDetailField("Key", "Value");
        a.Should().Be(b);
    }

    [Fact]
    public void BacklinkInfo_construction()
    {
        var backlink = new BacklinkInfo("p-1", "page", "My Page");
        backlink.SourceId.Should().Be("p-1");
        backlink.SourceLinkType.Should().Be("page");
        backlink.SourceTitle.Should().Be("My Page");
    }

    [Fact]
    public void BacklinkInfo_is_record_with_equality()
    {
        var a = new BacklinkInfo("t-1", "task", "Task 1");
        var b = new BacklinkInfo("t-1", "task", "Task 1");
        a.Should().Be(b);
    }

    [Theory]
    [InlineData(ToastType.Success)]
    [InlineData(ToastType.Info)]
    [InlineData(ToastType.Warning)]
    [InlineData(ToastType.Error)]
    public void ToastType_all_values(ToastType type)
    {
        Enum.IsDefined(type).Should().BeTrue();
    }
}
