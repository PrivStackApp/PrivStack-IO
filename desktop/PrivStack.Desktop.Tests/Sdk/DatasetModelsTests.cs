namespace PrivStack.Desktop.Tests.Sdk;

using System.Text.Json;
using PrivStack.Sdk.Capabilities;

public class DatasetModelsTests
{
    // =========================================================================
    // DatasetId
    // =========================================================================

    [Fact]
    public void DatasetId_ToString()
    {
        var id = new DatasetId { Value = "abc-123" };
        id.ToString().Should().Be("abc-123");
    }

    [Fact]
    public void DatasetId_implicit_string_conversion()
    {
        DatasetId id = new() { Value = "abc-123" };
        string s = id;
        s.Should().Be("abc-123");
    }

    [Fact]
    public void DatasetId_serializes_as_plain_string()
    {
        var id = new DatasetId { Value = "abc-123" };
        var json = JsonSerializer.Serialize(id);
        json.Should().Be("\"abc-123\"");
    }

    [Fact]
    public void DatasetId_deserializes_from_string()
    {
        var id = JsonSerializer.Deserialize<DatasetId>("\"abc-123\"");
        id!.Value.Should().Be("abc-123");
    }

    [Fact]
    public void DatasetId_deserializes_from_legacy_object()
    {
        var id = JsonSerializer.Deserialize<DatasetId>("{\"0\": \"abc-123\"}");
        id!.Value.Should().Be("abc-123");
    }

    // =========================================================================
    // DatasetInfo
    // =========================================================================

    [Fact]
    public void DatasetInfo_defaults()
    {
        var info = new DatasetInfo
        {
            Id = new DatasetId { Value = "ds-1" },
            Name = "Sales"
        };
        info.SourceFileName.Should().BeNull();
        info.RowCount.Should().Be(0);
        info.Columns.Should().BeEmpty();
        info.Category.Should().BeNull();
    }

    // =========================================================================
    // DatasetColumnInfo
    // =========================================================================

    [Fact]
    public void DatasetColumnInfo_construction()
    {
        var col = new DatasetColumnInfo
        {
            Name = "amount",
            ColumnType = "DECIMAL",
            Ordinal = 2
        };
        col.Name.Should().Be("amount");
        col.ColumnType.Should().Be("DECIMAL");
        col.Ordinal.Should().Be(2);
    }

    // =========================================================================
    // DatasetQuery
    // =========================================================================

    [Fact]
    public void DatasetQuery_defaults()
    {
        var q = new DatasetQuery { DatasetId = "ds-1" };
        q.Page.Should().Be(0);
        q.PageSize.Should().Be(100);
        q.FilterText.Should().BeNull();
        q.SortColumn.Should().BeNull();
        q.SortDesc.Should().BeFalse();
    }

    // =========================================================================
    // DatasetQueryResult
    // =========================================================================

    [Fact]
    public void DatasetQueryResult_defaults()
    {
        var r = new DatasetQueryResult();
        r.Columns.Should().BeEmpty();
        r.ColumnTypes.Should().BeEmpty();
        r.Rows.Should().BeEmpty();
        r.TotalCount.Should().Be(0);
    }

    // =========================================================================
    // DatasetRelation
    // =========================================================================

    [Fact]
    public void DatasetRelation_defaults()
    {
        var rel = new DatasetRelation
        {
            Id = "r-1",
            SourceDatasetId = "ds-1",
            SourceColumn = "customer_id",
            TargetDatasetId = "ds-2",
            TargetColumn = "id"
        };
        rel.RelationType.Should().Be("many_to_one");
    }

    // =========================================================================
    // ViewConfig, ViewFilter, ViewSort
    // =========================================================================

    [Fact]
    public void ViewConfig_defaults()
    {
        var cfg = new ViewConfig();
        cfg.VisibleColumns.Should().BeNull();
        cfg.Filters.Should().BeEmpty();
        cfg.Sorts.Should().BeEmpty();
        cfg.GroupBy.Should().BeNull();
    }

