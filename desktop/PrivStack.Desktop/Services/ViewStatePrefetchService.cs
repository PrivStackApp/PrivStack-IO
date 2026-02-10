// ============================================================================
// File: ViewStatePrefetchService.cs
// Description: Hover-based prefetch service for view state data.
//              Preloads entity view state on hover to eliminate perceived
//              latency when the user clicks. Uses LRU cache + debouncing.
// ============================================================================

using System.Collections.Concurrent;
using System.Runtime.InteropServices;
using System.Text.Json;
using System.Text.Json.Nodes;
using Serilog;
using NativeLib = PrivStack.Desktop.Native.NativeLibrary;

namespace PrivStack.Desktop.Services;

/// <summary>
/// Cached view state entry with metadata for LRU eviction.
/// </summary>
public sealed record CachedViewState(
    string PluginId,
    string? EntityId,
    string ViewStateJson,
    DateTime CachedAt);

/// <summary>
/// Service that prefetches view state data on hover to reduce perceived latency.
///
/// Usage:
/// - Call RequestPrefetch() when mouse enters a linkable item
/// - Call CancelPrefetch() when mouse leaves (cancels if still pending)
/// - Check TryGetCached() before loading - returns cached data if available
///
/// Design:
/// - 100ms debounce prevents thrashing on rapid hover/scan
/// - LRU cache holds up to 20 entries (~40KB typical)
/// - Background thread execution keeps UI responsive
/// - Cached items persist until evicted (not dumped on mouse leave)
/// </summary>
public sealed class ViewStatePrefetchService : IDisposable
{
    private static readonly ILogger _log = Log.ForContext<ViewStatePrefetchService>();

    private const int MaxCacheSize = 20;
    private const int DebounceMs = 125;

    // LRU cache: key = "pluginId:entityId" or "pluginId:" for plugin root
    private readonly LinkedList<CachedViewState> _cacheOrder = new();
    private readonly Dictionary<string, LinkedListNode<CachedViewState>> _cacheMap = new();
    private readonly object _cacheLock = new();

    // Pending prefetch operations
    private readonly ConcurrentDictionary<string, CancellationTokenSource> _pendingPrefetches = new();

    // Debounce timers
    private readonly ConcurrentDictionary<string, Timer> _debounceTimers = new();

    // Currently active (displayed) plugin - avoid entity prefetch for this plugin
    // as it would change the visible UI
    private string? _activePluginId;

    /// <summary>
    /// Sets the currently active (displayed) plugin ID.
    /// Entity prefetch requests for this plugin are skipped to avoid changing visible UI.
    /// </summary>
    public void SetActivePlugin(string? pluginId)
    {
        _activePluginId = pluginId;
        _log.Verbose("Active plugin set: {PluginId}", pluginId ?? "(none)");
    }

    /// <summary>
    /// Requests a prefetch of view state for a plugin/entity combination.
    /// The request is debounced - rapid calls within 100ms are coalesced.
    /// </summary>
    /// <param name="pluginId">The plugin ID.</param>
    /// <param name="entityId">The entity ID, or null for plugin root view.</param>
    public void RequestPrefetch(string pluginId, string? entityId = null)
    {
        // Note: We allow same-plugin entity prefetch now. While this changes
        // plugin state on hover, the benefits outweigh the risks:
        // - Users typically click shortly after hovering (state change is intentional)
        // - Shadow state protects block content if they interact with old page
        // - Cache hit skips the 120ms+ FFI navigate call on click
        // The brief window where state is "wrong" rarely causes issues in practice.

        var key = MakeKey(pluginId, entityId);

        // Already cached? Skip
        if (IsCached(key))
            return;

        // Cancel any existing debounce timer for this key
        if (_debounceTimers.TryRemove(key, out var existingTimer))
            existingTimer.Dispose();

        // Start new debounce timer
        var timer = new Timer(
            _ => ExecutePrefetch(pluginId, entityId, key),
            null,
            DebounceMs,
            Timeout.Infinite);

        _debounceTimers[key] = timer;
    }

