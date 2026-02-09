namespace PrivStack.Sdk;

/// <summary>
/// Interface for plugins to provide commands to the command palette.
/// </summary>
public interface ICommandProvider
{
    IEnumerable<CommandDefinition> GetCommands();
    int Priority => 100;
}

/// <summary>
/// A command that can be executed from the command palette.
/// </summary>
public sealed record CommandDefinition
{
    public required string Name { get; init; }
    public required string Description { get; init; }
    public string Keywords { get; init; } = string.Empty;
    public string Category { get; init; } = "General";
    public required Action Execute { get; init; }
    public string? Icon { get; init; }
}
