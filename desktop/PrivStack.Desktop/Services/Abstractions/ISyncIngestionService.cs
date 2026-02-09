namespace PrivStack.Desktop.Services.Abstractions;

/// <summary>
/// Abstraction over sync event polling and ingestion.
/// </summary>
public interface ISyncIngestionService
{
    void Start();
    void Stop();
}
