namespace PrivStack.Sdk.Capabilities;

/// <summary>
/// Capability for plugins that support per-entity sharing via PrivStack Cloud.
/// Plugins implement this to declare which entity types can be shared and
/// provide metadata for the share UI.
/// </summary>
public interface IShareableBehavior
{
    /// <summary>
    /// Entity types that support sharing (e.g., ["task", "project"], ["page"]).
    /// </summary>
    IReadOnlyList<string> ShareableEntityTypes { get; }

    /// <summary>
    /// If true, plugin is excluded from sharing entirely (e.g., Calendar, Contacts).
    /// Default is false.
    /// </summary>
    bool IsExcludedFromSharing => false;

    /// <summary>
    /// Returns a human-readable title for the entity (shown in share dialogs).
    /// </summary>
    string GetEntityTitle(string entityId);

    /// <summary>
    /// Returns the entity type for a given entity ID (e.g., "task", "page").
    /// </summary>
    string GetEntityType(string entityId);
}
