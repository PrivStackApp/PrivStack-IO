using System.Net;
using System.Security.Cryptography;
using System.Text;
using System.Text.Json;
using System.Text.RegularExpressions;
using Microsoft.AspNetCore.Builder;
using Microsoft.AspNetCore.Hosting;
using Microsoft.AspNetCore.Http;
using Microsoft.AspNetCore.Routing;
using Microsoft.Extensions.Logging;
using PrivStack.Desktop.Services.Abstractions;
using PrivStack.Desktop.Services.Plugin;
using PrivStack.Sdk.Capabilities;
using ILogger = Serilog.ILogger;
using Log = Serilog.Log;

namespace PrivStack.Desktop.Services.Api;

/// <summary>
/// Kestrel-based local HTTP API server.
/// Discovers <see cref="IApiProvider"/> capability providers and maps their routes.
/// Authenticates via X-API-Key header (or Authorization: Bearer).
/// </summary>
public sealed class LocalApiServer : ILocalApiServer, IDisposable
{
    private static readonly ILogger _log = Log.ForContext<LocalApiServer>();

    private readonly IPluginRegistry _pluginRegistry;
    private readonly IAppSettingsService _appSettings;
    private readonly IWorkspaceService _workspaceService;
    private WebApplication? _app;

    public LocalApiServer(IPluginRegistry pluginRegistry, IAppSettingsService appSettings, IWorkspaceService workspaceService)
    {
        _pluginRegistry = pluginRegistry;
        _appSettings = appSettings;
        _workspaceService = workspaceService;
    }

    public bool IsRunning => _app != null;
    public int? Port => IsRunning ? _appSettings.Settings.ApiPort : null;
    public string BindAddress { get; set; } = "127.0.0.1";

    public async Task StartAsync(CancellationToken ct = default)
    {
        if (_app != null) return;

        var settings = _appSettings.Settings;
        var port = settings.ApiPort;

        // Ensure API key exists — generate on first start
        if (string.IsNullOrEmpty(settings.ApiKey))
        {
            var keyBytes = new byte[32];
            RandomNumberGenerator.Fill(keyBytes);
            settings.ApiKey = Convert.ToBase64String(keyBytes)
                .Replace("+", "-").Replace("/", "_").TrimEnd('=');
            _appSettings.Save();
            _log.Information("Generated new API key");
        }

        var builder = WebApplication.CreateSlimBuilder();
        var bind = BindAddress;
        builder.WebHost.ConfigureKestrel(k =>
        {
            if (bind is "127.0.0.1" or "localhost" or "::1")
                k.ListenLocalhost(port);
            else
                k.Listen(IPAddress.Parse(bind), port);
        });

        // Suppress ASP.NET Core's default console logging — we use Serilog
        builder.Logging.ClearProviders();

        _app = builder.Build();

        // Shell routes (no auth for status)
        var workspaceName = _workspaceService.GetActiveWorkspace()?.Name;
        var workspaceId = _workspaceService.GetActiveWorkspace()?.Id;
        _app.MapGet("/api/v1/status", () => Results.Ok(new
        {
            status = "ok",
            version = "1",
            workspace = workspaceName,
            workspace_id = workspaceId,
        }));

        // All other routes require API key
        var apiGroup = _app.MapGroup("/api/v1").AddEndpointFilter(async (context, next) =>
        {
            var httpContext = context.HttpContext;

            // Extract API key from header
            var apiKey = httpContext.Request.Headers["X-API-Key"].FirstOrDefault();
            if (string.IsNullOrEmpty(apiKey))
            {
                var authHeader = httpContext.Request.Headers.Authorization.FirstOrDefault();
                if (authHeader != null && authHeader.StartsWith("Bearer ", StringComparison.OrdinalIgnoreCase))
                    apiKey = authHeader["Bearer ".Length..];
            }

            var expectedKey = _appSettings.Settings.ApiKey;
            if (string.IsNullOrEmpty(apiKey) || !ConstantTimeEquals(apiKey, expectedKey))
            {
                return Results.Json(new { error = "Unauthorized" }, statusCode: 401);
            }

            return await next(context);
        });

        // Route listing
        var routeManifest = new List<object>();

        apiGroup.MapGet("/routes", () => Results.Ok(new { routes = routeManifest }));

        // Discover and map plugin routes
        var providers = _pluginRegistry.GetCapabilityProviders<IApiProvider>();
        foreach (var provider in providers)
        {
            MapProviderRoutes(apiGroup, provider, routeManifest);
        }

        _log.Information("Local API server starting on http://{Bind}:{Port} with {RouteCount} plugin routes",
            bind, port, routeManifest.Count);

        await _app.StartAsync(ct);
    }

