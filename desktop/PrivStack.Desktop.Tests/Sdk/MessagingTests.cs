namespace PrivStack.Desktop.Tests.Sdk;

using PrivStack.Sdk.Capabilities;
using PrivStack.Sdk.Messaging;

public class MessagingTests
{
    // =========================================================================
    // EntitySyncedMessage
    // =========================================================================

    [Fact]
    public void EntitySyncedMessage_required_fields()
    {
        var msg = new EntitySyncedMessage
        {
            EntityId = "e-1",
            EntityType = "page"
        };
        msg.EntityId.Should().Be("e-1");
        msg.EntityType.Should().Be("page");
    }

    [Fact]
    public void EntitySyncedMessage_defaults()
    {
        var msg = new EntitySyncedMessage
        {
            EntityId = "e-1",
            EntityType = "page"
        };
        msg.JsonData.Should().BeNull();
        msg.IsRemoval.Should().BeFalse();
    }

    [Fact]
    public void EntitySyncedMessage_removal()
    {
        var msg = new EntitySyncedMessage
        {
            EntityId = "e-1",
            EntityType = "task",
            IsRemoval = true
        };
        msg.IsRemoval.Should().BeTrue();
    }

    [Fact]
    public void EntitySyncedMessage_with_json_data()
    {
        var msg = new EntitySyncedMessage
        {
            EntityId = "e-1",
            EntityType = "page",
            JsonData = "{\"title\":\"Test\"}"
        };
        msg.JsonData.Should().Contain("title");
    }

    [Fact]
    public void EntitySyncedMessage_is_record_with_equality()
    {
        var a = new EntitySyncedMessage { EntityId = "e-1", EntityType = "page" };
        var b = new EntitySyncedMessage { EntityId = "e-1", EntityType = "page" };
        a.Should().Be(b);
    }

    // =========================================================================
    // IntentSignalMessage
    // =========================================================================

    [Fact]
    public void IntentSignalMessage_required_fields()
    {
        var msg = new IntentSignalMessage
        {
            SourcePluginId = "privstack.notes",
            SignalType = IntentSignalType.TextContent,
            Content = "Schedule a meeting tomorrow"
        };
        msg.SourcePluginId.Should().Be("privstack.notes");
        msg.Content.Should().Contain("meeting");
    }

    [Fact]
    public void IntentSignalMessage_defaults()
    {
        var msg = new IntentSignalMessage
        {
            SourcePluginId = "test",
            SignalType = IntentSignalType.TextContent,
            Content = "test"
        };
        msg.EntityType.Should().BeNull();
        msg.EntityId.Should().BeNull();
        msg.EntityTitle.Should().BeNull();
        msg.Metadata.Should().BeNull();
        msg.Timestamp.Should().BeCloseTo(DateTimeOffset.UtcNow, TimeSpan.FromSeconds(5));
    }

    [Fact]
    public void IntentSignalMessage_with_all_fields()
    {
        var msg = new IntentSignalMessage
        {
            SourcePluginId = "privstack.tasks",
            SignalType = IntentSignalType.EntityCreated,
            Content = "Buy groceries",
            EntityType = "task",
            EntityId = "t-123",
            EntityTitle = "Buy groceries",
            Metadata = new Dictionary<string, string> { ["priority"] = "high" }
        };
        msg.EntityType.Should().Be("task");
        msg.Metadata.Should().ContainKey("priority");
    }

    [Theory]
    [InlineData(IntentSignalType.TextContent)]
    [InlineData(IntentSignalType.EntityCreated)]
    [InlineData(IntentSignalType.EntityUpdated)]
    [InlineData(IntentSignalType.EmailReceived)]
    [InlineData(IntentSignalType.UserRequest)]
    public void IntentSignalType_all_values(IntentSignalType type)
    {
        Enum.IsDefined(type).Should().BeTrue();
    }

    // =========================================================================
    // IntentExecutedMessage
    // =========================================================================

    [Fact]
    public void IntentExecutedMessage_required_fields()
    {
        var msg = new IntentExecutedMessage
        {
            SourcePluginId = "privstack.notes",
            SourceEntityType = "page",
            SourceEntityId = "p-1",
            CreatedEntityId = "e-1",
            CreatedEntityType = "event"
        };
        msg.SourcePluginId.Should().Be("privstack.notes");
        msg.CreatedEntityId.Should().Be("e-1");
        msg.NavigationLinkType.Should().BeNull();
    }

    [Fact]
    public void IntentExecutedMessage_with_navigation()
    {
        var msg = new IntentExecutedMessage
        {
            SourcePluginId = "privstack.notes",
            SourceEntityType = "page",
            SourceEntityId = "p-1",
            CreatedEntityId = "e-1",
            CreatedEntityType = "event",
            NavigationLinkType = "calendar_event"
        };
        msg.NavigationLinkType.Should().Be("calendar_event");
    }

    // =========================================================================
    // ContentSuggestion Messages
    // =========================================================================

    [Fact]
    public void ContentSuggestionPushedMessage_construction()
    {
        var card = new ContentSuggestionCard
        {
            SuggestionId = "s-1",
            PluginId = "privstack.notes",
            Title = "Suggestion"
        };
        var msg = new ContentSuggestionPushedMessage { Card = card };
        msg.Card.SuggestionId.Should().Be("s-1");
    }

