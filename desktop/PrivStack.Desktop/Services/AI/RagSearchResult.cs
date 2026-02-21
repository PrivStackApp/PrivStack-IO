namespace PrivStack.Desktop.Services.AI;

/// <summary>
/// A single result from a RAG semantic search query.
/// </summary>
public sealed record RagSearchResult
{
    public required string EntityId { get; init; }
    public required string EntityType { get; init; }
    public required string PluginId { get; init; }
    public required string ChunkPath { get; init; }
    public required string Title { get; init; }
    public required string LinkType { get; init; }
    public required double Score { get; init; }
    public string ChunkText { get; init; } = "";
}
