using System.Collections.Concurrent;
using System.Runtime.InteropServices;
using System.Text.Json;
using System.Threading.Channels;
using CommunityToolkit.Mvvm.Messaging;
using PrivStack.Services.Plugin;
using PrivStack.Sdk.Capabilities;
using PrivStack.Sdk.Messaging;
using PrivStack.Services.Abstractions;
using Serilog;
using NativeLib = PrivStack.Services.Native.NativeLibrary;

namespace PrivStack.Services.AI;

/// <summary>
/// Background worker that maintains the RAG vector index.
/// Listens for EntitySyncedMessage to incrementally re-index changed entities,
/// and performs a full index on startup if the embedding model is available.
///
/// Uses a batch-coalescing debounce: incoming requests are queued and a single
/// timer resets on each arrival. When the timer fires, all queued requests are
/// deduplicated and dispatched as a single batch — calling each provider at most
/// once per batch instead of once per entity.
/// </summary>
internal sealed class RagIndexService : IRecipient<EntitySyncedMessage>, IDisposable
{
    private static readonly ILogger _log = Log.ForContext<RagIndexService>();
    private const int QueueCapacity = 20;
    private static readonly TimeSpan DebounceInterval = TimeSpan.FromMilliseconds(500);

    private readonly IEmbeddingService _embeddingService;
    private readonly IPluginRegistry _pluginRegistry;
    private readonly IAppSettingsService _appSettings;
    private readonly Channel<List<RagIndexRequest>> _channel;
    private readonly ConcurrentQueue<RagIndexRequest> _pendingQueue = new();
    private readonly object _timerLock = new();
    private readonly CancellationTokenSource _disposeCts = new();
    private System.Threading.Timer? _coalescingTimer;
    private Task? _consumerTask;
    private bool _disposed;

    public RagIndexService(IEmbeddingService embeddingService, IPluginRegistry pluginRegistry, IAppSettingsService appSettings)
    {
        _embeddingService = embeddingService;
        _pluginRegistry = pluginRegistry;
        _appSettings = appSettings;
        _channel = Channel.CreateBounded<List<RagIndexRequest>>(
            new BoundedChannelOptions(QueueCapacity)
            {
                FullMode = BoundedChannelFullMode.DropOldest,
                SingleReader = true,
            });

        WeakReferenceMessenger.Default.Register<EntitySyncedMessage>(this);
        _consumerTask = Task.Run(() => ConsumeAsync(_disposeCts.Token));

        // Auto-initialize if model is already downloaded (deferred to allow plugins to activate first)
        _ = Task.Run(async () =>
        {
            // Wait for plugins to activate before attempting full index
            await Task.Delay(TimeSpan.FromSeconds(5), _disposeCts.Token);
            try
            {
                if (!_appSettings.Settings.AiEnabled)
                {
                    _log.Debug("AI is disabled — skipping embedding model auto-initialization");
                    return;
                }

                await _embeddingService.InitializeAsync(_disposeCts.Token);
                if (_embeddingService.IsReady)
                {
                    _log.Information("Embedding model available on startup — starting full index");
                    await StartFullIndexAsync(_disposeCts.Token);
                }
            }
            catch (OperationCanceledException) { }
            catch (Exception ex)
            {
                _log.Debug(ex, "Auto-initialization of RAG index deferred (model not yet downloaded)");
            }
        });
    }

    // ── IRecipient<EntitySyncedMessage> ──

    public void Receive(EntitySyncedMessage message)
    {
        if (!_embeddingService.IsReady) return;

        _pendingQueue.Enqueue(new RagIndexRequest
        {
            EntityId = message.EntityId,
            EntityType = message.EntityType,
            IsRemoval = message.IsRemoval,
        });

        // Reset the coalescing timer — extends the window while changes keep arriving
        lock (_timerLock)
        {
            _coalescingTimer?.Dispose();
            _coalescingTimer = new System.Threading.Timer(
                _ => FlushPendingQueue(),
                null,
                DebounceInterval,
                Timeout.InfiniteTimeSpan);
        }
    }

