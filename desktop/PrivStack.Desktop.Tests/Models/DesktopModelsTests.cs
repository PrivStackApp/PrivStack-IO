namespace PrivStack.Desktop.Tests.Models;

using System.Text.Json;
using PrivStack.Desktop.Models;

public class DesktopModelsTests
{
    // =========================================================================
    // FileEventSyncState
    // =========================================================================

    [Fact]
    public void FileEventSyncState_defaults()
    {
        var state = new FileEventSyncState();
        state.ProcessedWatermarks.Should().BeEmpty();
    }

    [Fact]
    public void FileEventSyncState_serialization_roundtrip()
    {
        var state = new FileEventSyncState
        {
            ProcessedWatermarks = new Dictionary<string, string>
            {
                ["peer-a"] = "event-042.enc",
                ["peer-b"] = "event-017.enc"
            }
        };
        var json = JsonSerializer.Serialize(state);
        json.Should().Contain("processed_watermarks");

        var deserialized = JsonSerializer.Deserialize<FileEventSyncState>(json);
        deserialized!.ProcessedWatermarks.Should().HaveCount(2);
        deserialized.ProcessedWatermarks["peer-a"].Should().Be("event-042.enc");
    }

    // =========================================================================
    // DatasetSidecar
    // =========================================================================

    [Fact]
    public void DatasetSidecar_defaults()
    {
        var sidecar = new DatasetSidecar();
        sidecar.Version.Should().Be(1);
        sidecar.DatasetId.Should().BeEmpty();
        sidecar.DatasetName.Should().BeEmpty();
        sidecar.SourceFilename.Should().BeEmpty();
        sidecar.FileSize.Should().Be(0);
        sidecar.FileHash.Should().BeEmpty();
        sidecar.PeerId.Should().BeEmpty();
    }

    [Fact]
    public void DatasetSidecar_serialization_roundtrip()
    {
        var now = DateTimeOffset.UtcNow;
        var sidecar = new DatasetSidecar
        {
            DatasetId = "ds-1",
            DatasetName = "Sales",
            SourceFilename = "sales.csv",
            FileSize = 1024,
            FileHash = "sha256:abc",
            ImportedAt = now,
            PeerId = "peer-1"
        };
        var json = JsonSerializer.Serialize(sidecar);
        json.Should().Contain("dataset_id");
        json.Should().Contain("file_hash");

        var deserialized = JsonSerializer.Deserialize<DatasetSidecar>(json);
        deserialized!.DatasetId.Should().Be("ds-1");
        deserialized.FileSize.Should().Be(1024);
    }

    // =========================================================================
    // StorageLocation
    // =========================================================================

    [Fact]
    public void StorageLocation_defaults()
    {
        var loc = new StorageLocation();
        loc.Type.Should().Be("Default");
        loc.CustomPath.Should().BeNull();
    }

    [Fact]
    public void StorageLocation_with_custom_path()
    {
        var loc = new StorageLocation
        {
            Type = "Custom",
            CustomPath = "/Users/test/Sync"
        };
        loc.Type.Should().Be("Custom");
        loc.CustomPath.Should().Be("/Users/test/Sync");
    }

    [Theory]
    [InlineData(DataDirectoryType.Default)]
    [InlineData(DataDirectoryType.Custom)]
    [InlineData(DataDirectoryType.GoogleDrive)]
    [InlineData(DataDirectoryType.ICloud)]
    [InlineData(DataDirectoryType.PrivStackCloud)]
    public void DataDirectoryType_all_values(DataDirectoryType type)
    {
        Enum.IsDefined(type).Should().BeTrue();
    }

    // =========================================================================
    // PluginPaletteModels
    // =========================================================================

    [Fact]
    public void PluginPaletteItem_construction()
    {
        var item = new PluginPaletteItem(
            Id: "heading",
            Name: "Heading",
            Description: "Large heading block",
            Icon: "H1",
            Keywords: "heading title h1",
            Command: "add_block",
            ArgsJson: "{\"type\":\"heading\"}"
        );
        item.Id.Should().Be("heading");
        item.Keywords.Should().Contain("heading");
    }

    [Fact]
    public void PluginPaletteDefinition_construction()
    {
        var def = new PluginPaletteDefinition(
            Id: "blocks",
            Title: "Add Block",
            Placeholder: "Search blocks...",
            Shortcut: "Cmd+/",
            Items: new List<PluginPaletteItem>()
        );
        def.Id.Should().Be("blocks");
        def.Shortcut.Should().Be("Cmd+/");
        def.PluginId.Should().BeEmpty(); // default before registration
    }

    [Fact]
    public void PluginPaletteDefinition_PluginId_set_at_registration()
    {
        var def = new PluginPaletteDefinition(
            Id: "blocks",
            Title: "Add Block",
            Placeholder: "Search...",
            Shortcut: null,
            Items: []
        ) { PluginId = "privstack.notes" };

        def.PluginId.Should().Be("privstack.notes");
    }

    // =========================================================================
    // EntityMetadata
    // =========================================================================

    [Fact]
    public void EntityMetadata_construction()
    {
        var meta = new EntityMetadata(
            EntityId: "e-1",
            LinkType: "page",
            Title: "My Page",
            Preview: "First paragraph...",
            CreatedAt: DateTimeOffset.UtcNow,
            ModifiedAt: null,
            ParentId: null,
            ParentTitle: null,
            Tags: new List<string> { "work", "important" },
            Properties: new Dictionary<string, JsonElement>()
        );
        meta.EntityId.Should().Be("e-1");
        meta.LinkType.Should().Be("page");
        meta.Tags.Should().HaveCount(2);
        meta.ModifiedAt.Should().BeNull();
    }
}
