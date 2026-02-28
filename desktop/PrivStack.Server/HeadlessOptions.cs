namespace PrivStack.Server;

/// <summary>
/// Parsed CLI arguments for the headless server.
/// </summary>
public sealed record HeadlessOptions
{
    public string? WorkspaceName { get; init; }
    public int? Port { get; init; }
    public string? BindAddress { get; init; }
    public bool ShowApiKey { get; init; }
    public bool GenerateApiKey { get; init; }
    public bool Setup { get; init; }
    public bool SetupNetwork { get; init; }
    public bool SetupTls { get; init; }
    public bool SetupPolicy { get; init; }
}
