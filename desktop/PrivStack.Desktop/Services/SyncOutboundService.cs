using System.Collections.Concurrent;
using PrivStack.Desktop.Native;
using PrivStack.Desktop.Services.Abstractions;
using Serilog;

namespace PrivStack.Desktop.Services;

/// <summary>
/// Bridges local entity mutations to the P2P sync engine.
/// Immediately registers entities for sync (cheap HashSet insert in Rust),
/// then debounces snapshot recordings to avoid flooding during rapid edits.
/// </summary>
internal sealed class SyncOutboundService : ISyncOutboundService, IDisposable
{
    private static readonly ILogger _log = Log.ForContext<SyncOutboundService>();
    private const int DebounceMs = 2000;

    private readonly ISyncService _syncService;
    private readonly ConcurrentDictionary<string, DebounceEntry> _pending = new();
    private bool _disposed;

    public SyncOutboundService(ISyncService syncService)
    {
        _syncService = syncService;
    }

    public void NotifyEntityChanged(string entityId, string entityType, string? payload)
    {
        if (_disposed) return;

        // Register for sync immediately (idempotent HashSet insert, very cheap)
        try
        {
            _syncService.ShareDocumentForSync(entityId);
        }
        catch (Exception ex)
        {
            _log.Warning(ex, "Failed to share document {EntityId} for sync", entityId);
            return;
        }

        // No payload means we can't record a snapshot (e.g., Delete returns no body)
        if (string.IsNullOrEmpty(payload)) return;

        // Debounce the snapshot recording: reset timer if entity already pending
        _pending.AddOrUpdate(
            entityId,
            _ => CreateEntry(entityId, entityType, payload),
            (_, existing) =>
            {
                existing.EntityType = entityType;
                existing.Payload = payload;
                existing.Timer.Change(DebounceMs, Timeout.Infinite);
                return existing;
            });
    }

    public void CancelAll()
    {
        foreach (var kvp in _pending)
        {
            if (_pending.TryRemove(kvp.Key, out var entry))
            {
                entry.Timer.Dispose();
            }
        }
    }

    private DebounceEntry CreateEntry(string entityId, string entityType, string payload)
    {
        var entry = new DebounceEntry
        {
            EntityType = entityType,
            Payload = payload,
        };
        entry.Timer = new Timer(_ => OnDebounceElapsed(entityId), null, DebounceMs, Timeout.Infinite);
        return entry;
    }

    private void OnDebounceElapsed(string entityId)
    {
        if (!_pending.TryRemove(entityId, out var entry)) return;

        try
        {
            entry.Timer.Dispose();
            _syncService.RecordSyncSnapshot(entityId, entry.EntityType, entry.Payload);
            _log.Debug("Recorded sync snapshot for {EntityType} {EntityId}", entry.EntityType, entityId);
        }
        catch (Exception ex)
        {
            _log.Warning(ex, "Failed to record sync snapshot for {EntityId}", entityId);
        }
    }

    public void Dispose()
    {
        if (_disposed) return;
        _disposed = true;
        CancelAll();
    }

    private sealed class DebounceEntry
    {
        public Timer Timer { get; set; } = null!;
        public string EntityType { get; set; } = string.Empty;
        public string Payload { get; set; } = string.Empty;
    }
}
