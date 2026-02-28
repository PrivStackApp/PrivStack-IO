namespace PrivStack.Desktop.Tests.Models;

using System.Text.Json;
using PrivStack.Services.Models;

public class PropertyModelsTests
{
    // =========================================================================
    // PropertyDefinition
    // =========================================================================

    [Fact]
    public void PropertyDefinition_defaults()
    {
        var def = new PropertyDefinition();
        def.Id.Should().BeEmpty();
        def.Name.Should().BeEmpty();
        def.Type.Should().Be(PropertyType.Text);
        def.Description.Should().BeNull();
        def.Options.Should().BeNull();
        def.DefaultValue.Should().BeNull();
        def.Icon.Should().BeNull();
        def.SortOrder.Should().Be(0);
        def.GroupId.Should().BeNull();
    }

    [Fact]
    public void PropertyDefinition_with_initializer()
    {
        var def = new PropertyDefinition
        {
            Id = "prop-1",
            Name = "Category",
            Type = PropertyType.Select,
            Options = ["Work", "Personal"],
            Icon = "Folder",
            SortOrder = 10,
            GroupId = "group-1"
        };

        def.Id.Should().Be("prop-1");
        def.Name.Should().Be("Category");
        def.Type.Should().Be(PropertyType.Select);
        def.Options.Should().HaveCount(2);
        def.Icon.Should().Be("Folder");
        def.SortOrder.Should().Be(10);
        def.GroupId.Should().Be("group-1");
    }

    [Fact]
    public void PropertyDefinition_serialize_roundtrip()
    {
        var def = new PropertyDefinition
        {
            Id = "test-id",
            Name = "Rating",
            Type = PropertyType.Number,
            SortOrder = 5
        };

        var json = JsonSerializer.Serialize(def);
        var deserialized = JsonSerializer.Deserialize<PropertyDefinition>(json);

        deserialized.Should().NotBeNull();
        deserialized!.Id.Should().Be("test-id");
        deserialized.Name.Should().Be("Rating");
        deserialized.Type.Should().Be(PropertyType.Number);
        deserialized.SortOrder.Should().Be(5);
    }

    [Fact]
    public void PropertyDefinition_type_serializes_as_string()
    {
        var def = new PropertyDefinition { Type = PropertyType.MultiSelect };
        var json = JsonSerializer.Serialize(def);
        json.Should().Contain("\"MultiSelect\"");
    }

    [Fact]
    public void PropertyDefinition_with_record()
    {
        var original = new PropertyDefinition { Id = "1", Name = "Test", SortOrder = 1 };
        var modified = original with { Name = "Modified", SortOrder = 2 };

        original.Name.Should().Be("Test");
        modified.Name.Should().Be("Modified");
        modified.Id.Should().Be("1");
        modified.SortOrder.Should().Be(2);
    }

    [Fact]
    public void PropertyDefinition_relation_type_with_allowed_link_types()
    {
        var def = new PropertyDefinition
        {
            Id = "rel-1",
            Name = "Related To",
            Type = PropertyType.Relation,
            AllowedLinkTypes = ["contact", "task"]
        };

        def.AllowedLinkTypes.Should().HaveCount(2);
        def.AllowedLinkTypes.Should().Contain("contact");
    }

    // =========================================================================
    // PropertyType enum
    // =========================================================================

    [Theory]
    [InlineData(PropertyType.Text)]
    [InlineData(PropertyType.Number)]
    [InlineData(PropertyType.Date)]
    [InlineData(PropertyType.Checkbox)]
    [InlineData(PropertyType.Select)]
    [InlineData(PropertyType.MultiSelect)]
    [InlineData(PropertyType.Url)]
    [InlineData(PropertyType.Relation)]
    public void PropertyType_all_values_serialize(PropertyType type)
    {
        var def = new PropertyDefinition { Type = type };
        var json = JsonSerializer.Serialize(def);
        var deserialized = JsonSerializer.Deserialize<PropertyDefinition>(json);
        deserialized!.Type.Should().Be(type);
    }

    // =========================================================================
    // PropertyGroup
    // =========================================================================

    [Fact]
    public void PropertyGroup_defaults()
    {
        var group = new PropertyGroup();
        group.Id.Should().BeEmpty();
        group.Name.Should().BeEmpty();
        group.SortOrder.Should().Be(0);
    }

