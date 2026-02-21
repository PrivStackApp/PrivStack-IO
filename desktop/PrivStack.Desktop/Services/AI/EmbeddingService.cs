using Microsoft.ML.OnnxRuntime;
using Microsoft.ML.OnnxRuntime.Tensors;
using Microsoft.ML.Tokenizers;
using Serilog;

namespace PrivStack.Desktop.Services.AI;

/// <summary>
/// Wraps ONNX Runtime inference for the nomic-embed-text-v1.5 embedding model.
/// Thread-safe via SemaphoreSlim(1) — same pattern as <see cref="LocalLlamaProvider"/>.
/// </summary>
internal sealed class EmbeddingService : IDisposable
{
    private static readonly ILogger _log = Log.ForContext<EmbeddingService>();
    private const int EmbeddingDim = 768;
    private const int MaxTokens = 8192;

    private readonly EmbeddingModelManager _modelManager;
    private readonly SemaphoreSlim _semaphore = new(1, 1);
    private InferenceSession? _session;
    private Tokenizer? _tokenizer;
    private bool _disposed;

    public bool IsReady => _session != null && _tokenizer != null;

    public EmbeddingService(EmbeddingModelManager modelManager)
    {
        _modelManager = modelManager;
    }

    /// <summary>
    /// Loads the ONNX model and tokenizer. No-op if already initialized.
    /// </summary>
    public async Task InitializeAsync(CancellationToken ct = default)
    {
        if (IsReady) return;
        if (!_modelManager.IsModelDownloaded)
        {
            _log.Warning("Embedding model not downloaded, skipping initialization");
            return;
        }

        await _semaphore.WaitAsync(ct);
        try
        {
            if (IsReady) return;

            _log.Information("Loading embedding model from {Path}", _modelManager.ModelPath);

            var sessionOptions = new SessionOptions
            {
                GraphOptimizationLevel = GraphOptimizationLevel.ORT_ENABLE_ALL,
                InterOpNumThreads = 1,
                IntraOpNumThreads = Environment.ProcessorCount > 4 ? 4 : 2,
            };

            _session = await Task.Run(() => new InferenceSession(_modelManager.ModelPath, sessionOptions), ct);

            _tokenizer = await Task.Run(() =>
            {
                using var vocabStream = File.OpenRead(_modelManager.VocabPath);
                return Microsoft.ML.Tokenizers.WordPieceTokenizer.Create(vocabStream);
            }, ct);

            _log.Information("Embedding model loaded ({Dim}-dim)", EmbeddingDim);
        }
        finally
        {
            _semaphore.Release();
        }
    }

    /// <summary>
    /// Embed a single text string.
    /// </summary>
    public async Task<double[]> EmbedAsync(string text, EmbeddingTaskType taskType, CancellationToken ct = default)
    {
        var results = await EmbedBatchAsync([text], taskType, ct);
        return results[0];
    }

    /// <summary>
    /// Embed a batch of text strings. Returns one L2-normalized vector per input.
    /// </summary>
    public async Task<double[][]> EmbedBatchAsync(string[] texts, EmbeddingTaskType taskType, CancellationToken ct = default)
    {
        if (!IsReady)
            throw new InvalidOperationException("Embedding service not initialized");

        await _semaphore.WaitAsync(ct);
        try
        {
            return await Task.Run(() =>
            {
                var results = new double[texts.Length][];
                for (var i = 0; i < texts.Length; i++)
                {
                    ct.ThrowIfCancellationRequested();
                    results[i] = EmbedSingle(texts[i], taskType);
                }
                return results;
            }, ct);
        }
        finally
        {
            _semaphore.Release();
        }
    }

    private double[] EmbedSingle(string text, EmbeddingTaskType taskType)
    {
        var prefix = taskType == EmbeddingTaskType.Document ? "search_document: " : "search_query: ";
        var prefixedText = prefix + text;

        // Tokenize
        var encoded = _tokenizer!.EncodeToIds(prefixedText);
        var tokenCount = Math.Min(encoded.Count, MaxTokens);
        var inputIds = new long[tokenCount];
        for (var i = 0; i < tokenCount; i++)
            inputIds[i] = encoded[i];

        var seqLen = inputIds.Length;

        // Attention mask (all 1s)
        var attentionMask = new long[seqLen];
        Array.Fill(attentionMask, 1L);

        // Token type IDs (all 0s)
        var tokenTypeIds = new long[seqLen];

        // Create tensors
        var inputIdsTensor = new DenseTensor<long>(inputIds, [1, seqLen]);
        var attentionMaskTensor = new DenseTensor<long>(attentionMask, [1, seqLen]);
        var tokenTypeIdsTensor = new DenseTensor<long>(tokenTypeIds, [1, seqLen]);

        var inputs = new List<NamedOnnxValue>
        {
            NamedOnnxValue.CreateFromTensor("input_ids", inputIdsTensor),
            NamedOnnxValue.CreateFromTensor("attention_mask", attentionMaskTensor),
            NamedOnnxValue.CreateFromTensor("token_type_ids", tokenTypeIdsTensor),
        };

        // Run inference
        using var results = _session!.Run(inputs);
        var outputTensor = results.First().AsTensor<float>();

        // Mean pooling over sequence dimension (output shape: [1, seqLen, 768])
        var embedding = new double[EmbeddingDim];
        for (var t = 0; t < seqLen; t++)
        {
            for (var d = 0; d < EmbeddingDim; d++)
            {
                embedding[d] += outputTensor[0, t, d];
            }
        }
        for (var d = 0; d < EmbeddingDim; d++)
            embedding[d] /= seqLen;

        // L2 normalize
        var norm = 0.0;
        for (var d = 0; d < EmbeddingDim; d++)
            norm += embedding[d] * embedding[d];
        norm = Math.Sqrt(norm);

        if (norm > 1e-12)
        {
            for (var d = 0; d < EmbeddingDim; d++)
                embedding[d] /= norm;
        }

        return embedding;
    }

    public void Dispose()
    {
        if (_disposed) return;
        _disposed = true;
        _session?.Dispose();
        _semaphore.Dispose();
    }
}
