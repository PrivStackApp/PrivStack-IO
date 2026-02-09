namespace PrivStack.Sdk;

/// <summary>
/// Plugin-namespaced settings storage. Keys are automatically scoped to
/// the owning plugin ID to prevent collisions.
/// </summary>
public interface IPluginSettings
{
    /// <summary>
    /// Gets a setting value, returning <paramref name="defaultValue"/> if not found.
    /// </summary>
    T Get<T>(string key, T defaultValue);

    /// <summary>
    /// Sets a setting value. Persists automatically.
    /// </summary>
    void Set<T>(string key, T value);
}
