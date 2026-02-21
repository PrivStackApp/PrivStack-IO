using System.ComponentModel;
using Serilog;

namespace PrivStack.Desktop.Services.AI;

/// <summary>
/// Manages download and lifecycle of the nomic-embed-text-v1.5 ONNX model + tokenizer.
/// Follows the same temp-file-and-rename pattern as <see cref="AiModelManager"/>.
/// </summary>
public sealed class EmbeddingModelManager : INotifyPropertyChanged
{
    private static readonly ILogger _log = Log.ForContext<EmbeddingModelManager>();

    private const string ModelFileName = "model.onnx";
    private const string VocabFileName = "vocab.txt";
    private const string ModelUrl = "https://huggingface.co/nomic-ai/nomic-embed-text-v1.5/resolve/main/onnx/model.onnx";
    private const string VocabUrl = "https://huggingface.co/nomic-ai/nomic-embed-text-v1.5/resolve/main/vocab.txt";
    private const long ApproxModelSizeBytes = 270_000_000;

    private string? _cachedModelsDirectory;
    private readonly HttpClient _httpClient;
    private bool _isDownloading;
    private double _downloadProgress;
    private CancellationTokenSource? _downloadCts;

    public event PropertyChangedEventHandler? PropertyChanged;
    public event EventHandler<EventArgs>? DownloadCompleted;
    public event EventHandler<Exception>? DownloadFailed;

    public string ModelsDirectory
    {
        get
        {
            var wsDir = DataPaths.WorkspaceDataDir;
            var target = wsDir != null
                ? Path.Combine(wsDir, "models", "embedding")
                : Path.Combine(DataPaths.BaseDir, "models", "embedding");

            if (_cachedModelsDirectory != target)
            {
                _cachedModelsDirectory = target;
                Directory.CreateDirectory(target);
            }

            return target;
        }
    }

    public bool IsDownloading
    {
        get => _isDownloading;
        private set { if (_isDownloading != value) { _isDownloading = value; OnPropertyChanged(nameof(IsDownloading)); } }
    }

    public double DownloadProgress
    {
        get => _downloadProgress;
        private set { if (Math.Abs(_downloadProgress - value) > 0.001) { _downloadProgress = value; OnPropertyChanged(nameof(DownloadProgress)); } }
    }

    public string ModelPath => Path.Combine(ModelsDirectory, ModelFileName);
    public string VocabPath => Path.Combine(ModelsDirectory, VocabFileName);
    public bool IsModelDownloaded => File.Exists(ModelPath) && File.Exists(VocabPath);

    public string ModelSizeDisplay => $"{ApproxModelSizeBytes / 1_000_000.0:F0} MB";

    public EmbeddingModelManager()
    {
        _httpClient = new HttpClient();
        _httpClient.DefaultRequestHeaders.Add("User-Agent", "PrivStack/1.0");
    }

    public async Task DownloadModelAsync(CancellationToken cancellationToken = default)
    {
        if (IsDownloading)
            throw new InvalidOperationException("A download is already in progress");

        _downloadCts = CancellationTokenSource.CreateLinkedTokenSource(cancellationToken);

        try
        {
            IsDownloading = true;
            DownloadProgress = 0;

            // Download vocab first (small)
            _log.Information("Downloading embedding vocab from {Url}", VocabUrl);
            await DownloadFileAsync(VocabUrl, VocabPath, 0, 5, _downloadCts.Token);

            // Download model (large)
            _log.Information("Downloading embedding model from {Url}", ModelUrl);
            await DownloadFileAsync(ModelUrl, ModelPath, 5, 100, _downloadCts.Token);

            DownloadProgress = 100;
            _log.Information("Successfully downloaded embedding model to {Dir}", ModelsDirectory);
            DownloadCompleted?.Invoke(this, EventArgs.Empty);
        }
        catch (OperationCanceledException)
        {
            _log.Information("Embedding model download was cancelled");
            throw;
        }
        catch (Exception ex)
        {
            _log.Error(ex, "Failed to download embedding model");
            DownloadFailed?.Invoke(this, ex);
            throw;
        }
        finally
        {
            IsDownloading = false;
            _downloadCts?.Dispose();
            _downloadCts = null;
        }
    }

    private async Task DownloadFileAsync(
        string url, string destPath, double progressStart, double progressEnd, CancellationToken ct)
    {
        var tempPath = destPath + ".tmp";
        try
        {
            using var response = await _httpClient.GetAsync(url, HttpCompletionOption.ResponseHeadersRead, ct);
            response.EnsureSuccessStatusCode();

            var totalBytes = response.Content.Headers.ContentLength ?? ApproxModelSizeBytes;

            await using var contentStream = await response.Content.ReadAsStreamAsync(ct);
            await using var fileStream = new FileStream(tempPath, FileMode.Create, FileAccess.Write, FileShare.None, 81920, true);

            var buffer = new byte[81920];
            long totalBytesRead = 0;
            int bytesRead;

            while ((bytesRead = await contentStream.ReadAsync(buffer, ct)) > 0)
            {
                await fileStream.WriteAsync(buffer.AsMemory(0, bytesRead), ct);
                totalBytesRead += bytesRead;
                var fraction = (double)totalBytesRead / totalBytes;
                DownloadProgress = progressStart + fraction * (progressEnd - progressStart);
            }

            if (File.Exists(destPath)) File.Delete(destPath);
            File.Move(tempPath, destPath);
        }
        catch
        {
            if (File.Exists(tempPath)) try { File.Delete(tempPath); } catch { /* ignore */ }
            throw;
        }
    }

    public void CancelDownload() => _downloadCts?.Cancel();

    public void DeleteModel()
    {
        if (File.Exists(ModelPath)) File.Delete(ModelPath);
        if (File.Exists(VocabPath)) File.Delete(VocabPath);
        _log.Information("Deleted embedding model files");
    }

    private void OnPropertyChanged(string propertyName) =>
        PropertyChanged?.Invoke(this, new PropertyChangedEventArgs(propertyName));
}
