// ============================================================================
// File: EmbeddingDataService.cs
// Description: Fetches RAG vector embeddings via FFI, projects 768-dim to 3D
//              via random projection, and computes K-nearest-neighbor edges
//              using cosine similarity with a minimum threshold cutoff.
// ============================================================================

using System.Runtime.InteropServices;
using System.Text.Json;
using PrivStack.UI.Adaptive.Models;
using PrivStack.UI.Adaptive.Services;
using Serilog;
using NativeLib = PrivStack.Desktop.Native.NativeLibrary;

namespace PrivStack.Desktop.Plugins.Graph.Services;

internal sealed class EmbeddingDataService
{
    private static readonly ILogger _log = Log.ForContext<EmbeddingDataService>();

    public async Task<EmbeddingSpaceData> LoadAsync(
        int maxPoints = 1000,
        double similarityThreshold = 0.85,
        int maxNeighbors = 5,
        string[]? entityTypes = null,
        CancellationToken ct = default)
    {
        // Fetch embeddings from Rust on a background thread
        var rawEntries = await Task.Run(() => FetchEmbeddings(maxPoints, entityTypes), ct);

        if (rawEntries.Count == 0)
            return new EmbeddingSpaceData();

        // Project to 3D and build points
        var points = new List<EmbeddingPoint>(rawEntries.Count);
        var rawEmbeddings = new List<double[]>(rawEntries.Count);

        foreach (var entry in rawEntries)
        {
            var embedding = entry.Embedding;
            rawEmbeddings.Add(embedding);

            var (x, y, z) = RandomProjection.Project(embedding);
            points.Add(new EmbeddingPoint
            {
                EntityId = entry.EntityId,
                EntityType = entry.EntityType,
                Title = entry.Title,
                ChunkText = entry.ChunkText,
                LinkType = entry.LinkType,
                PluginId = entry.PluginId,
                X = x,
                Y = y,
                Z = z,
            });
        }

        // Compute similarity edges on background thread
        var edges = await Task.Run(
            () => ComputeSimilarityEdges(rawEmbeddings, similarityThreshold, maxNeighbors), ct);

        return new EmbeddingSpaceData
        {
            Points = points,
            Edges = edges,
            TotalAvailable = rawEntries.Count,
        };
    }

    private static List<RawEmbeddingEntry> FetchEmbeddings(int limit, string[]? entityTypes)
    {
        var request = new { entity_types = entityTypes, limit };
        var requestJson = JsonSerializer.Serialize(request);
        _log.Information("RagFetchAll request: {Json}", requestJson);
        var resultPtr = NativeLib.RagFetchAll(requestJson);

        if (resultPtr == nint.Zero)
        {
            _log.Warning("RagFetchAll returned null pointer");
            return [];
        }

        try
        {
            var responseJson = Marshal.PtrToStringUTF8(resultPtr)
                ?? throw new InvalidOperationException("Null string from FFI");

            // Log first 500 chars of response for debugging
            _log.Information("RagFetchAll response ({Len} chars): {Preview}",
                responseJson.Length,
                responseJson.Length > 500 ? responseJson[..500] + "..." : responseJson);

            using var doc = JsonDocument.Parse(responseJson);
            var root = doc.RootElement;

            if (!root.TryGetProperty("success", out var successProp) || !successProp.GetBoolean())
            {
                var errorMsg = root.TryGetProperty("error_message", out var errProp)
                    ? errProp.GetString() : "unknown";
                _log.Warning("RagFetchAll failed: {Error}", errorMsg);
                return [];
            }

            if (!root.TryGetProperty("data", out var dataProp) || dataProp.ValueKind != JsonValueKind.Array)
            {
                _log.Warning("RagFetchAll: no data array in response");
                return [];
            }

            var entries = new List<RawEmbeddingEntry>();
            foreach (var item in dataProp.EnumerateArray())
            {
                var embProp = item.GetProperty("embedding");
                var embedding = new double[embProp.GetArrayLength()];
                int i = 0;
                foreach (var v in embProp.EnumerateArray())
                    embedding[i++] = v.GetDouble();

                entries.Add(new RawEmbeddingEntry
                {
                    EntityId = item.GetProperty("entity_id").GetString() ?? "",
                    EntityType = item.GetProperty("entity_type").GetString() ?? "",
                    PluginId = item.GetProperty("plugin_id").GetString() ?? "",
                    Title = item.GetProperty("title").GetString() ?? "",
                    LinkType = item.GetProperty("link_type").GetString() ?? "",
                    ChunkText = item.TryGetProperty("chunk_text", out var ct) ? ct.GetString() ?? "" : "",
                    Embedding = embedding,
                });
            }

            _log.Debug("Fetched {Count} embeddings for visualization", entries.Count);
            return entries;
        }
        finally
        {
            NativeLib.FreeString(resultPtr);
        }
    }

    private static List<EmbeddingSimilarityEdge> ComputeSimilarityEdges(
        List<double[]> embeddings, double threshold, int maxNeighbors)
    {
        var n = embeddings.Count;
        if (n == 0) return [];

        // Pre-compute norms
        var norms = new double[n];
        for (int i = 0; i < n; i++)
        {
            double sum = 0;
            foreach (var v in embeddings[i]) sum += v * v;
            norms[i] = Math.Sqrt(sum);
        }

        // KNN: For each point, find its top-K nearest neighbors by cosine similarity.
        // This guarantees every point has connections regardless of threshold.
        // The threshold acts as a minimum display cutoff for the weakest edges.
        var topK = new (int Index, double Sim)[maxNeighbors];
        var edgeSet = new HashSet<(int, int)>();
        var edges = new List<EmbeddingSimilarityEdge>();

        for (int i = 0; i < n; i++)
        {
            // Find top-K neighbors for point i
            int filled = 0;
            double minSim = double.MaxValue;
            int minIdx = 0;

            for (int j = 0; j < n; j++)
            {
                if (i == j) continue;
                var sim = CosineSimilarity(embeddings[i], embeddings[j], norms[i], norms[j]);

                if (filled < maxNeighbors)
                {
                    topK[filled] = (j, sim);
                    if (sim < minSim) { minSim = sim; minIdx = filled; }
                    filled++;
                }
                else if (sim > minSim)
                {
                    topK[minIdx] = (j, sim);
                    // Recompute min
                    minSim = double.MaxValue;
                    for (int k = 0; k < maxNeighbors; k++)
                    {
                        if (topK[k].Sim < minSim) { minSim = topK[k].Sim; minIdx = k; }
                    }
                }
            }

            // Add edges (deduplicated)
            for (int k = 0; k < filled; k++)
            {
                var j = topK[k].Index;
                var sim = topK[k].Sim;
                if (sim < threshold) continue; // threshold still filters the weakest

                var key = i < j ? (i, j) : (j, i);
                if (edgeSet.Add(key))
                    edges.Add(new EmbeddingSimilarityEdge(key.Item1, key.Item2, sim));
            }
        }

        return edges;
    }

    private static double CosineSimilarity(double[] a, double[] b, double normA, double normB)
    {
        if (normA == 0 || normB == 0) return 0;
        var dim = Math.Min(a.Length, b.Length);
        double dot = 0;
        for (int i = 0; i < dim; i++) dot += a[i] * b[i];
        return dot / (normA * normB);
    }

    private sealed record RawEmbeddingEntry
    {
        public required string EntityId { get; init; }
        public required string EntityType { get; init; }
        public required string PluginId { get; init; }
        public required string Title { get; init; }
        public required string LinkType { get; init; }
        public required string ChunkText { get; init; }
        public required double[] Embedding { get; init; }
    }
}
