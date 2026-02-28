using System.Collections.ObjectModel;
using CommunityToolkit.Mvvm.ComponentModel;
using PrivStack.Sdk.Capabilities;

namespace PrivStack.Desktop.Plugins.Dashboard.Models;

/// <summary>
/// Holds binary size info for a single installed plugin.
/// </summary>
public partial class PluginSizeInfo : ObservableObject
{
    [ObservableProperty]
    private string _pluginId = string.Empty;

    [ObservableProperty]
    private string _name = string.Empty;

    [ObservableProperty]
    private string _icon = "Package";

    [ObservableProperty]
    private long _sizeBytes;

    public string FormattedSize => SystemMetricsHelper.FormatBytes(SizeBytes);
}

/// <summary>
/// Holds data storage metrics for a single plugin (entity counts, sizes, table breakdown).
/// </summary>
public partial class PluginDataInfo : ObservableObject
{
    [ObservableProperty]
    private string _name = string.Empty;

    [ObservableProperty]
    private string _icon = string.Empty;

    [ObservableProperty]
    private int _entityCount;

    [ObservableProperty]
    private string _formattedSize = "0 B";

    [ObservableProperty]
    private bool _isExpanded;

    [ObservableProperty]
    private ObservableCollection<DataTableInfo> _tables = [];

    public string Summary
    {
        get
        {
            var parts = new List<string>();
            parts.Add($"{EntityCount:N0} {(EntityCount == 1 ? "entity" : "entities")}");
            if (!string.IsNullOrEmpty(FormattedSize) && FormattedSize != "0 B")
                parts.Add(FormattedSize);
            if (Tables.Count > 1)
                parts.Add($"{Tables.Count} tables");
            return string.Join("  ·  ", parts);
        }
    }
}

/// <summary>
/// Detailed memory breakdown for diagnostics.
/// </summary>
public sealed class MemoryDiagnostic
{
    public long WorkingSet { get; init; }
    public long GcHeap { get; init; }
    public long NativeEstimate { get; init; }
    public long Gen0 { get; init; }
    public long Gen1 { get; init; }
    public long Gen2 { get; init; }
    public long Loh { get; init; }
    public long Poh { get; init; }
    public long Fragmented { get; init; }
    public int LoadedAssemblies { get; init; }
    public int ThreadCount { get; init; }
    public int ActivePlugins { get; init; }
    public int TotalPlugins { get; init; }
    public int ActiveEntitySchemas { get; init; }

    public string FormatDetail()
    {
        var f = SystemMetricsHelper.FormatBytes;
        return $"GC: {f(Gen0)} Gen0, {f(Gen1)} Gen1, {f(Gen2)} Gen2, {f(Loh)} LOH" +
               $" | Native: ~{f(NativeEstimate)}" +
               $" | {LoadedAssemblies} assemblies, {ThreadCount} threads" +
               $" | Plugins: {ActivePlugins}/{TotalPlugins} active";
    }
}

/// <summary>
/// Shared byte formatting helper.
/// </summary>
public static class SystemMetricsHelper
{
    public static string FormatBytes(long bytes)
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
    /// Formats actual + estimated bytes into a dual display string.
    /// Both > 0 and differ: "X.X MB | ~Y.Y MB est."
    /// Equal or estimated is 0: "X.X MB"
    /// Only estimated > 0: "~X.X MB"
    /// </summary>
    public static string FormatBytesWithEstimate(long actual, long estimated)
    {
        if (actual > 0 && estimated > 0 && actual != estimated)
            return $"{FormatBytes(actual)} | ~{FormatBytes(estimated)} est.";
        if (actual > 0)
            return FormatBytes(actual);
        if (estimated > 0)
            return $"~{FormatBytes(estimated)}";
        return "0 B";
    }
}
