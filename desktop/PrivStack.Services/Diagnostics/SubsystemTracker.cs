using System.Collections.Concurrent;
using System.Runtime.InteropServices;
using NativeLib = PrivStack.Services.Native.NativeLibrary;

namespace PrivStack.Services.Diagnostics;

/// <summary>
/// Tracks per-subsystem memory usage and task counts across the application.
/// Uses AsyncLocal to tag managed threads/tasks with a subsystem ID and
/// measures allocation rates via GC.GetAllocatedBytesForCurrentThread().
/// </summary>
public sealed class SubsystemTracker
{
    /// <summary>
    /// Global singleton instance. Set during service registration.
    /// Allows lightweight instrumentation without constructor injection.
    /// </summary>
    public static SubsystemTracker? Instance { get; set; }

    private static readonly AsyncLocal<string?> CurrentSubsystem = new();

    private readonly ConcurrentDictionary<string, SubsystemState> _states = new();

    private const int RateSampleCount = 5;

    /// <summary>
    /// Register a subsystem for tracking. If the subsystem was already auto-created
    /// (from a scope or RunTagged call before explicit registration), updates the
    /// display name and category to the authoritative values.
    /// </summary>
    public void Register(string id, string displayName, string category)
    {
        _states.AddOrUpdate(id,
            new SubsystemState(displayName, category),
            (_, existing) =>
            {
                existing.DisplayName = displayName;
                existing.Category = category;
                return existing;
            });
    }

    /// <summary>
    /// Run an async action tagged with the given subsystem ID.
    /// Tracks active task count and managed allocation delta.
    /// For long-running tasks, call ReportAllocations() periodically from within
    /// the action to update the allocation counter (since the finally block
    /// only runs on task completion).
    /// </summary>
    public Task RunTagged(string subsystemId, Func<Task> action)
    {
        return Task.Run(async () =>
        {
            CurrentSubsystem.Value = subsystemId;
            var state = GetOrCreateState(subsystemId);
            Interlocked.Increment(ref state.ActiveTaskCount);
            var startBytes = GC.GetAllocatedBytesForCurrentThread();

            try
            {
                await action();
            }
            finally
            {
                var delta = GC.GetAllocatedBytesForCurrentThread() - startBytes;
                Interlocked.Add(ref state.ManagedAllocBytes, delta);
                Interlocked.Decrement(ref state.ActiveTaskCount);
                CurrentSubsystem.Value = null;
            }
        });
    }

    /// <summary>
    /// Run a synchronous action tagged with the given subsystem ID.
    /// </summary>
    public Task RunTagged(string subsystemId, Action action)
    {
        return Task.Run(() =>
        {
            CurrentSubsystem.Value = subsystemId;
            var state = GetOrCreateState(subsystemId);
            Interlocked.Increment(ref state.ActiveTaskCount);
            var startBytes = GC.GetAllocatedBytesForCurrentThread();

            try
            {
                action();
            }
            finally
            {
                var delta = GC.GetAllocatedBytesForCurrentThread() - startBytes;
                Interlocked.Add(ref state.ManagedAllocBytes, delta);
                Interlocked.Decrement(ref state.ActiveTaskCount);
                CurrentSubsystem.Value = null;
            }
        });
    }

    /// <summary>
    /// Enter a subsystem scope for tagging synchronous code blocks.
    /// The returned IDisposable restores the previous tag on dispose.
    /// </summary>
    public IDisposable EnterScope(string subsystemId)
    {
        var state = GetOrCreateState(subsystemId);
        Interlocked.Increment(ref state.ActiveTaskCount);
        var prev = CurrentSubsystem.Value;
        CurrentSubsystem.Value = subsystemId;
        var startBytes = GC.GetAllocatedBytesForCurrentThread();
        return new ScopeGuard(state, prev, startBytes);
    }