    public async Task StopAsync()
    {
        if (_app == null) return;

        _log.Information("Local API server stopping");
        await _app.StopAsync();
        await _app.DisposeAsync();
        _app = null;
    }

    public void Dispose()
    {
        if (_app != null)
        {
            _app.StopAsync().GetAwaiter().GetResult();
            _app.DisposeAsync().AsTask().GetAwaiter().GetResult();
            _app = null;
        }
    }

    private void MapProviderRoutes(
        RouteGroupBuilder group,
        IApiProvider provider,
        List<object> routeManifest)
    {
        var slug = provider.ApiSlug;
        var routes = provider.GetRoutes();

        foreach (var route in routes)
        {
            var fullPath = string.IsNullOrEmpty(route.Path)
                ? $"/{slug}"
                : $"/{slug}/{route.Path}";

            // Convert {id} style params to ASP.NET Core :id style is not needed —
            // ASP.NET Core already uses {id} syntax natively.
            var paramNames = ExtractParamNames(route.Path);

            routeManifest.Add(new
            {
                method = route.Method.ToString().ToUpperInvariant(),
                path = $"/api/v1{fullPath}",
                description = route.Description ?? "",
                plugin = slug,
            });

            var routeId = route.RouteId;
            var capturedProvider = provider;

            RequestDelegate handler = async httpContext =>
            {
                try
                {
                    var pathParams = new Dictionary<string, string>();
                    foreach (var name in paramNames)
                    {
                        var value = httpContext.GetRouteValue(name)?.ToString();
                        if (value != null)
                            pathParams[name] = value;
                    }

                    var queryParams = new Dictionary<string, string>();
                    foreach (var kvp in httpContext.Request.Query)
                        queryParams[kvp.Key] = kvp.Value.FirstOrDefault() ?? "";

                    string? body = null;
                    if (httpContext.Request.ContentLength > 0 ||
                        httpContext.Request.Headers.ContentType.Count > 0)
                    {
                        using var reader = new StreamReader(httpContext.Request.Body, Encoding.UTF8);
                        body = await reader.ReadToEndAsync();
                    }

                    var apiRequest = new ApiRequest
                    {
                        RouteId = routeId,
                        PathParams = pathParams,
                        QueryParams = queryParams,
                        Body = body,
                    };

                    var response = await capturedProvider.HandleRequestAsync(apiRequest, httpContext.RequestAborted);

                    httpContext.Response.StatusCode = response.StatusCode;
                    httpContext.Response.ContentType = response.ContentType;
                    if (response.Body != null)
                        await httpContext.Response.WriteAsync(response.Body, httpContext.RequestAborted);
                }
                catch (Exception ex)
                {
                    _log.Error(ex, "API handler error for {Method} {Path}", route.Method, fullPath);
                    httpContext.Response.StatusCode = 500;
                    httpContext.Response.ContentType = "application/json";
                    await httpContext.Response.WriteAsync(
                        """{"error":"Internal server error"}""",
                        httpContext.RequestAborted);
                }
            };

            switch (route.Method)
            {
                case ApiMethod.Get:
                    group.MapGet(fullPath, handler);
                    break;
                case ApiMethod.Post:
                    group.MapPost(fullPath, handler);
                    break;
                case ApiMethod.Put:
                    group.MapPut(fullPath, handler);
                    break;
                case ApiMethod.Patch:
                    group.MapPatch(fullPath, handler);
                    break;
                case ApiMethod.Delete:
                    group.MapDelete(fullPath, handler);
                    break;
            }
        }

        _log.Debug("Mapped {Count} routes for API provider: {Slug}", routes.Count, slug);
    }

    private static List<string> ExtractParamNames(string path)
    {
        var names = new List<string>();
        foreach (Match match in Regex.Matches(path, @"\{(\w+)\}"))
            names.Add(match.Groups[1].Value);
        return names;
    }

    private static bool ConstantTimeEquals(string? a, string? b)
    {
        if (a == null || b == null) return a == b;
        var aBytes = Encoding.UTF8.GetBytes(a);
        var bBytes = Encoding.UTF8.GetBytes(b);
        return CryptographicOperations.FixedTimeEquals(aBytes, bBytes);
    }
}
