namespace PrivStack.Desktop.ViewModels.AiTray;

/// <summary>
/// Lightweight VM wrapping a <see cref="Services.AI.ConversationSession"/> for the history list.
/// </summary>
public sealed class ConversationSessionViewModel
{
    public required string Id { get; init; }
    public required string Title { get; init; }
    public required string TimeAgo { get; init; }
    public required int MessageCount { get; init; }
    public bool IsActive { get; set; }

    internal static string FormatTimeAgo(DateTimeOffset timestamp)
    {
        var elapsed = DateTimeOffset.UtcNow - timestamp;
        if (elapsed.TotalMinutes < 1) return "Just now";
        if (elapsed.TotalMinutes < 60) return $"{(int)elapsed.TotalMinutes}m ago";
        if (elapsed.TotalHours < 24) return $"{(int)elapsed.TotalHours}h ago";
        if (elapsed.TotalDays < 2) return "Yesterday";
        if (elapsed.TotalDays < 7) return $"{(int)elapsed.TotalDays}d ago";
        return timestamp.LocalDateTime.ToString("MMM d");
    }
}