    /// <summary>
    /// Merge native (Rust) subsystem memory counters into the tracker.
    /// Called periodically from the Dashboard timer.
    /// </summary>
    public void MergeNativeCounters(NativeSubsystemEntry[] entries)
    {
        foreach (var entry in entries)
        {
            if (entry.Id >= SubsystemDefinitions.RustIdMap.Length) continue;
            var csharpId = SubsystemDefinitions.RustIdMap[entry.Id];
            var state = GetOrCreateState(csharpId);
            Interlocked.Exchange(ref state.NativeBytes, entry.Bytes);
            Interlocked.Exchange(ref state.NativeAllocs, (long)entry.Allocs);
        }
    }

    /// <summary>
    /// Update rolling allocation rate samples. Call once per second.
    /// </summary>
    public void UpdateRateSamples()
    {
        foreach (var (_, state) in _states)
        {
            var current = Interlocked.Read(ref state.ManagedAllocBytes);
            var delta = current - state.LastSampleBytes;
            state.LastSampleBytes = current;

            state.RateSamples[state.RateSampleIndex % RateSampleCount] = delta;
            state.RateSampleIndex++;

            var count = Math.Min(state.RateSampleIndex, RateSampleCount);
            long sum = 0;
            for (var i = 0; i < count; i++)
                sum += state.RateSamples[i];
            Interlocked.Exchange(ref state.SmoothedAllocRate, count > 0 ? sum / count : 0);
        }
    }

    /// <summary>
    /// Get a snapshot of all tracked subsystems.
    /// </summary>
    public SubsystemSnapshot[] GetSnapshots()
    {
        var result = new SubsystemSnapshot[_states.Count];
        var i = 0;
        foreach (var (id, state) in _states)
        {
            result[i++] = new SubsystemSnapshot
            {
                Id = id,
                DisplayName = state.DisplayName,
                Category = state.Category,
                ActiveTaskCount = Volatile.Read(ref state.ActiveTaskCount),
                ManagedAllocBytes = Interlocked.Read(ref state.ManagedAllocBytes),
                ManagedAllocRate = Interlocked.Read(ref state.SmoothedAllocRate),
                NativeBytes = Interlocked.Read(ref state.NativeBytes),
                NativeAllocs = Interlocked.Read(ref state.NativeAllocs),
            };
        }
        return result;
    }

    /// <summary>
    /// Fetch native subsystem memory from the Rust allocator via FFI
    /// and merge it into the tracker.
    /// </summary>
    public void RefreshNativeCounters()
    {
        nint ptr = default;
        try
        {
            ptr = NativeLib.SubsystemMemory();
            if (ptr == 0) return;

            var json = Marshal.PtrToStringUTF8(ptr);
            if (string.IsNullOrEmpty(json)) return;

            var entries = System.Text.Json.JsonSerializer.Deserialize<NativeSubsystemEntry[]>(json);
            if (entries != null)
                MergeNativeCounters(entries);
        }
        finally
        {
            if (ptr != 0)
                NativeLib.FreeString(ptr);
        }
    }

    /// <summary>
    /// Populate runtime subsystems from .NET system APIs.
    /// Call once per timer tick alongside RefreshNativeCounters/UpdateRateSamples.
    /// </summary>
    public void RefreshRuntimeMetrics()
    {
        // .NET GC heap
        var gcState = GetOrCreateState("runtime.gc");
        var gcHeap = GC.GetTotalMemory(false);
        Interlocked.Exchange(ref gcState.ManagedAllocBytes, gcHeap);

        // Thread pool — actual pool thread count + pending work items
        var tpState = GetOrCreateState("runtime.threadpool");
        Volatile.Write(ref tpState.ActiveTaskCount, ThreadPool.ThreadCount);
        Interlocked.Exchange(ref tpState.ManagedAllocBytes, ThreadPool.PendingWorkItemCount * 64L);

        // Native (Rust) — total Rust-side memory across all subsystems
        // Individual Core subsystems (storage, sync, crypto, cloud) have their
        // own per-subsystem breakdown from MergeNativeCounters. This entry shows
        // the aggregate so it's never "—" when Rust is active.
        try
        {
            long totalRustBytes = 0;
            foreach (var (id, state) in _states)
            {
                if (id.StartsWith("core.", StringComparison.Ordinal) || id == "runtime.native")
                    totalRustBytes += Interlocked.Read(ref state.NativeBytes);
            }
            var nativeState = GetOrCreateState("runtime.native");
            Interlocked.Exchange(ref nativeState.NativeBytes, totalRustBytes);
        }
        catch { /* snapshot iteration may race with registration */ }

        // Shell — mark active (UI thread is always running)
        var shellState = GetOrCreateState("shell");
        Volatile.Write(ref shellState.ActiveTaskCount, 1);

        // Rendering — mark active while UI is running
        var renderState = GetOrCreateState("rendering");
        Volatile.Write(ref renderState.ActiveTaskCount, 1);
    }