    [Fact]
    public void PropertyGroup_serialize_roundtrip()
    {
        var group = new PropertyGroup { Id = "g1", Name = "Reference", SortOrder = 10 };
        var json = JsonSerializer.Serialize(group);
        var deserialized = JsonSerializer.Deserialize<PropertyGroup>(json);

        deserialized!.Id.Should().Be("g1");
        deserialized.Name.Should().Be("Reference");
        deserialized.SortOrder.Should().Be(10);
    }

    // =========================================================================
    // RelationEntry
    // =========================================================================

    [Fact]
    public void RelationEntry_defaults()
    {
        var entry = new RelationEntry();
        entry.LinkType.Should().BeEmpty();
        entry.EntityId.Should().BeEmpty();
    }

    [Fact]
    public void RelationEntry_serialize_roundtrip()
    {
        var entry = new RelationEntry { LinkType = "contact", EntityId = "c-123" };
        var json = JsonSerializer.Serialize(entry);
        var deserialized = JsonSerializer.Deserialize<RelationEntry>(json);

        deserialized!.LinkType.Should().Be("contact");
        deserialized.EntityId.Should().Be("c-123");
    }

    [Fact]
    public void RelationEntry_json_uses_short_names()
    {
        var entry = new RelationEntry { LinkType = "task", EntityId = "t-1" };
        var json = JsonSerializer.Serialize(entry);
        json.Should().Contain("\"lt\"");
        json.Should().Contain("\"id\"");
    }

    // =========================================================================
    // PropertyTemplate
    // =========================================================================

    [Fact]
    public void PropertyTemplate_defaults()
    {
        var template = new PropertyTemplate();
        template.Id.Should().BeEmpty();
        template.Name.Should().BeEmpty();
        template.Description.Should().BeNull();
        template.Icon.Should().BeNull();
        template.Entries.Should().BeEmpty();
    }

    [Fact]
    public void PropertyTemplate_with_entries()
    {
        var template = new PropertyTemplate
        {
            Id = "t1",
            Name = "Reading List",
            Description = "Track books and articles",
            Entries =
            [
                new PropertyTemplateEntry { PropertyDefId = "author", DefaultValue = null },
                new PropertyTemplateEntry { PropertyDefId = "progress", DefaultValue = "Not Started" }
            ]
        };

        template.Entries.Should().HaveCount(2);
        template.Entries[1].DefaultValue.Should().Be("Not Started");
    }

    [Fact]
    public void PropertyTemplate_serialize_roundtrip()
    {
        var template = new PropertyTemplate
        {
            Id = "t2",
            Name = "Research",
            Entries =
            [
                new PropertyTemplateEntry
                {
                    PropertyDefId = "inline-1",
                    InlineDefinition = new PropertyDefinition
                    {
                        Id = "inline-1",
                        Name = "Source URL",
                        Type = PropertyType.Url
                    }
                }
            ]
        };

        var json = JsonSerializer.Serialize(template);
        var deserialized = JsonSerializer.Deserialize<PropertyTemplate>(json);

        deserialized!.Name.Should().Be("Research");
        deserialized.Entries.Should().HaveCount(1);
        deserialized.Entries[0].InlineDefinition.Should().NotBeNull();
        deserialized.Entries[0].InlineDefinition!.Type.Should().Be(PropertyType.Url);
    }

    // =========================================================================
    // EntityMetadata
    // =========================================================================

    [Fact]
    public void EntityMetadata_construction()
    {
        var meta = new EntityMetadata(
            "entity-1", "task", "My Task", "Some preview",
            DateTimeOffset.UtcNow, DateTimeOffset.UtcNow,
            null, null,
            new List<string> { "work", "urgent" },
            new Dictionary<string, JsonElement>());

        meta.EntityId.Should().Be("entity-1");
        meta.LinkType.Should().Be("task");
        meta.Title.Should().Be("My Task");
        meta.Preview.Should().Be("Some preview");
        meta.Tags.Should().HaveCount(2);
        meta.Properties.Should().BeEmpty();
        meta.ParentId.Should().BeNull();
        meta.ParentTitle.Should().BeNull();
    }

    [Fact]
    public void EntityMetadata_with_parent()
    {
        var meta = new EntityMetadata(
            "entity-2", "page", "Child Page", null,
            null, null,
            "parent-1", "Parent Page",
            new List<string>(),
            new Dictionary<string, JsonElement>());

        meta.ParentId.Should().Be("parent-1");
        meta.ParentTitle.Should().Be("Parent Page");
    }
}
