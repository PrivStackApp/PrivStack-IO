namespace PrivStack.Sdk;

/// <summary>
/// Structured logger for plugin diagnostics. Wraps the host logging infrastructure
/// with the plugin ID automatically in context.
/// </summary>
public interface IPluginLogger
{
    void Debug(string messageTemplate, params object[] args);
    void Info(string messageTemplate, params object[] args);
    void Warn(string messageTemplate, params object[] args);
    void Error(string messageTemplate, params object[] args);
    void Error(Exception ex, string messageTemplate, params object[] args);
}
