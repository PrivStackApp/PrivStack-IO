namespace PrivStack.Sdk.Messaging;

/// <summary>
/// Broadcast when an entity has been imported from a remote peer via P2P sync.
/// Plugins subscribe to this to refresh their UI when remote changes arrive.
/// </summary>
public sealed record EntitySyncedMessage
{
    /// <summary>
    /// The entity's unique identifier.
    /// </summary>
    public required string EntityId { get; init; }

    /// <summary>
    /// The entity type (e.g., "page", "task", "event", "sticky_note").
    /// </summary>
    public required string EntityType { get; init; }

    /// <summary>
    /// The raw JSON data of the imported entity, if available.
    /// </summary>
    public string? JsonData { get; init; }

    /// <summary>
    /// True if the entity was removed (deleted/trashed) rather than created/updated.
    /// </summary>
    public bool IsRemoval { get; init; }
}
