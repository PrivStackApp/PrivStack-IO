using System.ComponentModel;
using PrivStack.Desktop.Services;
using PrivStack.Sdk.Services;

namespace PrivStack.Desktop.Sdk;

/// <summary>
/// Wraps the singleton AudioRecorderService behind the SDK IAudioRecorderService interface.
/// Translates PropertyChanged events for RecordingDuration into DurationChanged events.
/// </summary>
internal sealed class AudioRecorderServiceAdapter : IAudioRecorderService, IDisposable
{
    private readonly AudioRecorderService _inner;

    public AudioRecorderServiceAdapter()
    {
        _inner = AudioRecorderService.Instance;
        _inner.PropertyChanged += OnInnerPropertyChanged;
    }

    public bool IsRecordingAvailable => _inner.IsRecordingAvailable();
    public bool IsRecording => _inner.IsRecording;
    public TimeSpan RecordingDuration => _inner.RecordingDuration;

    public Task<string> StartRecordingAsync() => _inner.StartRecordingAsync();
    public Task<string> StopRecordingAsync() => _inner.StopRecordingAsync();
    public void CancelRecording() => _inner.CancelRecording();

    public event EventHandler<TimeSpan>? DurationChanged;

    private void OnInnerPropertyChanged(object? sender, PropertyChangedEventArgs e)
    {
        if (e.PropertyName == nameof(AudioRecorderService.RecordingDuration))
            DurationChanged?.Invoke(this, _inner.RecordingDuration);
    }

    public void Dispose()
    {
        _inner.PropertyChanged -= OnInnerPropertyChanged;
    }
}