    [Fact]
    public void ContentSuggestionUpdatedMessage_required_fields()
    {
        var msg = new ContentSuggestionUpdatedMessage
        {
            SuggestionId = "s-1",
            PluginId = "privstack.notes"
        };
        msg.NewState.Should().BeNull();
        msg.NewContent.Should().BeNull();
        msg.ErrorMessage.Should().BeNull();
        msg.NewActions.Should().BeNull();
    }

    [Fact]
    public void ContentSuggestionUpdatedMessage_with_updates()
    {
        var msg = new ContentSuggestionUpdatedMessage
        {
            SuggestionId = "s-1",
            PluginId = "privstack.notes",
            NewState = ContentSuggestionState.Ready,
            NewContent = "Updated content",
            NewActions = new List<SuggestionAction>
            {
                new() { ActionId = "apply", DisplayName = "Apply" }
            }
        };
        msg.NewState.Should().Be(ContentSuggestionState.Ready);
        msg.NewActions.Should().HaveCount(1);
    }

    [Fact]
    public void ContentSuggestionRemovedMessage_construction()
    {
        var msg = new ContentSuggestionRemovedMessage
        {
            SuggestionId = "s-1",
            PluginId = "privstack.notes"
        };
        msg.SuggestionId.Should().Be("s-1");
        msg.PluginId.Should().Be("privstack.notes");
    }

    [Fact]
    public void ContentSuggestionActionRequestedMessage_construction()
    {
        var msg = new ContentSuggestionActionRequestedMessage
        {
            SuggestionId = "s-1",
            PluginId = "privstack.notes",
            ActionId = "replace"
        };
        msg.ActionId.Should().Be("replace");
    }

    [Fact]
    public void ContentSuggestionDismissedMessage_construction()
    {
        var msg = new ContentSuggestionDismissedMessage
        {
            SuggestionId = "s-1",
            PluginId = "privstack.notes"
        };
        msg.SuggestionId.Should().Be("s-1");
    }

    // =========================================================================
    // DatasetInsightRequestMessage
    // =========================================================================

    [Fact]
    public void DatasetInsightRequestMessage_construction()
    {
        var msg = new DatasetInsightRequestMessage
        {
            SuggestionId = "s-1",
            DatasetId = "ds-1",
            DatasetName = "Sales",
            Columns = new List<string> { "date", "amount", "category" },
            ColumnTypes = new List<string> { "DATE", "DECIMAL", "VARCHAR" },
            SampleRows = new List<IReadOnlyList<object?>>
            {
                new List<object?> { "2024-01-01", 100.0, "Food" }
            },
            TotalRowCount = 1000
        };
        msg.Columns.Should().HaveCount(3);
        msg.TotalRowCount.Should().Be(1000);
        msg.ChartEligibleColumns.Should().BeNull();
        msg.ChartEligibleColumnTypes.Should().BeNull();
    }

    [Fact]
    public void DatasetInsightRequestMessage_with_chart_eligible()
    {
        var msg = new DatasetInsightRequestMessage
        {
            SuggestionId = "s-1",
            DatasetId = "ds-1",
            DatasetName = "Sales",
            Columns = new List<string> { "date", "amount", "computed" },
            ColumnTypes = new List<string> { "DATE", "DECIMAL", "DECIMAL" },
            SampleRows = [],
            TotalRowCount = 500,
            ChartEligibleColumns = new List<string> { "date", "amount" },
            ChartEligibleColumnTypes = new List<string> { "DATE", "DECIMAL" }
        };
        msg.ChartEligibleColumns.Should().HaveCount(2);
    }

    // =========================================================================
    // ChartQueryErrorMessage / ChartQueryFixResult
    // =========================================================================

    [Fact]
    public void ChartQueryFixResult_construction()
    {
        var fix = new ChartQueryFixResult
        {
            ChartType = "bar",
            Title = "Sales by Category",
            XColumn = "category",
            YColumn = "total"
        };
        fix.ChartType.Should().Be("bar");
        fix.Aggregation.Should().BeNull();
        fix.GroupBy.Should().BeNull();
    }

    [Fact]
    public void ChartQueryFixResult_with_aggregation()
    {
        var fix = new ChartQueryFixResult
        {
            ChartType = "bar",
            Title = "Sales",
            XColumn = "category",
            YColumn = "amount",
            Aggregation = "sum",
            GroupBy = "category"
        };
        fix.Aggregation.Should().Be("sum");
        fix.GroupBy.Should().Be("category");
    }

    [Fact]
    public void ChartQueryErrorMessage_construction()
    {
        ChartQueryFixResult? capturedResult = null;
        var msg = new ChartQueryErrorMessage
        {
            ChartTitle = "Sales Chart",
            DatasetId = "ds-1",
            XColumn = "date",
            YColumn = "amount",
            ChartType = "line",
            ErrorMessage = "Column 'amount' must appear in GROUP BY",
            AvailableColumns = new List<string> { "date", "amount", "category" },
            ColumnTypes = new List<string> { "DATE", "DECIMAL", "VARCHAR" },
            OnFixed = result => capturedResult = result
        };
        msg.ChartTitle.Should().Be("Sales Chart");
        msg.Aggregation.Should().BeNull();
        msg.GroupBy.Should().BeNull();

        // Verify callback works
        msg.OnFixed(new ChartQueryFixResult
        {
            ChartType = "bar",
            Title = "Fixed",
            XColumn = "date",
            YColumn = "amount"
        });
        capturedResult.Should().NotBeNull();
        capturedResult!.Title.Should().Be("Fixed");
    }
}
