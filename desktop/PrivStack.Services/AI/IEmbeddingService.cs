namespace PrivStack.Services.AI;

/// <summary>
/// Abstraction for text embedding. Desktop provides the ONNX implementation;
/// Server can supply a no-op or remote implementation.
/// </summary>
public interface IEmbeddingService
{
    bool IsReady { get; }
    Task InitializeAsync(CancellationToken ct = default);
    Task UnloadAsync();
    Task<double[]> EmbedAsync(string text, EmbeddingTaskType taskType, CancellationToken ct = default);
    Task<double[][]> EmbedBatchAsync(string[] texts, EmbeddingTaskType taskType, CancellationToken ct = default);
}
