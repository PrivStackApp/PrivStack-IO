namespace PrivStack.Desktop.Services.AI;

/// <summary>
/// Internal request queued for the RAG index background worker.
/// </summary>
internal sealed record RagIndexRequest
{
    /// <summary>Entity ID to re-index (or delete).</summary>
    public required string EntityId { get; init; }

    /// <summary>Entity type key.</summary>
    public required string EntityType { get; init; }

    /// <summary>True if the entity was deleted and vectors should be removed.</summary>
    public bool IsRemoval { get; init; }
}
