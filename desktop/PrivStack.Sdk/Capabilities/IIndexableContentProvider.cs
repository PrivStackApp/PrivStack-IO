namespace PrivStack.Sdk.Capabilities;

/// <summary>
/// Capability interface for plugins that contribute searchable content to the RAG vector index.
/// The shell calls this during full re-index and incremental updates to embed plugin content
/// for semantic search across the knowledge base.
/// </summary>
public interface IIndexableContentProvider
{
    /// <summary>
    /// Returns content chunks for embedding into the RAG vector index.
    /// Supports incremental indexing via <see cref="IndexableContentRequest.ModifiedSince"/>.
    /// </summary>
    Task<IndexableContentResult> GetIndexableContentAsync(
        IndexableContentRequest request, CancellationToken ct = default);
}
