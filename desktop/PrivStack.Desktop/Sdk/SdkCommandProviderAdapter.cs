using PrivStack.Desktop.Services;

namespace PrivStack.Desktop.Sdk;

/// <summary>
/// Wraps an SDK ICommandProvider for the Desktop command palette.
/// </summary>
internal sealed class SdkCommandProviderAdapter : ICommandProvider
{
    private readonly PrivStack.Sdk.ICommandProvider _inner;

    public SdkCommandProviderAdapter(PrivStack.Sdk.ICommandProvider inner)
    {
        _inner = inner;
    }

    public int Priority => _inner.Priority;

    public IEnumerable<CommandDefinition> GetCommands()
    {
        return _inner.GetCommands().Select(c => new CommandDefinition
        {
            Name = c.Name,
            Description = c.Description,
            Keywords = c.Keywords,
            Category = c.Category,
            Icon = c.Icon,
            Execute = c.Execute,
        });
    }
}
