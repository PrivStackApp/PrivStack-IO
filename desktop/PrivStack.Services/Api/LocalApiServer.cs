using System.Net;
using System.Security.Cryptography;
using System.Text;
using System.Text.Json;
using System.Text.RegularExpressions;
using Microsoft.AspNetCore.Builder;
using Microsoft.AspNetCore.Hosting;
using Microsoft.AspNetCore.Http;
using Microsoft.AspNetCore.Routing;
using Microsoft.AspNetCore.Server.Kestrel.Core;
using Microsoft.Extensions.DependencyInjection;
using Microsoft.Extensions.Logging;
using PrivStack.Services.Abstractions;
using PrivStack.Services.Plugin;
using PrivStack.Services.Sdk;
using PrivStack.Sdk.Capabilities;
using ILogger = Serilog.ILogger;
using Log = Serilog.Log;

namespace PrivStack.Services.Api;

/// <summary>
/// Kestrel-based local HTTP API server.
/// Discovers <see cref="IApiProvider"/> capability providers and maps their routes.
/// Authenticates via X-API-Key header (or Authorization: Bearer).
/// Supports TLS via manual certificates or extensibility hooks for ACME providers.
/// </summary>
public sealed class LocalApiServer : ILocalApiServer, IDisposable
{
    private static readonly ILogger _log = Log.ForContext<LocalApiServer>();

    private readonly IPluginRegistry _pluginRegistry;
    private readonly IAppSettingsService _appSettings;
    private readonly IWorkspaceService _workspaceService;
    private ISdkTransport? _transport;
    private WebApplication? _app;

    public LocalApiServer(IPluginRegistry pluginRegistry, IAppSettingsService appSettings, IWorkspaceService workspaceService)
    {
        _pluginRegistry = pluginRegistry;
        _appSettings = appSettings;
        _workspaceService = workspaceService;
    }

    /// <summary>
    /// Wires the SDK transport for passthrough endpoints. Called after DI construction.
    /// </summary>
    internal void SetSdkTransport(ISdkTransport transport) => _transport = transport;

    public bool IsRunning => _app != null;
    public int? Port => IsRunning ? _appSettings.Settings.ApiPort : null;
    public string BindAddress { get; set; } = "127.0.0.1";
    public TlsOptions? TlsOptions { get; set; }

    /// <summary>
    /// Hook for registering additional services before the app is built.
    /// Used by PrivStack.Server to register LettuceEncrypt for ACME TLS.
    /// </summary>
    public Action<IServiceCollection>? OnConfigureServices { get; set; }

    /// <summary>
    /// Hook for custom Kestrel configuration. When set, replaces the default Kestrel setup.
    /// Used by PrivStack.Server for Let's Encrypt Kestrel integration.
    /// </summary>
    public Action<KestrelServerOptions>? OnConfigureKestrel { get; set; }

    /// <summary>
    /// Hook for adding middleware after the app is built but before routes are mapped.
    /// Used by PrivStack.Server for ACME challenge middleware.
    /// </summary>
    public Action<WebApplication>? OnConfigureApp { get; set; }

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
        var useTls = TlsOptions != null;

        // Configure Kestrel
        if (OnConfigureKestrel != null)
        {
            // Delegated configuration (e.g., Let's Encrypt with ACME challenge port)
            builder.WebHost.ConfigureKestrel(OnConfigureKestrel);
        }
        else if (TlsOptions is { Mode: TlsMode.Manual })
        {
            // Manual TLS certificate
            var cert = TlsOptions.LoadCertificate();
            builder.WebHost.ConfigureKestrel(k =>
            {
                if (bind is "127.0.0.1" or "localhost" or "::1")
                    k.ListenLocalhost(port, lo => lo.UseHttps(cert));
                else
                    k.Listen(IPAddress.Parse(bind), port, lo => lo.UseHttps(cert));
            });
        }
        else
        {
            // No TLS — plain HTTP
            builder.WebHost.ConfigureKestrel(k =>
            {
                if (bind is "127.0.0.1" or "localhost" or "::1")
                    k.ListenLocalhost(port);
                else
                    k.Listen(IPAddress.Parse(bind), port);
            });
        }

        // Suppress ASP.NET Core's default console logging — we use Serilog
        builder.Logging.ClearProviders();

        // Allow Server project to register additional services (e.g., LettuceEncrypt)
        OnConfigureServices?.Invoke(builder.Services);

        _app = builder.Build();

        // Allow Server project to add middleware (e.g., ACME challenge handler)
        OnConfigureApp?.Invoke(_app);

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

        // SDK passthrough endpoints — allows Desktop client mode to proxy all data operations
        MapSdkPassthroughRoutes(apiGroup);

