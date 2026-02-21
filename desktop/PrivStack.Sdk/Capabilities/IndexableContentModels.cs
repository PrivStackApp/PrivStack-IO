namespace PrivStack.Sdk.Capabilities;

/// <summary>
/// Request parameters for <see cref="IIndexableContentProvider.GetIndexableContentAsync"/>.
/// </summary>
public sealed record IndexableContentRequest
{
    /// <summary>
    /// When set, only return entities modified after this timestamp (incremental indexing).
    /// When null, return all indexable content (full re-index).
    /// </summary>
    public DateTimeOffset? ModifiedSince { get; init; }

    /// <summary>
    /// Maximum number of chunks to return. 0 means unlimited.
    /// </summary>
    public int BatchSize { get; init; }
}

/// <summary>
/// Composite result from <see cref="IIndexableContentProvider.GetIndexableContentAsync"/>.
/// </summary>
public sealed record IndexableContentResult
{
    /// <summary>Content chunks to embed and index.</summary>
    public IReadOnlyList<ContentChunk> Chunks { get; init; } = [];

    /// <summary>Entity IDs whose vectors should be removed (deleted/trashed entities).</summary>
    public IReadOnlyList<string> DeletedEntityIds { get; init; } = [];
}

/// <summary>
/// A single chunk of text content to be embedded into the RAG vector index.
/// Each entity may produce one or more chunks (e.g., a page with multiple blocks).
/// </summary>
public sealed record ContentChunk
{
    /// <summary>Entity ID (primary key in the entity store).</summary>
    public required string EntityId { get; init; }

    /// <summary>Entity type key (e.g., "page", "task", "contact").</summary>
    public required string EntityType { get; init; }

    /// <summary>Plugin ID that owns this entity.</summary>
    public required string PluginId { get; init; }

    /// <summary>
    /// Path within the entity identifying this chunk.
    /// Use "content" for single-chunk entities, or "block/0", "block/1" for multi-chunk.
    /// </summary>
    public required string ChunkPath { get; init; }

    /// <summary>The text content to embed.</summary>
    public required string Text { get; init; }

    /// <summary>SHA-256 hash of <see cref="Text"/> for skip-if-unchanged during incremental indexing.</summary>
    public required string ContentHash { get; init; }

    /// <summary>Display title for search results.</summary>
    public required string Title { get; init; }

    /// <summary>Link type key for <see cref="IDeepLinkTarget"/> navigation (e.g., "page", "task").</summary>
    public required string LinkType { get; init; }

    /// <summary>Last modified timestamp of the source entity.</summary>
    public DateTimeOffset ModifiedAt { get; init; }
}