    /// <summary>
    /// Static convenience: run tagged if tracker is available, otherwise plain Task.Run.
    /// </summary>
    public static Task RunTaggedStatic(string subsystemId, Func<Task> action)
    {
        return Instance?.RunTagged(subsystemId, action) ?? Task.Run(action);
    }

    /// <summary>
    /// Static convenience: run tagged if tracker is available, otherwise plain Task.Run.
    /// </summary>
    public static Task RunTaggedStatic(string subsystemId, Action action)
    {
        return Instance?.RunTagged(subsystemId, action) ?? Task.Run(action);
    }

    private SubsystemState GetOrCreateState(string id)
    {
        return _states.GetOrAdd(id, static key =>
        {
            // Infer display name and category from the subsystem ID pattern
            if (key.StartsWith("plugin.", StringComparison.Ordinal))
            {
                // "plugin.privstack.notes" → "Notes", category "Plugin"
                var pluginId = key["plugin.".Length..];
                var lastDot = pluginId.LastIndexOf('.');
                var shortName = lastDot >= 0 ? pluginId[(lastDot + 1)..] : pluginId;
                // Title-case the short name
                var display = shortName.Length > 0
                    ? char.ToUpperInvariant(shortName[0]) + shortName[1..]
                    : shortName;
                return new SubsystemState(display, "Plugin");
            }

            return new SubsystemState(key, "Other");
        });
    }

    private sealed class SubsystemState(string displayName, string category)
    {
        public string DisplayName = displayName;
        public string Category = category;
        public int ActiveTaskCount;
        public long ManagedAllocBytes;
        public long SmoothedAllocRate;
        public long NativeBytes;
        public long NativeAllocs;

        // Rate sampling state (not thread-safe — only accessed from timer thread)
        public long LastSampleBytes;
        public readonly long[] RateSamples = new long[RateSampleCount];
        public int RateSampleIndex;
    }

    private sealed class ScopeGuard(SubsystemState state, string? prevSubsystem, long startBytes) : IDisposable
    {
        public void Dispose()
        {
            var delta = GC.GetAllocatedBytesForCurrentThread() - startBytes;
            Interlocked.Add(ref state.ManagedAllocBytes, delta);
            Interlocked.Decrement(ref state.ActiveTaskCount);
            CurrentSubsystem.Value = prevSubsystem;
        }
    }
}

public struct SubsystemSnapshot
{
    public string Id;
    public string DisplayName;
    public string Category;
    public int ActiveTaskCount;
    public long ManagedAllocBytes;
    public long ManagedAllocRate;
    public long NativeBytes;
    public long NativeAllocs;

    public readonly string Status => ActiveTaskCount > 0 ? "Active" : NativeBytes > 0 ? "Idle" : "Stopped";
}

public struct NativeSubsystemEntry
{
    [System.Text.Json.Serialization.JsonPropertyName("id")]
    public byte Id { get; set; }

    [System.Text.Json.Serialization.JsonPropertyName("bytes")]
    public long Bytes { get; set; }

    [System.Text.Json.Serialization.JsonPropertyName("allocs")]
    public ulong Allocs { get; set; }
}
