namespace PrivStack.Desktop.Native;

/// <summary>
/// Manages PrivStack native library lifecycle: initialization and shutdown.
/// </summary>
public interface IPrivStackRuntime : IDisposable
{
    bool IsInitialized { get; }
    string NativeVersion { get; }
    void Initialize(string dbPath);
    void Shutdown();
}