        // Discover and map plugin routes
        var providers = _pluginRegistry.GetCapabilityProviders<IApiProvider>();
        foreach (var provider in providers)
        {
            MapProviderRoutes(apiGroup, provider, routeManifest);
        }

        var protocol = useTls ? "https" : "http";
        _log.Information("Local API server starting on {Protocol}://{Bind}:{Port} with {RouteCount} plugin routes",
            protocol, bind, port, routeManifest.Count);

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

    /// <summary>
    /// Maps SDK passthrough endpoints that allow Desktop in client mode to proxy
    /// all data operations to this server over HTTP.
    /// </summary>
    private void MapSdkPassthroughRoutes(RouteGroupBuilder apiGroup)
    {
        if (_transport == null)
        {
            _log.Warning("SDK transport not wired — SDK passthrough routes will not be available");
            return;
        }

        var transport = _transport;

        // Core SDK execute — the primary data operation endpoint
        apiGroup.MapPost("/sdk/execute", async (HttpContext ctx) =>
        {
            var body = await ReadBodyAsync(ctx);
            if (body == null) return Results.BadRequest(new { error = "Request body required" });

            var result = transport.Execute(body);
            return result != null
                ? Results.Text(result, "application/json")
                : Results.Json(new { success = false, error_code = "ffi_error", error_message = "Execute returned null" }, statusCode: 500);
        });

        // Cross-plugin search
        apiGroup.MapPost("/sdk/search", async (HttpContext ctx) =>
        {
            var body = await ReadBodyAsync(ctx);
            if (body == null) return Results.BadRequest(new { error = "Request body required" });

            var result = transport.Search(body);
            return result != null
                ? Results.Text(result, "application/json")
                : Results.Json(new { success = false, error_code = "ffi_error", error_message = "Search returned null" }, statusCode: 500);
        });

        // ── Database maintenance ──

        apiGroup.MapGet("/sdk/db/diagnostics", () =>
        {
            var result = transport.DbDiagnostics();
            return Results.Text(result ?? "{}", "application/json");
        });

        apiGroup.MapPost("/sdk/db/maintenance", () =>
        {
            var result = transport.DbMaintenance();
            return Results.Json(new { error_code = (int)result });
        });

        apiGroup.MapPost("/sdk/db/find-orphans", async (HttpContext ctx) =>
        {
            var body = await ReadBodyAsync(ctx) ?? "[]";
            var result = transport.FindOrphanEntities(body);
            return Results.Text(result ?? "[]", "application/json");
        });

        apiGroup.MapPost("/sdk/db/delete-orphans", async (HttpContext ctx) =>
        {
            var body = await ReadBodyAsync(ctx) ?? "[]";
            var result = transport.DeleteOrphanEntities(body);
            return Results.Text(result ?? "{\"deleted\":0}", "application/json");
        });

        apiGroup.MapPost("/sdk/db/compact", () =>
        {
            var result = transport.CompactDatabases();
            return Results.Text(result ?? "{}", "application/json");
        });

        // ── Vault operations ──

        apiGroup.MapPost("/sdk/vault/is-initialized", async (HttpContext ctx) =>
        {
            var body = await ReadJsonAsync(ctx);
            if (body == null) return Results.BadRequest(new { error = "Request body required" });
            var vaultId = body.RootElement.GetProperty("vault_id").GetString()!;
            return Results.Json(new { result = transport.VaultIsInitialized(vaultId) });
        });

        apiGroup.MapPost("/sdk/vault/initialize", async (HttpContext ctx) =>
        {
            var body = await ReadJsonAsync(ctx);
            if (body == null) return Results.BadRequest(new { error = "Request body required" });
            var vaultId = body.RootElement.GetProperty("vault_id").GetString()!;
            var password = body.RootElement.GetProperty("password").GetString()!;
            var result = transport.VaultInitialize(vaultId, password);
            return Results.Json(new { error_code = (int)result });
        });

        apiGroup.MapPost("/sdk/vault/unlock", async (HttpContext ctx) =>
        {
            var body = await ReadJsonAsync(ctx);
            if (body == null) return Results.BadRequest(new { error = "Request body required" });
            var vaultId = body.RootElement.GetProperty("vault_id").GetString()!;
            var password = body.RootElement.GetProperty("password").GetString()!;
            var result = transport.VaultUnlock(vaultId, password);
            return Results.Json(new { error_code = (int)result });
        });

        apiGroup.MapPost("/sdk/vault/lock", async (HttpContext ctx) =>
        {
            var body = await ReadJsonAsync(ctx);
            if (body == null) return Results.BadRequest(new { error = "Request body required" });
            var vaultId = body.RootElement.GetProperty("vault_id").GetString()!;
            transport.VaultLock(vaultId);
            return Results.Json(new { error_code = 0 });
        });

        apiGroup.MapPost("/sdk/vault/is-unlocked", async (HttpContext ctx) =>
        {
            var body = await ReadJsonAsync(ctx);
            if (body == null) return Results.BadRequest(new { error = "Request body required" });
            var vaultId = body.RootElement.GetProperty("vault_id").GetString()!;
            return Results.Json(new { result = transport.VaultIsUnlocked(vaultId) });
        });

        apiGroup.MapPost("/sdk/vault/blob-store", async (HttpContext ctx) =>
        {
            var body = await ReadJsonAsync(ctx);
            if (body == null) return Results.BadRequest(new { error = "Request body required" });
            var vaultId = body.RootElement.GetProperty("vault_id").GetString()!;
            var blobId = body.RootElement.GetProperty("blob_id").GetString()!;
            var dataB64 = body.RootElement.GetProperty("data").GetString()!;
            var data = Convert.FromBase64String(dataB64);
            var result = transport.VaultBlobStore(vaultId, blobId, data);
            return Results.Json(new { error_code = (int)result });
        });

        apiGroup.MapPost("/sdk/vault/blob-read", async (HttpContext ctx) =>
        {
            var body = await ReadJsonAsync(ctx);
            if (body == null) return Results.BadRequest(new { error = "Request body required" });
            var vaultId = body.RootElement.GetProperty("vault_id").GetString()!;
            var blobId = body.RootElement.GetProperty("blob_id").GetString()!;
            var (data, result) = transport.VaultBlobRead(vaultId, blobId);
            return Results.Json(new { error_code = (int)result, data = Convert.ToBase64String(data) });
        });

        apiGroup.MapPost("/sdk/vault/blob-delete", async (HttpContext ctx) =>
        {
            var body = await ReadJsonAsync(ctx);
            if (body == null) return Results.BadRequest(new { error = "Request body required" });
            var vaultId = body.RootElement.GetProperty("vault_id").GetString()!;
            var blobId = body.RootElement.GetProperty("blob_id").GetString()!;
            var result = transport.VaultBlobDelete(vaultId, blobId);
            return Results.Json(new { error_code = (int)result });
        });

        // ── Blob (unencrypted) operations ──

        apiGroup.MapPost("/sdk/blob/store", async (HttpContext ctx) =>
        {
            var body = await ReadJsonAsync(ctx);
            if (body == null) return Results.BadRequest(new { error = "Request body required" });
            var ns = body.RootElement.GetProperty("ns").GetString()!;
            var blobId = body.RootElement.GetProperty("blob_id").GetString()!;
            var dataB64 = body.RootElement.GetProperty("data").GetString()!;
            var data = Convert.FromBase64String(dataB64);
            string? metadataJson = null;
            if (body.RootElement.TryGetProperty("metadata", out var metaProp) && metaProp.ValueKind == JsonValueKind.String)
                metadataJson = metaProp.GetString();
            var result = transport.BlobStore(ns, blobId, data, metadataJson);
            return Results.Json(new { error_code = (int)result });
        });

        apiGroup.MapPost("/sdk/blob/read", async (HttpContext ctx) =>
        {
            var body = await ReadJsonAsync(ctx);
            if (body == null) return Results.BadRequest(new { error = "Request body required" });
            var ns = body.RootElement.GetProperty("ns").GetString()!;
            var blobId = body.RootElement.GetProperty("blob_id").GetString()!;
            var (data, result) = transport.BlobRead(ns, blobId);
            return Results.Json(new { error_code = (int)result, data = Convert.ToBase64String(data) });
        });

        apiGroup.MapPost("/sdk/blob/delete", async (HttpContext ctx) =>
        {
            var body = await ReadJsonAsync(ctx);
            if (body == null) return Results.BadRequest(new { error = "Request body required" });
            var ns = body.RootElement.GetProperty("ns").GetString()!;
            var blobId = body.RootElement.GetProperty("blob_id").GetString()!;
            var result = transport.BlobDelete(ns, blobId);
            return Results.Json(new { error_code = (int)result });
        });

        _log.Debug("Mapped SDK passthrough routes");
    }

    private static async Task<string?> ReadBodyAsync(HttpContext ctx)
    {
        using var reader = new StreamReader(ctx.Request.Body, Encoding.UTF8);
        var body = await reader.ReadToEndAsync();
        return string.IsNullOrEmpty(body) ? null : body;
    }

    private static async Task<JsonDocument?> ReadJsonAsync(HttpContext ctx)
    {
        var body = await ReadBodyAsync(ctx);
        if (body == null) return null;
        try { return JsonDocument.Parse(body); }
        catch { return null; }
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
