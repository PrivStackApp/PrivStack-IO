namespace PrivStack.Services.Diagnostics;

/// <summary>
/// Static table of built-in subsystem identifiers. Plugin subsystems
/// (plugin.{id}) are registered dynamically from PluginRegistry.
/// </summary>
public static class SubsystemDefinitions
{
    public static readonly SubsystemInfo[] BuiltIn =
    [
        new("shell", "Shell", "UI"),
        new("ai.rag", "RAG Index", "AI"),
        new("ai.intent", "Intent Engine", "AI"),
        new("ai.embedding", "Embeddings", "AI"),
        new("ai.llm", "Local LLM", "AI"),
        new("ai.whisper", "Speech-to-Text", "AI"),
        new("core.storage", "Storage", "Core"),
        new("core.sync", "P2P Sync", "Core"),
        new("core.cloud", "Cloud Sync", "Core"),
        new("core.crypto", "Crypto/Vault", "Core"),
        new("ipc", "IPC Server", "Services"),
        new("reminders", "Reminders", "Services"),
        new("updates", "Auto-Update", "Services"),
        new("runtime.gc", ".NET GC", "Runtime"),
        new("runtime.threadpool", "Thread Pool", "Runtime"),
        new("rendering", "Rendering", "UI"),
    ];

    /// <summary>
    /// Mapping from Rust subsystem IDs (0-7) to C# subsystem IDs.
    /// Matches the constants in allocator.rs.
    /// </summary>
    public static readonly string[] RustIdMap =
    [
        "runtime.native",   // 0: untagged
        "core.storage",     // 1: storage
        "core.sync",        // 2: sync
        "core.crypto",      // 3: crypto
        "core.cloud",       // 4: cloud
        "runtime.native",   // 5: plugins (grouped with native)
        "runtime.native",   // 6: ffi
        "runtime.native",   // 7: reserved
    ];
}

public readonly record struct SubsystemInfo(string Id, string DisplayName, string Category);
