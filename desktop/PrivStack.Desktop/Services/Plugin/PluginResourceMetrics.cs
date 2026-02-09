using System.Text.Json.Serialization;

namespace PrivStack.Desktop.Services.Plugin;

/// <summary>
/// Resource usage metrics for a plugin sandbox.
/// Used for monitoring and display in the plugin management UI.
/// </summary>
public sealed record PluginResourceMetrics
{
    /// <summary>
    /// Plugin ID for this metrics entry.
    /// </summary>
    [JsonPropertyName("plugin_id")]
    public string? PluginId { get; init; }

    /// <summary>
    /// Memory currently used by the plugin in bytes.
    /// </summary>
    [JsonPropertyName("memory_used_bytes")]
    public long MemoryUsedBytes { get; init; }

    /// <summary>
    /// Maximum memory allowed for this plugin in bytes.
    /// </summary>
    [JsonPropertyName("memory_limit_bytes")]
    public long MemoryLimitBytes { get; init; }

    /// <summary>
    /// Memory usage as a ratio (0.0 to 1.0).
    /// </summary>
    [JsonPropertyName("memory_usage_ratio")]
    public double MemoryUsageRatio { get; init; }

    /// <summary>
    /// Fuel consumed in the last plugin call.
    /// </summary>
    [JsonPropertyName("fuel_consumed_last_call")]
    public long FuelConsumedLastCall { get; init; }

    /// <summary>
    /// Fuel budget per call.
    /// </summary>
    [JsonPropertyName("fuel_budget_per_call")]
    public long FuelBudgetPerCall { get; init; }

    /// <summary>
    /// Average fuel consumed over the last 1000 calls.
    /// </summary>
    [JsonPropertyName("fuel_average_last_1000")]
    public long FuelAverageLast1000 { get; init; }

    /// <summary>
    /// Peak fuel consumed across all tracked calls.
    /// </summary>
    [JsonPropertyName("fuel_peak")]
    public long FuelPeak { get; init; }

    /// <summary>
    /// Number of calls in the fuel history (max 1000).
    /// </summary>
    [JsonPropertyName("fuel_history_count")]
    public int FuelHistoryCount { get; init; }

    /// <summary>
    /// Number of entities owned by this plugin.
    /// </summary>
    [JsonPropertyName("entity_count")]
    public int EntityCount { get; init; }

    /// <summary>
    /// Estimated disk usage in bytes for plugin entities.
    /// </summary>
    [JsonPropertyName("disk_usage_bytes")]
    public long DiskUsageBytes { get; init; }

    // =========================================================
    // Computed display properties
    // =========================================================

    /// <summary>
    /// Memory usage display string (e.g., "28.8 MB / 64 MB").
    /// </summary>
    public string MemoryDisplay => $"{FormatBytes(MemoryUsedBytes)} / {FormatBytes(MemoryLimitBytes)}";

    /// <summary>
    /// Memory usage as a percentage (0 to 100).
    /// </summary>
    public double MemoryUsagePercent => MemoryUsageRatio * 100;

    /// <summary>
    /// Disk usage display string (e.g., "142 entities (2.3 MB)").
    /// </summary>
    public string DiskDisplay => $"{EntityCount:N0} entities ({FormatBytes(DiskUsageBytes)})";

    /// <summary>
    /// Fuel usage as a percentage (0 to 100) based on last call.
    /// </summary>
    public double FuelUsagePercent => FuelBudgetPerCall > 0
        ? (double)FuelConsumedLastCall / FuelBudgetPerCall * 100
        : 0;

    /// <summary>
    /// Fuel usage display string for last call (e.g., "Last: 45m/1b (4.5%)").
    /// </summary>
    public string FuelDisplay => FuelBudgetPerCall > 0
        ? $"Last: {FormatNumber(FuelConsumedLastCall)}/{FormatNumber(FuelBudgetPerCall)} ({FuelUsagePercent:F1}%)"
        : "N/A";

    /// <summary>
    /// Fuel average display string over last 1000 calls (e.g., "Avg (500): 25k/1b (2.5%)").
    /// </summary>
    public string FuelAverageDisplay => FuelHistoryCount > 0 && FuelBudgetPerCall > 0
        ? $"Avg ({FuelHistoryCount}): {FormatNumber(FuelAverageLast1000)}/{FormatNumber(FuelBudgetPerCall)} ({(double)FuelAverageLast1000 / FuelBudgetPerCall * 100:F1}%)"
        : "No history";

    /// <summary>
    /// Fuel peak display string (e.g., "Peak: 100m/1b (10%)").
    /// </summary>
    public string FuelPeakDisplay => FuelBudgetPerCall > 0
        ? $"Peak: {FormatNumber(FuelPeak)}/{FormatNumber(FuelBudgetPerCall)} ({(double)FuelPeak / FuelBudgetPerCall * 100:F1}%)"
        : "N/A";

    /// <summary>
    /// Formats bytes into a human-readable string (KB, MB, GB).
    /// </summary>
    private static string FormatBytes(long bytes)
    {
        const long KB = 1024;
        const long MB = KB * 1024;
        const long GB = MB * 1024;

        return bytes switch
        {
            >= GB => $"{bytes / (double)GB:F1} GB",
            >= MB => $"{bytes / (double)MB:F1} MB",
            >= KB => $"{bytes / (double)KB:F1} KB",
            _ => $"{bytes} B"
        };
    }

    /// <summary>
    /// Formats large numbers into a human-readable string (k, m, b).
    /// </summary>
    private static string FormatNumber(long value)
    {
        const long K = 1_000;
        const long M = 1_000_000;
        const long B = 1_000_000_000;

        return value switch
        {
            >= B => $"{value / (double)B:F0}b",
            >= M => $"{value / (double)M:F0}m",
            >= K => $"{value / (double)K:F0}k",
            _ => $"{value}"
        };
    }
}
