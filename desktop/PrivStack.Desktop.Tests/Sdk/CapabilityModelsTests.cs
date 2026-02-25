namespace PrivStack.Desktop.Tests.Sdk;

using PrivStack.Sdk.Capabilities;

public class CapabilityModelsTests
{
    // =========================================================================
    // ConnectionRequirement
    // =========================================================================

    [Fact]
    public void ConnectionRequirement_construction()
    {
        var req = new ConnectionRequirement(
            Provider: "google",
            ProviderDisplayName: "Google",
            RequiredScopes: new List<string> { "https://mail.google.com/", "https://www.googleapis.com/auth/calendar" }
        );
        req.Provider.Should().Be("google");
        req.ProviderDisplayName.Should().Be("Google");
        req.RequiredScopes.Should().HaveCount(2);
    }

    [Fact]
    public void ConnectionRequirement_is_record_with_equality()
    {
        var scopes = new List<string> { "scope1" };
        var a = new ConnectionRequirement("ms", "Microsoft", scopes);
        var b = new ConnectionRequirement("ms", "Microsoft", scopes);
        a.Should().Be(b);
    }

    // =========================================================================
    // DatasetColumnDef
    // =========================================================================

    [Fact]
    public void DatasetColumnDef_construction()
    {
        var col = new DatasetColumnDef
        {
            Name = "email",
            ColumnType = "VARCHAR"
        };
        col.Name.Should().Be("email");
        col.ColumnType.Should().Be("VARCHAR");
    }

    // =========================================================================
    // DatasetView / ViewConfig
    // =========================================================================

    [Fact]
    public void DatasetView_defaults()
    {
        var view = new DatasetView
        {
            Id = "v-1",
            DatasetId = "ds-1",
            Name = "Active Users",
            Config = new ViewConfig()
        };
        view.IsDefault.Should().BeFalse();
        view.SortOrder.Should().Be(0);
    }

    [Fact]
    public void ViewConfig_with_filters_and_sorts()
    {
        var cfg = new ViewConfig
        {
            VisibleColumns = new List<string> { "name", "email" },
            Filters = new List<ViewFilter>
            {
                new() { Column = "active", Operator = "eq", Value = "true" }
            },
            Sorts = new List<ViewSort>
            {
                new() { Column = "name", Direction = "asc" }
            },
            GroupBy = "department"
        };
        cfg.VisibleColumns.Should().HaveCount(2);
        cfg.Filters.Should().HaveCount(1);
        cfg.Sorts.Should().HaveCount(1);
        cfg.GroupBy.Should().Be("department");
    }

    // =========================================================================
    // GroupedAggregateQuery
    // =========================================================================

    [Fact]
    public void GroupedAggregateQuery_construction()
    {
        var q = new GroupedAggregateQuery
        {
            DatasetId = "ds-1",
            XColumn = "month",
            YColumn = "revenue",
            GroupColumn = "region"
        };
        q.Aggregation.Should().BeNull();
        q.FilterText.Should().BeNull();
    }
}
