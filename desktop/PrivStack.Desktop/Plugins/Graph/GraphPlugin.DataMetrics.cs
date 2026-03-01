using PrivStack.Sdk.Capabilities;

namespace PrivStack.Desktop.Plugins.Graph;

public sealed partial class GraphPlugin : IDataMetricsProvider
{
    string IDataMetricsProvider.ProviderName => "Graph";
    string IDataMetricsProvider.ProviderIcon => "Graph";

    Task<PluginDataMetrics> IDataMetricsProvider.GetMetricsAsync(CancellationToken ct)
    {
        var tables = new List<DataTableInfo>();
        int nodeCount = 0;
        int edgeCount = 0;

        if (ViewModel?.GraphData is { } graph)
        {
            nodeCount = graph.NodeCount;
            edgeCount = graph.EdgeCount;
        }

        // Always report — even with 0 counts the graph plugin is loaded and allocated
        tables.Add(new DataTableInfo
        {
            Name = "Graph Nodes",
            EntityType = "graph_node",
            RowCount = nodeCount,
            BackingMode = "runtime",
            PluginId = Metadata.Id,
        });

        tables.Add(new DataTableInfo
        {
            Name = "Graph Edges",
            EntityType = "graph_edge",
            RowCount = edgeCount,
            BackingMode = "runtime",
            PluginId = Metadata.Id,
        });

        return Task.FromResult(new PluginDataMetrics
        {
            EntityCount = nodeCount + edgeCount,
            Tables = tables,
        });
    }
}