    private void FlushPendingQueue()
    {
        var batch = new Dictionary<string, RagIndexRequest>();
        while (_pendingQueue.TryDequeue(out var req))
        {
            // Keep the last request per EntityId (dedup rapid updates)
            batch[req.EntityId] = req;
        }

        if (batch.Count > 0)
        {
            _log.Debug("RAG batch coalesced: {Count} unique entities from queue", batch.Count);
            _channel.Writer.TryWrite(batch.Values.ToList());
        }
    }

    /// <summary>
    /// Triggers a full re-index of all plugin content. Skips unchanged chunks via content hash.
    /// </summary>
    public async Task StartFullIndexAsync(CancellationToken ct = default)
    {
        if (!_embeddingService.IsReady)
        {
            _log.Debug("Full index skipped — embedding service not ready");
            return;
        }

        _log.Information("Starting full RAG index");

        var providers = _pluginRegistry.GetCapabilityProviders<IIndexableContentProvider>();
        if (providers.Count == 0)
        {
            _log.Debug("No IIndexableContentProvider implementations found");
            return;
        }

        // Load existing hashes for skip-if-unchanged
        var existingHashes = LoadExistingHashes();

        var totalChunksIndexed = 0;
        var totalChunksSkipped = 0;

        foreach (var provider in providers)
        {
            try
            {
                var result = await provider.GetIndexableContentAsync(
                    new IndexableContentRequest { BatchSize = 0 }, ct);

                // Handle deletions
                foreach (var deletedId in result.DeletedEntityIds)
                {
                    DeleteVectors(deletedId);
                }

                // Process chunks
                foreach (var chunk in result.Chunks)
                {
                    ct.ThrowIfCancellationRequested();

                    var hashKey = $"{chunk.EntityId}:{chunk.ChunkPath}";
                    if (existingHashes.TryGetValue(hashKey, out var existingHash) &&
                        existingHash == chunk.ContentHash)
                    {
                        totalChunksSkipped++;
                        continue;
                    }

                    await IndexChunkAsync(chunk, ct);
                    totalChunksIndexed++;
                }
            }
            catch (OperationCanceledException)
            {
                throw;
            }
            catch (Exception ex)
            {
                _log.Error(ex, "Failed to index content from provider {Provider}", provider.GetType().Name);
            }
        }

        _log.Information("Full RAG index complete: {Indexed} indexed, {Skipped} unchanged",
            totalChunksIndexed, totalChunksSkipped);
    }

    // ── Background consumer ──

    private async Task ConsumeAsync(CancellationToken ct)
    {
        await foreach (var batch in _channel.Reader.ReadAllAsync(ct))
        {
            try
            {
                await ProcessBatchAsync(batch, ct);
            }
            catch (OperationCanceledException) when (ct.IsCancellationRequested)
            {
                return;
            }
            catch (Exception ex)
            {
                _log.Error(ex, "Failed to process RAG index batch ({Count} entities)", batch.Count);
            }
        }
    }

