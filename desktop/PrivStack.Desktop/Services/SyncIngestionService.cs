using System.Text.Json;
using CommunityToolkit.Mvvm.Messaging;
using PrivStack.Desktop.Models;
using PrivStack.Desktop.Native;
using PrivStack.Desktop.Services.Abstractions;
using PrivStack.Sdk.Messaging;
using Serilog;

namespace PrivStack.Desktop.Services;

/// <summary>
/// Background service that polls for incoming sync events and imports
/// entity snapshots from peers into the local stores.
/// </summary>
public sealed class SyncIngestionService : ISyncIngestionService, IDisposable
{
    private static readonly ILogger _log = Log.ForContext<SyncIngestionService>();

    private readonly ISyncService _service;
    private System.Timers.Timer? _pollTimer;
    private int _pollRunning; // re-entrancy guard
    private bool _disposed;

    public SyncIngestionService(ISyncService nativeService)
    {
        _service = nativeService;
    }

    /// <summary>
    /// Starts polling for sync events.
    /// </summary>
    public void Start()
    {
        if (_pollTimer != null) return;

        _log.Information("Starting sync ingestion polling");

        _pollTimer = new System.Timers.Timer(5000); // Poll every 5 seconds
        _pollTimer.AutoReset = true;
        _pollTimer.Elapsed += (_, _) => PollAndProcessEvents();
        _pollTimer.Start();
    }

    /// <summary>
    /// Stops polling for sync events.
    /// </summary>
    public void Stop()
    {
        _log.Information("Stopping sync ingestion polling");

        _pollTimer?.Stop();
        _pollTimer?.Dispose();
        _pollTimer = null;
    }

    private void PollAndProcessEvents()
    {
        // Skip if a previous tick is still running (mutex contention)
        if (Interlocked.CompareExchange(ref _pollRunning, 1, 0) != 0) return;
        try
        {
            if (!_service.IsSyncRunning()) return;

            var json = _service.PollSyncEvents();
            if (json == "[]") return;

            var events = JsonSerializer.Deserialize<List<SyncEventDto>>(json);
            if (events == null || events.Count == 0) return;

            foreach (var evt in events)
            {
                ProcessEvent(evt);
            }
        }
        catch (Exception ex)
        {
            _log.Error(ex, "Error polling sync events");
        }
        finally
        {
            Interlocked.Exchange(ref _pollRunning, 0);
        }
    }

    private void ProcessEvent(SyncEventDto evt)
    {
        switch (evt.EventType)
        {
            case "entity_snapshot":
                ProcessEntitySnapshot(evt);
                break;

            case "sync_completed":
                _log.Debug("Sync completed with {PeerId}: sent={Sent}, received={Received}",
                    evt.PeerId, evt.EventsSent, evt.EventsReceived);
                break;

            case "sync_failed":
                _log.Warning("Sync failed with {PeerId}: {Error}", evt.PeerId, evt.Error);
                break;

            case "peer_discovered":
                _log.Debug("Peer discovered: {PeerId} ({DeviceName})", evt.PeerId, evt.DeviceName);
                break;

            case "document_updated":
                _log.Debug("Document updated: {DocumentId}", evt.DocumentId);
                break;
        }
    }

    private void ProcessEntitySnapshot(SyncEventDto evt)
    {
        if (string.IsNullOrEmpty(evt.EntityType) || string.IsNullOrEmpty(evt.JsonData))
        {
            _log.Warning("Received entity_snapshot with missing entity_type or json_data");
            return;
        }

        _log.Information("Importing synced entity: type={EntityType}, doc={DocumentId}",
            evt.EntityType, evt.DocumentId);

        var success = _service.ImportSyncEntity(evt.EntityType, evt.JsonData);
        if (success)
        {
            _log.Information("Successfully imported {EntityType} from sync", evt.EntityType);
            BroadcastEntitySynced(evt);
        }
        else
        {
            _log.Warning("Failed to import {EntityType} from sync (doc={DocumentId})",
                evt.EntityType, evt.DocumentId);
        }
    }

    private static void BroadcastEntitySynced(SyncEventDto evt)
    {
        // Extract entity ID from the JSON data
        string? entityId = null;
        bool isRemoval = false;
        try
        {
            using var doc = JsonDocument.Parse(evt.JsonData!);
            if (doc.RootElement.TryGetProperty("id", out var idProp))
                entityId = idProp.GetString();
            if (doc.RootElement.TryGetProperty("is_trashed", out var trashedProp))
                isRemoval = trashedProp.GetBoolean();
        }
        catch
        {
            // Non-critical: message will have null EntityId, subscribers can filter
        }

        WeakReferenceMessenger.Default.Send(new EntitySyncedMessage
        {
            EntityId = entityId ?? evt.DocumentId ?? string.Empty,
            EntityType = evt.EntityType!,
            JsonData = evt.JsonData,
            IsRemoval = isRemoval,
        });
    }

    public void Dispose()
    {
        if (_disposed) return;
        _disposed = true;
        Stop();
    }
}
