using System.Text.Json.Serialization;

namespace PrivStack.Desktop.Models;

/// <summary>
/// Represents a file in cloud storage.
/// </summary>
public class CloudFile
{
    [JsonPropertyName("id")]
    public string Id { get; set; } = string.Empty;

    [JsonPropertyName("name")]
    public string Name { get; set; } = string.Empty;

    [JsonPropertyName("path")]
    public string Path { get; set; } = string.Empty;

    [JsonPropertyName("size")]
    public ulong Size { get; set; }

    [JsonPropertyName("modified_at_ms")]
    public long ModifiedAtMs { get; set; }

    [JsonPropertyName("content_hash")]
    public string? ContentHash { get; set; }

    /// <summary>
    /// Gets the modification time as a DateTime.
    /// </summary>
    public DateTime ModifiedAt => DateTimeOffset.FromUnixTimeMilliseconds(ModifiedAtMs).LocalDateTime;

    /// <summary>
    /// Gets a human-readable size string.
    /// </summary>
    public string SizeDisplay
    {
        get
        {
            if (Size < 1024) return $"{Size} B";
            if (Size < 1024 * 1024) return $"{Size / 1024.0:F1} KB";
            if (Size < 1024 * 1024 * 1024) return $"{Size / (1024.0 * 1024):F1} MB";
            return $"{Size / (1024.0 * 1024 * 1024):F1} GB";
        }
    }
}
