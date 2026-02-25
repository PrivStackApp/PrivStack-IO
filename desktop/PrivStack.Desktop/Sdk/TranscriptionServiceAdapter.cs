using PrivStack.Desktop.Services;
using PrivStack.Sdk.Services;

namespace PrivStack.Desktop.Sdk;

/// <summary>
/// Wraps the singleton WhisperService behind the SDK ITranscriptionService interface.
/// </summary>
internal sealed class TranscriptionServiceAdapter : ITranscriptionService
{
    public bool IsAvailable => WhisperService.Instance.IsAvailable();

    public Task<string> TranscribeFileAsync(string audioFilePath, CancellationToken ct = default)
    {
        return WhisperService.Instance.TranscribeAudioFileAsync(audioFilePath);
    }
}
