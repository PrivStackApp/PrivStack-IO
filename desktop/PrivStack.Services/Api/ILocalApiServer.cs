namespace PrivStack.Services.Api;

/// <summary>
/// Local HTTP API server for programmatic access to PrivStack data.
/// Supports optional TLS (manual certificate or Let's Encrypt).
/// </summary>
public interface ILocalApiServer
{
    bool IsRunning { get; }
    int? Port { get; }
    string BindAddress { get; set; }
    TlsOptions? TlsOptions { get; set; }
    Task StartAsync(CancellationToken ct = default);
    Task StopAsync();
}
