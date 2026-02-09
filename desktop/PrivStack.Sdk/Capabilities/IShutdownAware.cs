namespace PrivStack.Sdk.Capabilities;

/// <summary>
/// Capability interface for plugins or ViewModels that need to perform cleanup on app shutdown.
/// </summary>
public interface IShutdownAware
{
    void OnShutdown();
}
