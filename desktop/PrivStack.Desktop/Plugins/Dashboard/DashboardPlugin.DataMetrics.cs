using PrivStack.Services.Diagnostics;
using PrivStack.Sdk.Capabilities;

namespace PrivStack.Desktop.Plugins.Dashboard;

public sealed partial class DashboardPlugin : IDataMetricsProvider
{
    string IDataMetricsProvider.ProviderName => "Dashboard";
    string IDataMetricsProvider.ProviderIcon => "LayoutDashboard";

    Task<PluginDataMetrics> IDataMetricsProvider.GetMetricsAsync(CancellationToken ct)
    {
        var tables = new List<DataTableInfo>();

        // Plugin catalog entries (always present — Dashboard always loads these)
        var catalogCount = ViewModel?.AllPlugins.Count ?? 0;
        tables.Add(new DataTableInfo
        {
            Name = "Plugin Catalog",
            EntityType = "plugin_catalog",
            RowCount = catalogCount,
            BackingMode = "runtime",
            PluginId = Metadata.Id,
        });

        // Subsystem tracker entries
        var tracker = SubsystemTracker.Instance;
        if (tracker != null)
        {
            var snapshots = tracker.GetSnapshots();
            tables.Add(new DataTableInfo
            {
                Name = "Subsystems",
                EntityType = "subsystem",
                RowCount = snapshots.Length,
                BackingMode = "runtime",
                PluginId = Metadata.Id,
            });
        }

        // Data metrics cache (plugin data items on the Data tab)
        var dataItemCount = ViewModel?.PluginDataItems.Count ?? 0;
        if (dataItemCount > 0)
        {
            tables.Add(new DataTableInfo
            {
                Name = "Data Metrics Cache",
                EntityType = "data_metric",
                RowCount = dataItemCount,
                BackingMode = "runtime",
                PluginId = Metadata.Id,
            });
        }

        return Task.FromResult(new PluginDataMetrics
        {
            EntityCount = tables.Sum(t => t.RowCount),
            Tables = tables,
        });
    }
}
