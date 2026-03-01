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
    /// Register a subsystem for tracking.
    /// </summary>
    public void Register(string id, string displayName, string category)
    {
        _states.TryAdd(id, new SubsystemState(displayName, category));
    }

    /// <summary>
    /// Run an async action tagged with the given subsystem ID.
    /// Tracks active task count and managed allocation delta.
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
        return _states.GetOrAdd(id, static _ => new SubsystemState("Unknown", "Unknown"));
    }

    private sealed class SubsystemState(string displayName, string category)
    {
        public readonly string DisplayName = displayName;
        public readonly string Category = category;
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
