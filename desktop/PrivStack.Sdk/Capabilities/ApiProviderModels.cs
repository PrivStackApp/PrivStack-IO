namespace PrivStack.Sdk.Capabilities;

/// <summary>
/// HTTP method for an API route.
/// </summary>
public enum ApiMethod
{
    Get,
    Post,
    Put,
    Patch,
    Delete,
}

/// <summary>
/// Describes a single API route exposed by a plugin.
/// </summary>
public sealed class ApiRouteDescriptor
{
    /// <summary>
    /// Unique identifier for this route within the plugin (e.g. "list_tasks", "get_task").
    /// Passed back in <see cref="ApiRequest.RouteId"/> so the plugin can dispatch.
    /// </summary>
    public required string RouteId { get; init; }

    /// <summary>
    /// HTTP method for this route.
    /// </summary>
    public required ApiMethod Method { get; init; }

    /// <summary>
    /// Relative path pattern after the plugin slug.
    /// Use empty string for the root, "{id}" for path parameters, "search" for static segments.
    /// Examples: "", "{id}", "search", "projects", "projects/{id}".
    /// </summary>
    public required string Path { get; init; }

    /// <summary>
    /// Human-readable description shown in the /api/v1/routes listing.
    /// </summary>
    public string? Description { get; init; }
}

/// <summary>
/// Incoming API request passed to <see cref="IApiProvider.HandleRequestAsync"/>.
/// The shell constructs this from the HTTP request — plugins never touch Kestrel types.
/// </summary>
public sealed class ApiRequest
{
    /// <summary>
    /// The <see cref="ApiRouteDescriptor.RouteId"/> that matched this request.
    /// </summary>
    public required string RouteId { get; init; }

    /// <summary>
    /// Path parameters extracted from the route pattern (e.g. {"id": "abc-123"}).
    /// </summary>
    public IReadOnlyDictionary<string, string> PathParams { get; init; } =
        new Dictionary<string, string>();

    /// <summary>
    /// Query string parameters (e.g. {"q": "groceries", "status": "active"}).
    /// </summary>
    public IReadOnlyDictionary<string, string> QueryParams { get; init; } =
        new Dictionary<string, string>();

    /// <summary>
    /// Raw JSON request body, or null for bodyless requests (GET/DELETE).
    /// </summary>
    public string? Body { get; init; }
}

/// <summary>
/// Response from a plugin's API handler.
/// The shell writes this to the HTTP response.
/// </summary>
public sealed class ApiResponse
{
    public int StatusCode { get; init; }
    public string? Body { get; init; }
    public string ContentType { get; init; } = "application/json";

    public static ApiResponse Ok(string? json = null) =>
        new() { StatusCode = 200, Body = json };

    public static ApiResponse Created(string? json = null) =>
        new() { StatusCode = 201, Body = json };

    public static ApiResponse NoContent() =>
        new() { StatusCode = 204 };

    public static ApiResponse BadRequest(string message) =>
        new() { StatusCode = 400, Body = $$"""{"error":"{{EscapeJson(message)}}"}""" };

    public static ApiResponse NotFound(string? message = null) =>
        new()
        {
            StatusCode = 404,
            Body = message != null
                ? $$"""{"error":"{{EscapeJson(message)}}"}"""
                : """{"error":"Not found"}""",
        };

    public static ApiResponse Error(int statusCode, string message) =>
        new() { StatusCode = statusCode, Body = $$"""{"error":"{{EscapeJson(message)}}"}""" };

    private static string EscapeJson(string s) =>
        s.Replace("\\", "\\\\").Replace("\"", "\\\"");
}
