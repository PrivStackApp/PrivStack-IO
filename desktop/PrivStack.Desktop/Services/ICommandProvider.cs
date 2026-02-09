namespace PrivStack.Desktop.Services;

/// <summary>
/// Interface for modules/plugins to provide commands to the command palette.
/// Implement this interface to inject context-aware commands from any module.
/// </summary>
public interface ICommandProvider
{
    /// <summary>
    /// Gets the commands provided by this module.
    /// Called each time the command palette is opened or filtered,
    /// allowing for dynamic commands based on current state.
    /// </summary>
    /// <returns>A list of command items to add to the palette.</returns>
    IEnumerable<CommandDefinition> GetCommands();

    /// <summary>
    /// Gets the priority of this provider (lower = higher priority in results).
    /// Core commands should use 0, plugins should use 100+.
    /// </summary>
    int Priority => 100;
}

/// <summary>
/// Defines a command that can be executed from the command palette.
/// </summary>
public sealed record CommandDefinition
{
    /// <summary>
    /// Display name shown in the palette.
    /// </summary>
    public required string Name { get; init; }

    /// <summary>
    /// Short description of what the command does.
    /// </summary>
    public required string Description { get; init; }

    /// <summary>
    /// Space-separated keywords for search matching.
    /// </summary>
    public string Keywords { get; init; } = string.Empty;

    /// <summary>
    /// Category for grouping (e.g., "Contacts", "Notes", "Navigation").
    /// </summary>
    public string Category { get; init; } = "General";

    /// <summary>
    /// The action to execute when the command is selected.
    /// </summary>
    public required Action Execute { get; init; }

    /// <summary>
    /// Optional icon identifier.
    /// </summary>
    public string? Icon { get; init; }
}
