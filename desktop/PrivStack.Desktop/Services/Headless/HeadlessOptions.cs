namespace PrivStack.Desktop.Services.Headless;

/// <summary>
/// Parsed CLI arguments for headless (API-only) mode.
/// </summary>
public sealed record HeadlessOptions
{
    public string? WorkspaceName { get; init; }
    public int? Port { get; init; }
    public string BindAddress { get; init; } = "127.0.0.1";
    public bool ShowApiKey { get; init; }
    public bool GenerateApiKey { get; init; }
}
