using System.Collections.Concurrent;
using System.Runtime.InteropServices;
using System.Security.Cryptography;
using System.Text;
using System.Text.Json;
using System.Threading.Channels;
using CommunityToolkit.Mvvm.Messaging;
using PrivStack.Desktop.Services.Plugin;
using PrivStack.Sdk.Capabilities;
using PrivStack.Sdk.Messaging;
using Serilog;
using NativeLib = PrivStack.Desktop.Native.NativeLibrary;

namespace PrivStack.Desktop.Services.AI;

/// <summary>
/// Background worker that maintains the RAG vector index.
/// Listens for EntitySyncedMessage to incrementally re-index changed entities,
/// and performs a full index on startup if the embedding model is available.
/// Follows the Channel&lt;T&gt; bounded worker pattern from <see cref="IntentEngine"/>.
/// </summary>
internal sealed class RagIndexService : IRecipient<EntitySyncedMessage>, IDisposable
{
    private static readonly ILogger _log = Log.ForContext<RagIndexService>();
    private const int QueueCapacity = 50;
    private static readonly TimeSpan DebounceInterval = TimeSpan.FromMilliseconds(500);

    private readonly EmbeddingService _embeddingService;
    private readonly IPluginRegistry _pluginRegistry;
    private readonly Channel<RagIndexRequest> _channel;
    private readonly ConcurrentDictionary<string, Timer> _debounceTimers = new();
    private readonly CancellationTokenSource _disposeCts = new();
    private Task? _consumerTask;
    private bool _disposed;

    public RagIndexService(EmbeddingService embeddingService, IPluginRegistry pluginRegistry)
    {
        _embeddingService = embeddingService;
        _pluginRegistry = pluginRegistry;
        _channel = Channel.CreateBounded<RagIndexRequest>(
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

        var request = new RagIndexRequest
        {
            EntityId = message.EntityId,
            EntityType = message.EntityType,
            IsRemoval = message.IsRemoval,
        };

        // Debounce: if we get rapid-fire updates for the same entity, only process the last one
        var key = message.EntityId;
        if (_debounceTimers.TryGetValue(key, out var existingTimer))
        {
            existingTimer.Dispose();
        }

        var timer = new Timer(_ =>
        {
            _debounceTimers.TryRemove(key, out _);
            _channel.Writer.TryWrite(request);
        }, null, DebounceInterval, Timeout.InfiniteTimeSpan);

        _debounceTimers[key] = timer;
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
        await foreach (var request in _channel.Reader.ReadAllAsync(ct))
        {
            try
            {
                if (request.IsRemoval)
                {
                    DeleteVectors(request.EntityId);
                    continue;
                }

                await ProcessEntityAsync(request, ct);
            }
            catch (OperationCanceledException) when (ct.IsCancellationRequested)
            {
                return;
            }
            catch (Exception ex)
            {
                _log.Error(ex, "Failed to process RAG index request for {EntityId}", request.EntityId);
            }
        }
    }

    private async Task ProcessEntityAsync(RagIndexRequest request, CancellationToken ct)
    {
        if (!_embeddingService.IsReady) return;

        // Find the provider that handles this entity type
        var providers = _pluginRegistry.GetCapabilityProviders<IIndexableContentProvider>();

        foreach (var provider in providers)
        {
            try
            {
                var result = await provider.GetIndexableContentAsync(
                    new IndexableContentRequest { ModifiedSince = null, BatchSize = 0 }, ct);

                var chunks = result.Chunks
                    .Where(c => c.EntityId == request.EntityId)
                    .ToList();

                if (chunks.Count == 0) continue;

                // Load existing hashes for this entity
                var existingHashes = LoadExistingHashes();

                foreach (var chunk in chunks)
                {
                    var hashKey = $"{chunk.EntityId}:{chunk.ChunkPath}";
                    if (existingHashes.TryGetValue(hashKey, out var existingHash) &&
                        existingHash == chunk.ContentHash)
                    {
                        continue;
                    }

                    await IndexChunkAsync(chunk, ct);
                }

                return; // Found the provider, done
            }
            catch (Exception ex)
            {
                _log.Debug(ex, "Provider {Provider} did not contain entity {EntityId}",
                    provider.GetType().Name, request.EntityId);
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

        foreach (var timer in _debounceTimers.Values)
            timer.Dispose();
        _debounceTimers.Clear();

        _disposeCts.Dispose();
    }
}
