namespace PrivStack.Sdk.Services;

/// <summary>
/// Audio recording service for cross-platform microphone input.
/// Provides start/stop recording, duration tracking, and temp file output.
/// </summary>
public interface IAudioRecorderService
{
    /// <summary>Whether audio recording hardware is available.</summary>
    bool IsRecordingAvailable { get; }

    /// <summary>Whether a recording session is currently active.</summary>
    bool IsRecording { get; }

    /// <summary>Duration of the current recording session.</summary>
    TimeSpan RecordingDuration { get; }

    /// <summary>
    /// Start recording audio from the default microphone.
    /// Returns the temp file path where audio will be written.
    /// </summary>
    Task<string> StartRecordingAsync();

    /// <summary>
    /// Stop the current recording and return the path to the recorded audio file.
    /// </summary>
    Task<string> StopRecordingAsync();

    /// <summary>Cancel the current recording and discard the audio.</summary>
    void CancelRecording();

    /// <summary>Fired periodically while recording to update duration display.</summary>
    event EventHandler<TimeSpan>? DurationChanged;
}
