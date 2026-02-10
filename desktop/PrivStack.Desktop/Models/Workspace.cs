using System.Text.Json.Serialization;

namespace PrivStack.Desktop.Models;

/// <summary>
/// Represents a workspace with its own isolated database.
/// </summary>
public record Workspace
{
    [JsonPropertyName("id")]
    public string Id { get; init; } = string.Empty;

    [JsonPropertyName("name")]
    public string Name { get; init; } = string.Empty;

    [JsonPropertyName("created_at")]
    public DateTime CreatedAt { get; init; } = DateTime.UtcNow;

    [JsonPropertyName("has_password")]
    public bool HasPassword { get; init; }
}

/// <summary>
/// Registry of all workspaces and the currently active one.
/// </summary>
public record WorkspaceRegistry
{
    [JsonPropertyName("workspaces")]
    public List<Workspace> Workspaces { get; init; } = [];

    [JsonPropertyName("active_workspace_id")]
    public string? ActiveWorkspaceId { get; init; }
}
