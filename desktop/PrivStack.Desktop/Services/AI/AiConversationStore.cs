using System.Text.Json;
using System.Text.Json.Serialization;
using Serilog;

namespace PrivStack.Desktop.Services.AI;

/// <summary>
/// Persists AI chat conversation sessions to disk.
/// Follows the same debounced-JSON pattern as <see cref="AiMemoryService"/>.
/// File: {DataPaths.BaseDir}/ai-conversations.json (global, not per-workspace).
/// </summary>
internal sealed class AiConversationStore
{
    private const int MaxSessions = 100;
    private static readonly ILogger _log = Log.ForContext<AiConversationStore>();

    private readonly string _filePath;
    private List<ConversationSession> _sessions = [];
    private bool _isDirty;
    private System.Timers.Timer? _saveTimer;

    public AiConversationStore()
    {
        _filePath = Path.Combine(DataPaths.BaseDir, "ai-conversations.json");
        Load();
    }

    public ConversationSession CreateSession()
    {
        if (_sessions.Count >= MaxSessions)
            _sessions.RemoveAt(_sessions.Count - 1); // remove oldest (sorted desc)

        var session = new ConversationSession
        {
            Id = Guid.NewGuid().ToString("N"),
            Title = "New conversation",
            CreatedAt = DateTimeOffset.UtcNow,
            UpdatedAt = DateTimeOffset.UtcNow
        };
        _sessions.Insert(0, session);
        SaveDebounced();
        return session;
    }

    public ConversationSession? GetSession(string id) =>
        _sessions.FirstOrDefault(s => s.Id == id);

    public IReadOnlyList<ConversationSession> GetAllSessions() => _sessions;

    public void AddMessage(string sessionId, string role, string content)
    {
        var session = GetSession(sessionId);
        if (session == null) return;

        var tokenEstimate = content.Length / 4;
        session.Messages.Add(new ConversationMessage
        {
            Role = role,
            Content = content,
            Timestamp = DateTimeOffset.UtcNow,
            TokenEstimate = tokenEstimate
        });

        session.EstimatedTokens += tokenEstimate;
        session.UpdatedAt = DateTimeOffset.UtcNow;

        // Set title from first user message
        if (session.Title == "New conversation" && role == "user")
            session.Title = content.Length > 80 ? content[..80] + "..." : content;

        // Move to front (most recent)
        _sessions.Remove(session);
        _sessions.Insert(0, session);
        SaveDebounced();
    }

    public void DeleteSession(string id)
    {
        _sessions.RemoveAll(s => s.Id == id);
        SaveDebounced();
    }

    public void Flush()
    {
        _saveTimer?.Stop();
        _saveTimer?.Dispose();
        if (_isDirty) Save();
    }

    private void Load()
    {
        try
        {
            if (!File.Exists(_filePath)) return;
            var json = File.ReadAllText(_filePath);
            _sessions = JsonSerializer.Deserialize<List<ConversationSession>>(json) ?? [];
            _log.Debug("Loaded {Count} AI conversation sessions", _sessions.Count);
        }
        catch (Exception ex)
        {
            _log.Error(ex, "Failed to load AI conversations from {Path}", _filePath);
            _sessions = [];
        }
    }

    private void Save()
    {
        try
        {
            Directory.CreateDirectory(Path.GetDirectoryName(_filePath)!);
            var json = JsonSerializer.Serialize(_sessions, new JsonSerializerOptions { WriteIndented = true });
            File.WriteAllText(_filePath, json);
            _isDirty = false;
        }
        catch (Exception ex)
        {
            _log.Error(ex, "Failed to save AI conversations to {Path}", _filePath);
        }
    }

    private void SaveDebounced()
    {
        _isDirty = true;
        _saveTimer?.Stop();
        _saveTimer?.Dispose();
        _saveTimer = new System.Timers.Timer(1000) { AutoReset = false };
        _saveTimer.Elapsed += (_, _) => { if (_isDirty) Save(); };
        _saveTimer.Start();
    }
}

internal sealed class ConversationSession
{
    [JsonPropertyName("id")]
    public string Id { get; init; } = string.Empty;

    [JsonPropertyName("title")]
    public string Title { get; set; } = string.Empty;

    [JsonPropertyName("created_at")]
    public DateTimeOffset CreatedAt { get; init; }

    [JsonPropertyName("updated_at")]
    public DateTimeOffset UpdatedAt { get; set; }

    [JsonPropertyName("estimated_tokens")]
    public int EstimatedTokens { get; set; }

    [JsonPropertyName("messages")]
    public List<ConversationMessage> Messages { get; init; } = [];
}

internal sealed class ConversationMessage
{
    [JsonPropertyName("role")]
    public string Role { get; init; } = string.Empty;

    [JsonPropertyName("content")]
    public string Content { get; init; } = string.Empty;

    [JsonPropertyName("timestamp")]
    public DateTimeOffset Timestamp { get; init; }

    [JsonPropertyName("token_estimate")]
    public int TokenEstimate { get; init; }
}
