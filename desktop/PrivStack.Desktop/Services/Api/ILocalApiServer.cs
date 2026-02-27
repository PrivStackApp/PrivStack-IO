namespace PrivStack.Desktop.Services.Api;

/// <summary>
/// Local HTTP API server for programmatic access to PrivStack data.
/// Bound to 127.0.0.1 only — never exposed to the network.
/// </summary>
public interface ILocalApiServer
{
    bool IsRunning { get; }
    int? Port { get; }
    Task StartAsync(CancellationToken ct = default);
    Task StopAsync();
}
