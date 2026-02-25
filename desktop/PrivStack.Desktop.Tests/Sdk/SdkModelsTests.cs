namespace PrivStack.Desktop.Tests.Sdk;

using PrivStack.Sdk.Capabilities;

public class SdkModelsTests
{
    // =========================================================================
    // LinkableItem
    // =========================================================================

    [Fact]
    public void LinkableItem_construction()
    {
        var item = new LinkableItem
        {
            Id = "t-1",
            LinkType = "task",
            Title = "My Task"
        };
        item.Id.Should().Be("t-1");
        item.LinkType.Should().Be("task");
        item.Title.Should().Be("My Task");
        item.Subtitle.Should().BeNull();
        item.Icon.Should().BeNull();
        item.ModifiedAt.Should().BeNull();
        item.Metadata.Should().BeNull();
    }

    [Fact]
    public void LinkableItem_with_all_fields()
    {
        var now = DateTime.UtcNow;
        var item = new LinkableItem
        {
            Id = "c-1",
            LinkType = "contact",
            Title = "John Doe",
            Subtitle = "CEO",
            Icon = "User",
            ModifiedAt = now,
            Metadata = new Dictionary<string, string> { ["email"] = "john@example.com" }
        };
        item.Subtitle.Should().Be("CEO");
        item.ModifiedAt.Should().Be(now);
        item.Metadata.Should().ContainKey("email");
    }

    // =========================================================================
    // IndexableContentModels
    // =========================================================================

    [Fact]
    public void IndexableContentRequest_defaults()
    {
        var req = new IndexableContentRequest();
        req.ModifiedSince.Should().BeNull();
        req.BatchSize.Should().Be(0);
    }

    [Fact]
    public void IndexableContentResult_defaults()
    {
        var result = new IndexableContentResult();
        result.Chunks.Should().BeEmpty();
        result.DeletedEntityIds.Should().BeEmpty();
    }

    [Fact]
    public void ContentChunk_construction()
    {
        var chunk = new ContentChunk
        {
            EntityId = "e-1",
            EntityType = "page",
            PluginId = "privstack.notes",
            ChunkPath = "content",
            Text = "Hello world",
            ContentHash = "sha256:abc",
            Title = "My Page",
            LinkType = "page"
        };
        chunk.EntityId.Should().Be("e-1");
        chunk.Text.Should().Be("Hello world");
    }

    // =========================================================================
    // IntentModels
    // =========================================================================

    [Fact]
    public void IntentDescriptor_construction()
    {
        var desc = new IntentDescriptor
        {
            IntentId = "calendar.create_event",
            DisplayName = "Create Event",
            Description = "Creates a new calendar event",
            PluginId = "privstack.calendar"
        };
        desc.IntentId.Should().Be("calendar.create_event");
        desc.Slots.Should().BeEmpty();
        desc.Icon.Should().BeNull();
    }

    [Fact]
    public void IntentDescriptor_with_slots()
    {
        var desc = new IntentDescriptor
        {
            IntentId = "tasks.create_task",
            DisplayName = "Create Task",
            Description = "Creates a new task",
            PluginId = "privstack.tasks",
            Slots = new List<IntentSlot>
            {
                new() { Name = "title", DisplayName = "Title", Description = "Task title", Type = IntentSlotType.String },
                new() { Name = "due_date", DisplayName = "Due Date", Description = "When it's due", Type = IntentSlotType.Date, Required = false }
            }
        };
        desc.Slots.Should().HaveCount(2);
        desc.Slots[0].Required.Should().BeTrue(); // default
        desc.Slots[1].Required.Should().BeFalse();
    }

    [Fact]
    public void IntentSlot_defaults()
    {
        var slot = new IntentSlot
        {
            Name = "title",
            DisplayName = "Title",
            Description = "The title",
            Type = IntentSlotType.String
        };
        slot.Required.Should().BeTrue();
        slot.DefaultValue.Should().BeNull();
    }

    [Theory]
    [InlineData(IntentSlotType.String)]
    [InlineData(IntentSlotType.Text)]
    [InlineData(IntentSlotType.DateTime)]
    [InlineData(IntentSlotType.Date)]
    [InlineData(IntentSlotType.Time)]
    [InlineData(IntentSlotType.Duration)]
    [InlineData(IntentSlotType.Integer)]
    [InlineData(IntentSlotType.Boolean)]
    [InlineData(IntentSlotType.Email)]
    [InlineData(IntentSlotType.Url)]
    [InlineData(IntentSlotType.EntityReference)]
    public void IntentSlotType_all_values(IntentSlotType type)
    {
        var slot = new IntentSlot
        {
            Name = "test",
            DisplayName = "Test",
            Description = "Test",
            Type = type
        };
        slot.Type.Should().Be(type);
    }

    [Fact]
    public void IntentRequest_construction()
    {
        var req = new IntentRequest
        {
            IntentId = "calendar.create_event",
            Slots = new Dictionary<string, string>
            {
                ["title"] = "Meeting",
                ["start_date"] = "2024-06-15"
            }
        };
        req.IntentId.Should().Be("calendar.create_event");
        req.Slots.Should().HaveCount(2);
        req.SourceEntityId.Should().BeNull();
    }

    [Fact]
    public void IntentResult_failure()
    {
        var result = IntentResult.Failure("Something went wrong");
        result.Success.Should().BeFalse();
        result.ErrorMessage.Should().Be("Something went wrong");
        result.CreatedEntityId.Should().BeNull();
    }

    [Fact]
    public void IntentResult_success()
    {
        var result = new IntentResult
        {
            Success = true,
            CreatedEntityId = "e-123",
            CreatedEntityType = "event",
            NavigationLinkType = "event",
            Summary = "Created event 'Meeting'"
        };
        result.Success.Should().BeTrue();
        result.CreatedEntityId.Should().Be("e-123");
        result.Summary.Should().Contain("Meeting");
    }

    [Fact]
    public void IntentResult_with_warnings()
    {
        var result = new IntentResult
        {
            Success = true,
            Warnings = new List<string> { "Unknown slot 'foo' was ignored" }
        };
        result.Warnings.Should().HaveCount(1);
    }

    // =========================================================================
    // QuickActionDescriptor
    // =========================================================================

    [Fact]
    public void QuickActionDescriptor_defaults()
    {
        var desc = new QuickActionDescriptor
        {
            ActionId = "test.action",
            DisplayName = "Test Action",
            PluginId = "privstack.test"
        };
        desc.Category.Should().Be("Quick Actions");
        desc.HasUI.Should().BeFalse();
        desc.Description.Should().BeNull();
        desc.Icon.Should().BeNull();
        desc.DefaultShortcutHint.Should().BeNull();
    }

    [Fact]
    public void QuickActionDescriptor_with_ui()
    {
        var desc = new QuickActionDescriptor
        {
            ActionId = "task.new",
            DisplayName = "New Task",
            PluginId = "privstack.tasks",
            HasUI = true,
            DefaultShortcutHint = "Cmd+T",
            Icon = "CheckSquare"
        };
        desc.HasUI.Should().BeTrue();
        desc.DefaultShortcutHint.Should().Be("Cmd+T");
    }
}