    private async Task ProcessBatchAsync(List<RagIndexRequest> batch, CancellationToken ct)
    {
        if (!_embeddingService.IsReady) return;

        // Split into removals and upserts
        var removals = batch.Where(r => r.IsRemoval).ToList();
        var upserts = batch.Where(r => !r.IsRemoval).ToList();

        // Handle removals
        foreach (var removal in removals)
        {
            DeleteVectors(removal.EntityId);
        }

        if (upserts.Count == 0) return;

        // Build lookup of entity IDs we need to index, grouped by entity type
        var targetIds = new HashSet<string>(upserts.Select(u => u.EntityId));

        // Load existing hashes ONCE for the entire batch
        var existingHashes = LoadExistingHashes();

        var providers = _pluginRegistry.GetCapabilityProviders<IIndexableContentProvider>();
        var matchedIds = new HashSet<string>();

        foreach (var provider in providers)
        {
            if (matchedIds.Count >= targetIds.Count)
                break; // All entity IDs accounted for — skip remaining providers

            try
            {
                var result = await provider.GetIndexableContentAsync(
                    new IndexableContentRequest { ModifiedSince = null, BatchSize = 0 }, ct);

                var relevantChunks = result.Chunks
                    .Where(c => targetIds.Contains(c.EntityId))
                    .ToList();

                if (relevantChunks.Count == 0) continue;

                foreach (var chunk in relevantChunks)
                {
                    matchedIds.Add(chunk.EntityId);

                    var hashKey = $"{chunk.EntityId}:{chunk.ChunkPath}";
                    if (existingHashes.TryGetValue(hashKey, out var existingHash) &&
                        existingHash == chunk.ContentHash)
                    {
                        continue;
                    }

                    await IndexChunkAsync(chunk, ct);
                }
            }
            catch (Exception ex)
            {
                _log.Debug(ex, "Provider {Provider} failed during batch processing",
                    provider.GetType().Name);
            }
        }
    }

    private async Task IndexChunkAsync(ContentChunk chunk, CancellationToken ct)
    {
        if (string.IsNullOrWhiteSpace(chunk.Text)) return;

        var embedding = await _embeddingService.EmbedAsync(chunk.Text, EmbeddingTaskType.Document, ct);

        var now = DateTimeOffset.UtcNow.ToUnixTimeMilliseconds();
        var request = new
        {
            entity_id = chunk.EntityId,
            chunk_path = chunk.ChunkPath,
            plugin_id = chunk.PluginId,
            entity_type = chunk.EntityType,
            content_hash = chunk.ContentHash,
            dim = embedding.Length,
            embedding,
            title = chunk.Title,
            link_type = chunk.LinkType,
            indexed_at = now,
            chunk_text = chunk.Text,
        };

        var requestJson = JsonSerializer.Serialize(request);
        var resultPtr = NativeLib.RagUpsert(requestJson);
        if (resultPtr != nint.Zero)
            NativeLib.FreeString(resultPtr);
    }

    private static void DeleteVectors(string entityId)
    {
        var requestJson = JsonSerializer.Serialize(new { entity_id = entityId });
        var resultPtr = NativeLib.RagDelete(requestJson);
        if (resultPtr != nint.Zero)
            NativeLib.FreeString(resultPtr);
    }

    private static Dictionary<string, string> LoadExistingHashes()
    {
        var requestJson = JsonSerializer.Serialize(new { entity_types = (string[]?)null });
        var resultPtr = NativeLib.RagGetHashes(requestJson);
        if (resultPtr == nint.Zero) return new Dictionary<string, string>();

        try
        {
            var responseJson = Marshal.PtrToStringUTF8(resultPtr) ?? "{}";
            using var doc = JsonDocument.Parse(responseJson);
            var root = doc.RootElement;

            if (!root.TryGetProperty("success", out var successProp) || !successProp.GetBoolean())
                return new Dictionary<string, string>();

            if (!root.TryGetProperty("data", out var dataProp) || dataProp.ValueKind != JsonValueKind.Array)
                return new Dictionary<string, string>();

            var hashes = new Dictionary<string, string>();
            foreach (var item in dataProp.EnumerateArray())
            {
                var entityId = item.GetProperty("entity_id").GetString() ?? "";
                var chunkPath = item.GetProperty("chunk_path").GetString() ?? "";
                var contentHash = item.GetProperty("content_hash").GetString() ?? "";
                hashes[$"{entityId}:{chunkPath}"] = contentHash;
            }
            return hashes;
        }
        finally
        {
            NativeLib.FreeString(resultPtr);
        }
    }

    public void Dispose()
    {
        if (_disposed) return;
        _disposed = true;

        WeakReferenceMessenger.Default.Unregister<EntitySyncedMessage>(this);
        _disposeCts.Cancel();
        _channel.Writer.Complete();

        lock (_timerLock)
        {
            _coalescingTimer?.Dispose();
        }

        _disposeCts.Dispose();
    }
}
