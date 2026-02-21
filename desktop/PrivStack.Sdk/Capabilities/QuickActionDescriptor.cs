namespace PrivStack.Sdk.Capabilities;

/// <summary>
/// Metadata describing a global quick action contributed by a plugin.
/// </summary>
public sealed record QuickActionDescriptor
{
    /// <summary>Unique action identifier, e.g. "notes.new_sticky_note".</summary>
    public required string ActionId { get; init; }

    /// <summary>Human-readable name shown in the command palette.</summary>
    public required string DisplayName { get; init; }

    /// <summary>Short description of what the action does.</summary>
    public string? Description { get; init; }

    /// <summary>Owning plugin identifier, e.g. "privstack.notes".</summary>
    public required string PluginId { get; init; }

    /// <summary>Icon name for display in the command palette.</summary>
    public string? Icon { get; init; }

    /// <summary>Optional keyboard shortcut hint, e.g. "Cmd+Shift+S".</summary>
    public string? DefaultShortcutHint { get; init; }

    /// <summary>Category for grouping in the command palette. Defaults to "Quick Actions".</summary>
    public string Category { get; init; } = "Quick Actions";

    /// <summary>
    /// When true, invoking this action shows a modal overlay with UI from
    /// <see cref="IQuickActionProvider.CreateQuickActionContent"/>.
    /// When false, <see cref="IQuickActionProvider.ExecuteQuickActionAsync"/> is called directly.
    /// </summary>
    public bool HasUI { get; init; }
}