    [Fact]
    public void ViewFilter_construction()
    {
        var f = new ViewFilter { Column = "status", Operator = "eq", Value = "active" };
        f.Column.Should().Be("status");
    }

    [Fact]
    public void ViewSort_construction()
    {
        var s = new ViewSort { Column = "created_at", Direction = "desc" };
        s.Direction.Should().Be("desc");
    }

    // =========================================================================
    // AggregateQuery / AggregateQueryResult
    // =========================================================================

    [Fact]
    public void AggregateQuery_defaults()
    {
        var q = new AggregateQuery
        {
            DatasetId = "ds-1",
            XColumn = "date",
            YColumn = "amount"
        };
        q.Aggregation.Should().BeNull();
        q.GroupBy.Should().BeNull();
        q.FilterText.Should().BeNull();
    }

    [Fact]
    public void AggregateQueryResult_defaults()
    {
        var r = new AggregateQueryResult();
        r.Labels.Should().BeEmpty();
        r.Values.Should().BeEmpty();
    }

    // =========================================================================
    // GroupedAggregateQuery / GroupedAggregateResult
    // =========================================================================

    [Fact]
    public void GroupedAggregateResult_defaults()
    {
        var r = new GroupedAggregateResult();
        r.XLabels.Should().BeEmpty();
        r.Series.Should().BeEmpty();
    }

    [Fact]
    public void AggregateSeriesResult_construction()
    {
        var s = new AggregateSeriesResult
        {
            SeriesName = "Category A",
            Labels = new List<string> { "Jan", "Feb" },
            Values = new List<double> { 100.0, 200.0 }
        };
        s.Labels.Should().HaveCount(2);
        s.Values.Should().HaveCount(2);
    }

    // =========================================================================
    // MutationResult
    // =========================================================================

    [Fact]
    public void MutationResult_defaults()
    {
        var r = new MutationResult();
        r.AffectedRows.Should().Be(0);
        r.StatementType.Should().BeEmpty();
        r.Committed.Should().BeFalse();
        r.Preview.Should().BeNull();
    }

    // =========================================================================
    // SavedQueryInfo
    // =========================================================================

    [Fact]
    public void SavedQueryInfo_defaults()
    {
        var q = new SavedQueryInfo
        {
            Id = "q-1",
            Name = "Active Users",
            Sql = "SELECT * FROM users WHERE active"
        };
        q.Description.Should().BeNull();
        q.IsView.Should().BeFalse();
    }

    // =========================================================================
    // SqlExecutionResponse converter
    // =========================================================================

    [Fact]
    public void SqlExecutionResponse_deserializes_error()
    {
        var json = """{"error": "syntax error"}""";
        var resp = JsonSerializer.Deserialize<SqlExecutionResponse>(json);
        resp!.Error.Should().Be("syntax error");
        resp.Query.Should().BeNull();
        resp.Mutation.Should().BeNull();
    }

    [Fact]
    public void SqlExecutionResponse_deserializes_query()
    {
        var json = """{"type":"query","columns":["id"],"column_types":["INTEGER"],"rows":[],"total_count":0,"page":0,"page_size":100}""";
        var resp = JsonSerializer.Deserialize<SqlExecutionResponse>(json);
        resp!.Query.Should().NotBeNull();
        resp.Query!.Columns.Should().Contain("id");
    }

    [Fact]
    public void SqlExecutionResponse_deserializes_mutation()
    {
        var json = """{"type":"mutation","affected_rows":5,"statement_type":"INSERT","committed":true}""";
        var resp = JsonSerializer.Deserialize<SqlExecutionResponse>(json);
        resp!.Mutation.Should().NotBeNull();
        resp.Mutation!.AffectedRows.Should().Be(5);
        resp.Mutation.Committed.Should().BeTrue();
    }
}
