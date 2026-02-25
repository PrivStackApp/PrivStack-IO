namespace PrivStack.Desktop.Tests.Sdk;

using PrivStack.Sdk.Capabilities;

public class DataSourceModelsTests
{
    // =========================================================================
    // DataSourceEntry
    // =========================================================================

    [Fact]
    public void DataSourceEntry_construction()
    {
        var entry = new DataSourceEntry
        {
            Id = "ds-1",
            Name = "Transactions",
            PluginId = "privstack.finance",
            QueryKey = "finance.transactions"
        };
        entry.Id.Should().Be("ds-1");
        entry.Name.Should().Be("Transactions");
        entry.RowCount.Should().Be(0);
        entry.ColumnCount.Should().Be(0);
        entry.Detail.Should().BeNull();
        entry.SupportsChart.Should().BeFalse();
    }

    [Fact]
    public void DataSourceEntry_with_all_fields()
    {
        var entry = new DataSourceEntry
        {
            Id = "ds-2",
            Name = "Tasks",
            PluginId = "privstack.tasks",
            QueryKey = "tasks.all",
            RowCount = 150,
            ColumnCount = 8,
            Detail = "150 tasks",
            SupportsChart = true
        };
        entry.RowCount.Should().Be(150);
        entry.ColumnCount.Should().Be(8);
        entry.Detail.Should().Be("150 tasks");
        entry.SupportsChart.Should().BeTrue();
    }

    // =========================================================================
    // DataSourceGroup
    // =========================================================================

    [Fact]
    public void DataSourceGroup_construction()
    {
        var group = new DataSourceGroup
        {
            Name = "Finance",
            Entries = new List<DataSourceEntry>
            {
                new() { Id = "1", Name = "Transactions", PluginId = "test", QueryKey = "q1" },
                new() { Id = "2", Name = "Accounts", PluginId = "test", QueryKey = "q2" }
            }
        };
        group.Name.Should().Be("Finance");
        group.Icon.Should().BeNull();
        group.Entries.Should().HaveCount(2);
    }

    [Fact]
    public void DataSourceGroup_with_icon()
    {
        var group = new DataSourceGroup
        {
            Name = "Tasks",
            Icon = "CheckSquare",
            Entries = []
        };
        group.Icon.Should().Be("CheckSquare");
        group.Entries.Should().BeEmpty();
    }

    // =========================================================================
    // AiSuggestionModels
    // =========================================================================

    [Fact]
    public void ContentSuggestionCard_defaults()
    {
        var card = new ContentSuggestionCard
        {
            SuggestionId = "s-1",
            PluginId = "privstack.notes",
            Title = "AI Suggestion"
        };
        card.State.Should().Be(ContentSuggestionState.Loading);
        card.Summary.Should().BeNull();
        card.Content.Should().BeNull();
        card.Actions.Should().BeEmpty();
        card.ErrorMessage.Should().BeNull();
        card.UserPromptLabel.Should().BeNull();
    }

    [Fact]
    public void ContentSuggestionCard_with_actions()
    {
        var card = new ContentSuggestionCard
        {
            SuggestionId = "s-2",
            PluginId = "privstack.notes",
            Title = "Rewrite",
            State = ContentSuggestionState.Ready,
            Content = "Rewritten text here",
            Actions = new List<SuggestionAction>
            {
                new() { ActionId = "replace", DisplayName = "Replace", IsPrimary = true },
                new() { ActionId = "dismiss", DisplayName = "Dismiss", IsDestructive = true }
            }
        };
        card.State.Should().Be(ContentSuggestionState.Ready);
        card.Actions.Should().HaveCount(2);
        card.Actions[0].IsPrimary.Should().BeTrue();
        card.Actions[1].IsDestructive.Should().BeTrue();
    }

    [Fact]
    public void SuggestionAction_defaults()
    {
        var action = new SuggestionAction
        {
            ActionId = "apply",
            DisplayName = "Apply"
        };
        action.IsPrimary.Should().BeFalse();
        action.IsDestructive.Should().BeFalse();
    }

    [Theory]
    [InlineData(ContentSuggestionState.Loading)]
    [InlineData(ContentSuggestionState.Ready)]
    [InlineData(ContentSuggestionState.Error)]
    [InlineData(ContentSuggestionState.Applied)]
    public void ContentSuggestionState_all_values(ContentSuggestionState state)
    {
        var card = new ContentSuggestionCard
        {
            SuggestionId = "s",
            PluginId = "p",
            Title = "t",
            State = state
        };
        card.State.Should().Be(state);
    }
}
