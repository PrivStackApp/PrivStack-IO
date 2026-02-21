namespace PrivStack.Sdk.Messaging;

/// <summary>
/// Sent by the Notes plugin when a chart's dataset query fails (e.g., missing aggregation
/// on a GROUP BY column). The shell's <c>DatasetInsightOrchestrator</c> subscribes, sends
/// the error context to the AI for a corrected chart config, and invokes the callback.
/// </summary>
public sealed record ChartQueryErrorMessage
{
    /// <summary>Chart title for AI context.</summary>
    public required string ChartTitle { get; init; }

    /// <summary>Dataset identifier the chart queries against.</summary>
    public required string DatasetId { get; init; }

    /// <summary>Original X-axis column.</summary>
    public required string XColumn { get; init; }

    /// <summary>Original Y-axis column.</summary>
    public required string YColumn { get; init; }

    /// <summary>Original aggregation function (sum, avg, count, etc.), if any.</summary>
    public string? Aggregation { get; init; }

    /// <summary>Original GROUP BY column, if any.</summary>
    public string? GroupBy { get; init; }

    /// <summary>Chart type (bar, line, pie, etc.).</summary>
    public required string ChartType { get; init; }

    /// <summary>The DuckDB error message from the failed query.</summary>
    public required string ErrorMessage { get; init; }

    /// <summary>All column names available in the dataset.</summary>
    public required IReadOnlyList<string> AvailableColumns { get; init; }

    /// <summary>Column types parallel to <see cref="AvailableColumns"/>.</summary>
    public required IReadOnlyList<string> ColumnTypes { get; init; }

    /// <summary>
    /// Callback invoked with the corrected chart config, or null if the AI could not fix it.
    /// </summary>
    public required Action<ChartQueryFixResult?> OnFixed { get; init; }
}

/// <summary>
/// Corrected chart parameters returned by the AI after a failed chart query.
/// </summary>
public sealed record ChartQueryFixResult
{
    public required string ChartType { get; init; }
    public required string Title { get; init; }
    public required string XColumn { get; init; }
    public required string YColumn { get; init; }
    public string? Aggregation { get; init; }
    public string? GroupBy { get; init; }
}
