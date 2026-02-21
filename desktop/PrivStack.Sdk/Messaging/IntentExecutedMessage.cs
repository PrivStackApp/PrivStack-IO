namespace PrivStack.Sdk.Messaging;

/// <summary>
/// Broadcast after an intent is successfully executed, allowing source plugins
/// to create reverse links from the source entity to the newly created entity.
/// </summary>
public sealed record IntentExecutedMessage
{
    /// <summary>Plugin that emitted the original signal.</summary>
    public required string SourcePluginId { get; init; }

    /// <summary>Entity type that originated the signal (e.g. "task", "sticky_note").</summary>
    public required string SourceEntityType { get; init; }

    /// <summary>Entity ID that originated the signal.</summary>
    public required string SourceEntityId { get; init; }

    /// <summary>ID of the newly created entity.</summary>
    public required string CreatedEntityId { get; init; }

    /// <summary>Entity type of the newly created entity (e.g. "event", "task").</summary>
    public required string CreatedEntityType { get; init; }

    /// <summary>Link type for navigation (e.g. "calendar_event", "task").</summary>
    public string? NavigationLinkType { get; init; }
}
