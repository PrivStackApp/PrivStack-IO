namespace PrivStack.Sdk.Capabilities;

/// <summary>
/// Capability interface for plugins that expose local HTTP API endpoints.
/// Plugins declare routes and handle requests via SDK DTOs — they never touch Kestrel types.
/// The shell hosts the Kestrel server and routes requests to the appropriate provider.
/// </summary>
public interface IApiProvider
{
    /// <summary>
    /// URL slug for this plugin's routes (e.g. "tasks", "notes").
    /// Routes are mounted at /api/v1/{ApiSlug}/{path}.
    /// </summary>
    string ApiSlug { get; }

    /// <summary>
    /// Returns all route descriptors this plugin handles.
    /// Called once at server startup to build the route table.
    /// </summary>
    IReadOnlyList<ApiRouteDescriptor> GetRoutes();

    /// <summary>
    /// Handles an incoming API request and returns a response.
    /// The shell constructs <see cref="ApiRequest"/> from the HTTP request
    /// and writes <see cref="ApiResponse"/> back to the HTTP response.
    /// </summary>
    Task<ApiResponse> HandleRequestAsync(ApiRequest request, CancellationToken ct = default);
}
