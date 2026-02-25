namespace PrivStack.Desktop.Tests.Services;

using PrivStack.Desktop.Services;

public class EntityTypeMapTests
{
    [Fact]
    public void All_contains_known_entity_types()
    {
        EntityTypeMap.All.Should().NotBeEmpty();
        EntityTypeMap.All.Length.Should().BeGreaterOrEqualTo(10);
    }

    [Theory]
    [InlineData("page", "page")]
    [InlineData("task", "task")]
    [InlineData("contact", "contact")]
    [InlineData("event", "event")]
    [InlineData("journal_entry", "journal")]
    [InlineData("transaction", "transaction")]
    [InlineData("snippet", "snippet")]
    [InlineData("rss_article", "rss_article")]
    [InlineData("vault_file", "file")]
    [InlineData("sticky_note", "sticky_note")]
    [InlineData("email_message", "email_message")]
    [InlineData("web_clip", "web_clip")]
    public void GetLinkTypeForEntityType_returns_correct_link_type(string entityType, string expectedLinkType)
    {
        EntityTypeMap.GetLinkTypeForEntityType(entityType).Should().Be(expectedLinkType);
    }

    [Theory]
    [InlineData("page", "page")]
    [InlineData("task", "task")]
    [InlineData("contact", "contact")]
    [InlineData("journal", "journal_entry")]
    [InlineData("file", "vault_file")]
    public void GetEntityType_returns_correct_entity_type(string linkType, string expectedEntityType)
    {
        EntityTypeMap.GetEntityType(linkType).Should().Be(expectedEntityType);
    }

    [Fact]
    public void GetEntityType_returns_null_for_unknown_link_type()
    {
        EntityTypeMap.GetEntityType("nonexistent").Should().BeNull();
    }

    [Fact]
    public void GetLinkTypeForEntityType_returns_null_for_unknown()
    {
        EntityTypeMap.GetLinkTypeForEntityType("nonexistent").Should().BeNull();
    }

    [Theory]
    [InlineData("page", "Document")]
    [InlineData("task", "CheckSquare")]
    [InlineData("contact", "User")]
    [InlineData("event", "Calendar")]
    public void GetIcon_returns_correct_icon(string linkType, string expectedIcon)
    {
        EntityTypeMap.GetIcon(linkType).Should().Be(expectedIcon);
    }

    [Fact]
    public void GetIcon_returns_null_for_unknown()
    {
        EntityTypeMap.GetIcon("nonexistent").Should().BeNull();
    }

    [Theory]
    [InlineData("page", "Notes")]
    [InlineData("task", "Tasks")]
    [InlineData("contact", "Contacts")]
    [InlineData("event", "Calendar")]
    [InlineData("journal", "Journal")]
    public void GetDisplayName_returns_correct_display_name(string linkType, string expectedName)
    {
        EntityTypeMap.GetDisplayName(linkType).Should().Be(expectedName);
    }

    [Fact]
    public void GetDisplayName_returns_null_for_unknown()
    {
        EntityTypeMap.GetDisplayName("nonexistent").Should().BeNull();
    }

    [Fact]
    public void GetByLinkType_returns_full_info()
    {
        var info = EntityTypeMap.GetByLinkType("page");
        info.Should().NotBeNull();
        info!.EntityType.Should().Be("page");
        info.LinkType.Should().Be("page");
        info.Icon.Should().Be("Document");
        info.DisplayName.Should().Be("Notes");
    }

    [Fact]
    public void GetByLinkType_returns_null_for_unknown()
    {
        EntityTypeMap.GetByLinkType("nonexistent").Should().BeNull();
    }

    [Fact]
    public void All_entries_have_non_empty_fields()
    {
        foreach (var entry in EntityTypeMap.All)
        {
            entry.EntityType.Should().NotBeNullOrWhiteSpace();
            entry.LinkType.Should().NotBeNullOrWhiteSpace();
            entry.Icon.Should().NotBeNullOrWhiteSpace();
            entry.DisplayName.Should().NotBeNullOrWhiteSpace();
        }
    }

    [Fact]
    public void All_entity_types_are_unique()
    {
        var entityTypes = EntityTypeMap.All.Select(e => e.EntityType).ToList();
        entityTypes.Should().OnlyHaveUniqueItems();
    }

    [Fact]
    public void All_link_types_are_unique()
    {
        var linkTypes = EntityTypeMap.All.Select(e => e.LinkType).ToList();
        linkTypes.Should().OnlyHaveUniqueItems();
    }

    [Fact]
    public void Roundtrip_entity_to_link_to_entity()
    {
        foreach (var entry in EntityTypeMap.All)
        {
            var linkType = EntityTypeMap.GetLinkTypeForEntityType(entry.EntityType);
            linkType.Should().NotBeNull();
            var entityType = EntityTypeMap.GetEntityType(linkType!);
            entityType.Should().Be(entry.EntityType);
        }
    }
}
