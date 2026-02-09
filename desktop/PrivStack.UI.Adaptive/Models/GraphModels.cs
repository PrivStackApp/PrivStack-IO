// ============================================================================
// File: GraphModels.cs
// Description: Data models for the localized knowledge graph view.
//              Ported from PrivStack.Plugin.Graph to FluidUI.
// ============================================================================

using System.Text.Json.Serialization;

namespace PrivStack.UI.Adaptive.Models;

public record GraphNode
{
    [JsonPropertyName("id")] public string Id { get; init; } = string.Empty;
    [JsonPropertyName("title")] public string Title { get; init; } = string.Empty;
    [JsonPropertyName("node_type")] public string NodeType { get; init; } = "note";
    [JsonPropertyName("link_type")] public string LinkType { get; init; } = "note";
    public double X { get; set; }
    public double Y { get; set; }
    public double Vx { get; set; }
    public double Vy { get; set; }
    [JsonPropertyName("link_count")] public int LinkCount { get; set; }
    [JsonPropertyName("depth")] public int Depth { get; set; }
    public bool IsPinned { get; set; }
    public bool IsDragging { get; set; }
    public bool IsHovered { get; set; }
    public bool IsSelected { get; set; }
    public double Radius => (6 + Math.Log(LinkCount + 1) * 4) * Math.Max(0.6, 1.0 - Depth * 0.15);
}

public record GraphEdge
{
    [JsonPropertyName("source_id")] public string SourceId { get; init; } = string.Empty;
    [JsonPropertyName("target_id")] public string TargetId { get; init; } = string.Empty;
    [JsonPropertyName("edge_type")] public string EdgeType { get; init; } = "link";
    public bool IsHighlighted { get; set; }
}

public class GraphData
{
    public Dictionary<string, GraphNode> Nodes { get; init; } = new();
    public List<GraphEdge> Edges { get; init; } = [];

    public static GraphData FromJson(
        IReadOnlyList<System.Text.Json.JsonElement> nodeElements,
        IReadOnlyList<System.Text.Json.JsonElement> edgeElements)
    {
        var data = new GraphData();

        foreach (var el in nodeElements)
        {
            var id = el.GetStringProp("id") ?? "";
            if (string.IsNullOrEmpty(id)) continue;

            data.Nodes[id] = new GraphNode
            {
                Id = id,
                Title = el.GetStringProp("title") ?? id,
                NodeType = el.GetStringProp("node_type") ?? "note",
                LinkType = el.GetStringProp("link_type") ?? "note",
                LinkCount = el.GetIntProp("link_count", 0),
                Depth = el.GetIntProp("depth", 0),
            };
        }

        foreach (var el in edgeElements)
        {
            var sourceId = el.GetStringProp("source_id") ?? "";
            var targetId = el.GetStringProp("target_id") ?? "";
            if (string.IsNullOrEmpty(sourceId) || string.IsNullOrEmpty(targetId)) continue;
            if (!data.Nodes.ContainsKey(sourceId) || !data.Nodes.ContainsKey(targetId)) continue;

            data.Edges.Add(new GraphEdge
            {
                SourceId = sourceId,
                TargetId = targetId,
                EdgeType = el.GetStringProp("edge_type") ?? "link",
            });
        }

        // Update link counts from edges
        foreach (var edge in data.Edges)
        {
            if (data.Nodes.TryGetValue(edge.SourceId, out var src))
                src.LinkCount = Math.Max(src.LinkCount, 1);
            if (data.Nodes.TryGetValue(edge.TargetId, out var tgt))
                tgt.LinkCount = Math.Max(tgt.LinkCount, 1);
        }

        return data;
    }
}