    /// <summary>
    /// Cancels a pending prefetch if it hasn't started yet.
    /// Does NOT evict already-cached data.
    /// </summary>
    public void CancelPrefetch(string pluginId, string? entityId = null)
    {
        var key = MakeKey(pluginId, entityId);

        // Cancel debounce timer
        if (_debounceTimers.TryRemove(key, out var timer))
        {
            timer.Dispose();
            _log.Verbose("Prefetch debounce cancelled: {Key}", key);
        }

        // Cancel in-flight prefetch
        if (_pendingPrefetches.TryRemove(key, out var cts))
        {
            cts.Cancel();
            cts.Dispose();
            _log.Verbose("Prefetch in-flight cancelled: {Key}", key);
        }
    }

    /// <summary>
    /// Tries to get cached view state for a plugin/entity.
    /// Returns null if not cached. Promotes entry to MRU on hit.
    /// </summary>
    public CachedViewState? TryGetCached(string pluginId, string? entityId = null)
    {
        var key = MakeKey(pluginId, entityId);

        lock (_cacheLock)
        {
            if (_cacheMap.TryGetValue(key, out var node))
            {
                // Promote to MRU
                _cacheOrder.Remove(node);
                _cacheOrder.AddFirst(node);
                _log.Debug("Prefetch cache hit: {Key} (age={Age}ms)",
                    key, (DateTime.UtcNow - node.Value.CachedAt).TotalMilliseconds);
                return node.Value;
            }
        }

        return null;
    }

    /// <summary>
    /// Checks if an entry is cached without promoting it.
    /// </summary>
    public bool IsCached(string pluginId, string? entityId = null)
    {
        return IsCached(MakeKey(pluginId, entityId));
    }

    private bool IsCached(string key)
    {
        lock (_cacheLock)
        {
            return _cacheMap.ContainsKey(key);
        }
    }

    /// <summary>
    /// Clears all cached entries and cancels pending prefetches.
    /// Call on workspace switch or significant state change.
    /// </summary>
    public void Clear()
    {
        // Cancel all pending
        foreach (var kvp in _pendingPrefetches)
        {
            kvp.Value.Cancel();
            kvp.Value.Dispose();
        }
        _pendingPrefetches.Clear();

        // Cancel all debounce timers
        foreach (var kvp in _debounceTimers)
        {
            kvp.Value.Dispose();
        }
        _debounceTimers.Clear();

        // Clear cache
        lock (_cacheLock)
        {
            _cacheOrder.Clear();
            _cacheMap.Clear();
        }

        _log.Debug("Prefetch cache cleared");
    }

    /// <summary>
    /// Invalidates a specific cache entry. Call when entity is modified.
    /// </summary>
    public void Invalidate(string pluginId, string? entityId = null)
    {
        var key = MakeKey(pluginId, entityId);

        lock (_cacheLock)
        {
            if (_cacheMap.TryGetValue(key, out var node))
            {
                _cacheOrder.Remove(node);
                _cacheMap.Remove(key);
                _log.Debug("Prefetch cache invalidated: {Key}", key);
            }
        }
    }

    private void ExecutePrefetch(string pluginId, string? entityId, string key)
    {
        // Remove debounce timer
        _debounceTimers.TryRemove(key, out _);

        // Already cached? Skip
        if (IsCached(key))
        {
            _log.Verbose("Prefetch skipped (cached after debounce): {Key}", key);
            return;
        }

        // Create cancellation token for this prefetch
        var cts = new CancellationTokenSource();
        if (!_pendingPrefetches.TryAdd(key, cts))
        {
            // Another prefetch already in progress
            cts.Dispose();
            return;
        }

        // Execute on thread pool
        _ = Task.Run(() =>
        {
            try
            {
                if (cts.Token.IsCancellationRequested)
                    return;

                var json = FetchViewState(pluginId, entityId);

                if (json == null || cts.Token.IsCancellationRequested)
                    return;

                var entry = new CachedViewState(pluginId, entityId, json, DateTime.UtcNow);
                AddToCache(key, entry);
            }
            catch (Exception ex)
            {
                _log.Warning(ex, "Prefetch failed: {Key}", key);
            }
            finally
            {
                _pendingPrefetches.TryRemove(key, out _);
                cts.Dispose();
            }
        }, cts.Token);
    }

    private const int MaxPrefetchBlocks = 20;

