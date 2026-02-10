namespace PrivStack.Sdk.Capabilities;

/// <summary>
/// A single reminder that should fire at a specific UTC time.
/// Key must be globally unique for deduplication (format: "{pluginId}:{itemId}:{suffix}").
/// </summary>
public record ReminderInfo
{
    /// <summary>
    /// Globally unique key for deduplication (e.g., "privstack.tasks:abc123:1707350400").
    /// </summary>
    public required string Key { get; init; }

    /// <summary>
    /// Notification title (e.g., task title or event title).
    /// </summary>
    public required string Title { get; init; }

    /// <summary>
    /// Notification body text.
    /// </summary>
    public required string Body { get; init; }

    /// <summary>
    /// When the reminder should fire (UTC).
    /// </summary>
    public required DateTimeOffset FireAtUtc { get; init; }

    /// <summary>
    /// Plugin ID that owns this reminder.
    /// </summary>
    public required string SourcePluginId { get; init; }

    /// <summary>
    /// The item ID within the plugin (for deep-linking).
    /// </summary>
    public required string ItemId { get; init; }
}

/// <summary>
/// Capability interface for plugins that can produce reminders.
/// Implemented by plugins (Tasks, Calendar) and discovered via IPluginRegistry.GetCapabilityProviders.
/// </summary>
public interface IReminderProvider
{
    /// <summary>
    /// Returns all reminders whose FireAtUtc falls within [windowStart, windowEnd].
    /// Called every ~30 seconds by the ReminderSchedulerService.
    /// </summary>
    Task<IReadOnlyList<ReminderInfo>> GetRemindersInWindowAsync(
        DateTimeOffset windowStart,
        DateTimeOffset windowEnd,
        CancellationToken cancellationToken = default);
}
