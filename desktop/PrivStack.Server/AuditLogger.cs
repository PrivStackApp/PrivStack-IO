using System.Text.Json;
using Microsoft.AspNetCore.Http;

namespace PrivStack.Server;

/// <summary>
/// JSON Lines audit logger for enterprise compliance.
/// Writes structured events to a file at an admin-controlled path.
/// </summary>
internal sealed class AuditLogger : IDisposable
{
    private static readonly JsonSerializerOptions _jsonOptions = new()
    {
        PropertyNamingPolicy = JsonNamingPolicy.SnakeCaseLower,
    };

    private readonly string _logPath;
    private readonly string _level;
    private readonly StreamWriter _writer;
    private readonly object _lock = new();

    public AuditLogger(AuditPolicy config)
    {
        _logPath = config.LogPath ?? throw new ArgumentException("Audit log path is required");
        _level = config.Level;

        // Ensure directory exists
        var dir = Path.GetDirectoryName(_logPath);
        if (!string.IsNullOrEmpty(dir))
            Directory.CreateDirectory(dir);

        _writer = new StreamWriter(_logPath, append: true) { AutoFlush = true };
    }

    /// <summary>
    /// Creates ASP.NET Core middleware that logs API requests.
    /// </summary>
    public Func<HttpContext, RequestDelegate, Task> CreateMiddleware()
    {
        return async (context, next) =>
        {
            var method = context.Request.Method;

            // Level filtering
            if (_level == "write" && method is "GET" or "HEAD" or "OPTIONS")
            {
                await next(context);
                return;
            }

            if (_level == "auth")
            {
                // Only log auth-related events — handled separately
                await next(context);
                return;
            }

            var startTime = DateTimeOffset.UtcNow;
            await next(context);

            var entry = new AuditEntry
            {
                Timestamp = startTime.ToString("o"),
                Method = method,
                Path = context.Request.Path.Value ?? "",
                StatusCode = context.Response.StatusCode,
                RemoteIp = context.Connection.RemoteIpAddress?.ToString(),
                DurationMs = (int)(DateTimeOffset.UtcNow - startTime).TotalMilliseconds,
                HasApiKey = context.Request.Headers.ContainsKey("X-API-Key") ||
                            context.Request.Headers.Authorization.Count > 0,
            };

            WriteEntry(entry);
        };
    }

    /// <summary>
    /// Logs an authentication event (success/failure).
    /// </summary>
    public void LogAuthEvent(string eventType, string? remoteIp, bool success, string? detail = null)
    {
        WriteEntry(new AuditEntry
        {
            Timestamp = DateTimeOffset.UtcNow.ToString("o"),
            Method = "AUTH",
            Path = eventType,
            StatusCode = success ? 200 : 401,
            RemoteIp = remoteIp,
            Detail = detail,
        });
    }

    /// <summary>
    /// Logs a policy enforcement event.
    /// </summary>
    public void LogPolicyEvent(string action, string detail)
    {
        WriteEntry(new AuditEntry
        {
            Timestamp = DateTimeOffset.UtcNow.ToString("o"),
            Method = "POLICY",
            Path = action,
            StatusCode = 0,
            Detail = detail,
        });
    }

    private void WriteEntry(AuditEntry entry)
    {
        var json = JsonSerializer.Serialize(entry, _jsonOptions);
        lock (_lock)
        {
            _writer.WriteLine(json);
        }
    }

    public void Dispose()
    {
        lock (_lock)
        {
            _writer.Dispose();
        }
    }
}

internal sealed class AuditEntry
{
    public string Timestamp { get; set; } = "";
    public string Method { get; set; } = "";
    public string Path { get; set; } = "";
    public int StatusCode { get; set; }
    public string? RemoteIp { get; set; }
    public int DurationMs { get; set; }
    public bool HasApiKey { get; set; }
    public string? Detail { get; set; }
}
