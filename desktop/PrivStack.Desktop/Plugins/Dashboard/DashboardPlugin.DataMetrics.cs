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

        return Task.FromResult(new PluginDataMetrics
        {
            EntityCount = tables.Sum(t => t.RowCount),
            Tables = tables,
        });
    }
}
