namespace PrivStack.Desktop.Services.AI;

/// <summary>
/// Determines the task prefix prepended to text before embedding.
/// nomic-embed-text-v1.5 uses "search_document: " for indexing and "search_query: " for queries.
/// </summary>
public enum EmbeddingTaskType
{
    /// <summary>Content being indexed (prefix: "search_document: ").</summary>
    Document,

    /// <summary>User search query (prefix: "search_query: ").</summary>
    Query
}
