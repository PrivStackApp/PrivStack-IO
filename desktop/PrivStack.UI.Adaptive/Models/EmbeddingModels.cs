// ============================================================================
// File: EmbeddingModels.cs
// Description: Data models for 3D embedding space visualization.
// ============================================================================

namespace PrivStack.UI.Adaptive.Models;

public record EmbeddingPoint
{
    public string EntityId { get; init; } = string.Empty;
    public string EntityType { get; init; } = string.Empty;
    public string Title { get; init; } = string.Empty;
    public string ChunkText { get; init; } = string.Empty;
    public string LinkType { get; init; } = string.Empty;
    public string PluginId { get; init; } = string.Empty;

    // 3D world coordinates (projected from 768-dim)
    public double X { get; set; }
    public double Y { get; set; }
    public double Z { get; set; }

    // Cached screen-space coordinates (updated each frame)
    public double ScreenX { get; set; }
    public double ScreenY { get; set; }
    public double ScreenRadius { get; set; }

    // Interaction state
    public bool IsSelected { get; set; }
    public bool IsHovered { get; set; }
    public bool IsNeighborOfSelected { get; set; }
    public double NeighborSimilarity { get; set; }
}

public readonly record struct EmbeddingSimilarityEdge(
    int SourceIndex,
    int TargetIndex,
    double Similarity
);

public class EmbeddingSpaceData
{
    public List<EmbeddingPoint> Points { get; init; } = [];
    public List<EmbeddingSimilarityEdge> Edges { get; init; } = [];
    public int TotalAvailable { get; init; }
}
