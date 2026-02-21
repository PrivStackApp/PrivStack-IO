using System.Runtime.InteropServices;
using System.Text.Json;
using Serilog;
using NativeLib = PrivStack.Desktop.Native.NativeLibrary;

namespace PrivStack.Desktop.Services.AI;

/// <summary>
/// Provides semantic search over the RAG vector index.
/// Embeds the query string, calls the Rust FFI for cosine similarity search,
/// and returns ranked results.
/// </summary>
internal sealed class RagSearchService
{
    private static readonly ILogger _log = Log.ForContext<RagSearchService>();
    private readonly EmbeddingService _embeddingService;

    public RagSearchService(EmbeddingService embeddingService)
    {
        _embeddingService = embeddingService;
    }

    public bool IsReady => _embeddingService.IsReady;

    /// <summary>
    /// Semantic search across all indexed plugin content.
    /// </summary>
    public async Task<IReadOnlyList<RagSearchResult>> SearchAsync(
        string query, int limit = 20, string[]? entityTypes = null, CancellationToken ct = default)
    {
        if (!_embeddingService.IsReady)
        {
            _log.Debug("RAG search skipped — embedding service not ready");
            return [];
        }

        if (string.IsNullOrWhiteSpace(query))
            return [];

        // 1. Embed the query
        var queryEmbedding = await _embeddingService.EmbedAsync(query, EmbeddingTaskType.Query, ct);

        // 2. Call Rust FFI
        var request = new
        {
            embedding = queryEmbedding,
            limit,
            entity_types = entityTypes,
        };

        var requestJson = JsonSerializer.Serialize(request);
        var resultPtr = NativeLib.RagSearch(requestJson);

        if (resultPtr == nint.Zero)
        {
            _log.Warning("RAG search returned null pointer");
            return [];
        }

        try
        {
            var responseJson = Marshal.PtrToStringUTF8(resultPtr)
                ?? throw new InvalidOperationException("Null string from FFI");

            using var doc = JsonDocument.Parse(responseJson);
            var root = doc.RootElement;

            if (!root.TryGetProperty("success", out var successProp) || !successProp.GetBoolean())
            {
                var errorMsg = root.TryGetProperty("error_message", out var errProp) ? errProp.GetString() : "unknown";
                _log.Warning("RAG search failed: {Error}", errorMsg);
                return [];
            }

            if (!root.TryGetProperty("data", out var dataProp) || dataProp.ValueKind != JsonValueKind.Array)
                return [];

            var results = new List<RagSearchResult>();
            foreach (var item in dataProp.EnumerateArray())
            {
                results.Add(new RagSearchResult
                {
                    EntityId = item.GetProperty("entity_id").GetString() ?? "",
                    EntityType = item.GetProperty("entity_type").GetString() ?? "",
                    PluginId = item.GetProperty("plugin_id").GetString() ?? "",
                    ChunkPath = item.GetProperty("chunk_path").GetString() ?? "",
                    Title = item.GetProperty("title").GetString() ?? "",
                    LinkType = item.GetProperty("link_type").GetString() ?? "",
                    Score = item.GetProperty("score").GetDouble(),
                    ChunkText = item.TryGetProperty("chunk_text", out var chunkTextProp) ? chunkTextProp.GetString() ?? "" : "",
                });
            }

            return results;
        }
        finally
        {
            NativeLib.FreeString(resultPtr);
        }
    }
}
