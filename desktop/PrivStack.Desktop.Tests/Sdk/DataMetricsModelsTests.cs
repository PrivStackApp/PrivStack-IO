namespace PrivStack.Desktop.Tests.Sdk;

using PrivStack.Sdk.Capabilities;

public class DataMetricsModelsTests
{
    // =========================================================================
    // PluginDataMetrics
    // =========================================================================

    [Fact]
    public void PluginDataMetrics_defaults()
    {
        var m = new PluginDataMetrics();
        m.EntityCount.Should().Be(0);
        m.EstimatedSizeBytes.Should().Be(0);
        m.Tables.Should().BeEmpty();
    }

    // =========================================================================
    // DataTableInfo
    // =========================================================================

    [Fact]
    public void DataTableInfo_defaults()
    {
        var t = new DataTableInfo { Name = "Pages", EntityType = "page" };
        t.RowCount.Should().Be(0);
        t.BackingMode.Should().Be("entity");
        t.EstimatedSizeBytes.Should().Be(0);
        t.ActualSizeBytes.Should().Be(0);
        t.ParentName.Should().BeNull();
        t.ParentId.Should().BeNull();
        t.BlockId.Should().BeNull();
        t.PluginId.Should().BeNull();
    }

    [Fact]
    public void DataTableInfo_HasParent_false_when_no_parent()
    {
        var t = new DataTableInfo { Name = "Pages", EntityType = "page" };
        t.HasParent.Should().BeFalse();
    }

    [Fact]
    public void DataTableInfo_HasParent_true_when_set()
    {
        var t = new DataTableInfo { Name = "Table Rows", EntityType = "table_row", ParentId = "p-1" };
        t.HasParent.Should().BeTrue();
    }

    // =========================================================================
    // FormatSize
    // =========================================================================

    [Theory]
    [InlineData(0, "0 B")]
    [InlineData(512, "512 B")]
    [InlineData(1024, "1 KB")]
    [InlineData(1536, "1.5 KB")]
    [InlineData(1048576, "1 MB")]
    [InlineData(1073741824, "1 GB")]
    public void FormatSize_formats_correctly(long bytes, string expected)
    {
        DataTableInfo.FormatSize(bytes).Should().Be(expected);
    }

    // =========================================================================
    // FormattedSize
    // =========================================================================

    [Fact]
    public void FormattedSize_empty_when_both_zero()
    {
        var t = new DataTableInfo { Name = "T", EntityType = "t" };
        t.FormattedSize.Should().BeEmpty();
    }

    [Fact]
    public void FormattedSize_actual_only()
    {
        var t = new DataTableInfo { Name = "T", EntityType = "t", ActualSizeBytes = 1024 };
        t.FormattedSize.Should().Be("1 KB");
    }

    [Fact]
    public void FormattedSize_estimated_only()
    {
        var t = new DataTableInfo { Name = "T", EntityType = "t", EstimatedSizeBytes = 2048 };
        t.FormattedSize.Should().Be("~2 KB");
    }

    [Fact]
    public void FormattedSize_both()
    {
        var t = new DataTableInfo
        {
            Name = "T",
            EntityType = "t",
            ActualSizeBytes = 1024,
            EstimatedSizeBytes = 2048
        };
        t.FormattedSize.Should().Be("1 KB | ~2 KB est.");
    }

    // =========================================================================
    // FormattedRowCount
    // =========================================================================

    [Fact]
    public void FormattedRowCount_uses_thousands_separator()
    {
        var t = new DataTableInfo { Name = "T", EntityType = "t", RowCount = 1500 };
        t.FormattedRowCount.Should().Contain("1");
        t.FormattedRowCount.Should().Contain("500");
    }

    // =========================================================================
    // BackingModeIcon
    // =========================================================================

    [Theory]
    [InlineData("entity")]
    [InlineData("file")]
    [InlineData("blob")]
    public void BackingModeIcon_returns_svg_path(string mode)
    {
        var t = new DataTableInfo { Name = "T", EntityType = "t", BackingMode = mode };
        t.BackingModeIcon.Should().StartWith("M");
    }

    [Fact]
    public void BackingModeIcon_file_differs_from_entity()
    {
        var entity = new DataTableInfo { Name = "T", EntityType = "t", BackingMode = "entity" };
        var file = new DataTableInfo { Name = "T", EntityType = "t", BackingMode = "file" };
        entity.BackingModeIcon.Should().NotBe(file.BackingModeIcon);
    }
}
