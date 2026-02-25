namespace PrivStack.Sdk.Services;

/// <summary>
/// Speech-to-text transcription service.
/// Wraps Whisper or similar local transcription engine.
/// </summary>
public interface ITranscriptionService
{
    /// <summary>Whether the transcription engine is loaded and ready.</summary>
    bool IsAvailable { get; }

    /// <summary>
    /// Transcribe an audio file to text.
    /// </summary>
    /// <param name="audioFilePath">Path to the audio file (WAV, MP3, etc.).</param>
    /// <param name="ct">Cancellation token.</param>
    /// <returns>Transcribed text.</returns>
    Task<string> TranscribeFileAsync(string audioFilePath, CancellationToken ct = default);
}