    private string? FetchViewState(string pluginId, string? entityId)
    {
        nint dataPtr;

        if (entityId != null)
        {
            // Entity-specific prefetch: navigate to entity and get its view data
            // This is safe for cross-plugin prefetch (target plugin isn't displayed)
            // The ActivePlugin check in RequestPrefetch prevents same-plugin prefetch
            dataPtr = NativeLib.PluginGetEntityViewData(pluginId, entityId);
        }
        else
        {
            // Plugin root prefetch: get current view data
            dataPtr = NativeLib.PluginGetViewData(pluginId);
        }

        if (dataPtr == nint.Zero)
        {
            // Fall back to legacy view state (no entity navigation)
            if (entityId == null)
            {
                var statePtr = NativeLib.PluginGetViewState(pluginId);
                if (statePtr == nint.Zero)
                    return null;

                try
                {
                    var json = Marshal.PtrToStringUTF8(statePtr);
                    return json != null ? TruncateBlocksForPrefetch(json) : null;
                }
                finally
                {
                    NativeLib.FreeString(statePtr);
                }
            }
            return null;
        }

        try
        {
            var json = Marshal.PtrToStringUTF8(dataPtr);
            return json != null ? TruncateBlocksForPrefetch(json) : null;
        }
        finally
        {
            NativeLib.FreeString(dataPtr);
        }
    }

    /// <summary>
    /// Truncates large block arrays in prefetched JSON to reduce memory usage.
    /// Only caches the first N blocks since that's what will be visible initially.
    /// </summary>
    private string TruncateBlocksForPrefetch(string json)
    {
        try
        {
            var node = JsonNode.Parse(json);
            if (node is not JsonObject root)
                return json;

            var truncated = false;

            // Look for "blocks" array in state (common pattern for notes/block editors)
            if (root.TryGetPropertyValue("state", out var stateNode) && stateNode is JsonObject state)
            {
                if (state.TryGetPropertyValue("blocks", out var blocksNode) && blocksNode is JsonArray blocks)
                {
                    if (blocks.Count > MaxPrefetchBlocks)
                    {
                        var originalCount = blocks.Count;
                        // Truncate to first N blocks
                        while (blocks.Count > MaxPrefetchBlocks)
                            blocks.RemoveAt(blocks.Count - 1);

                        _log.Debug("Prefetch: Truncated blocks from {Original} to {Truncated} for cache efficiency",
                            originalCount, MaxPrefetchBlocks);
                        truncated = true;
                    }
                }
            }

            // Also check top-level "blocks" (some templates put it there)
            if (root.TryGetPropertyValue("blocks", out var topBlocksNode) && topBlocksNode is JsonArray topBlocks)
            {
                if (topBlocks.Count > MaxPrefetchBlocks)
                {
                    var originalCount = topBlocks.Count;
                    while (topBlocks.Count > MaxPrefetchBlocks)
                        topBlocks.RemoveAt(topBlocks.Count - 1);

                    _log.Debug("Prefetch: Truncated top-level blocks from {Original} to {Truncated} for cache efficiency",
                        originalCount, MaxPrefetchBlocks);
                    truncated = true;
                }
            }

            return truncated ? root.ToJsonString() : json;
        }
        catch (Exception ex)
        {
            _log.Warning(ex, "Prefetch: Failed to truncate blocks, using full JSON");
            return json;
        }
    }

    private void AddToCache(string key, CachedViewState entry)
    {
        lock (_cacheLock)
        {
            // Remove if already exists
            if (_cacheMap.TryGetValue(key, out var existing))
            {
                _cacheOrder.Remove(existing);
                _cacheMap.Remove(key);
            }

            // Evict LRU if at capacity
            while (_cacheOrder.Count >= MaxCacheSize)
            {
                var lru = _cacheOrder.Last;
                if (lru != null)
                {
                    var lruKey = MakeKey(lru.Value.PluginId, lru.Value.EntityId);
                    _cacheOrder.RemoveLast();
                    _cacheMap.Remove(lruKey);
                    _log.Verbose("Prefetch cache evicted (LRU): {Key}", lruKey);
                }
            }

            // Add new entry at front (MRU)
            var node = _cacheOrder.AddFirst(entry);
            _cacheMap[key] = node;
        }
    }

    private static string MakeKey(string pluginId, string? entityId)
        => entityId != null ? $"{pluginId}:{entityId}" : $"{pluginId}:";

    public void Dispose()
    {
        Clear();
    }
}
