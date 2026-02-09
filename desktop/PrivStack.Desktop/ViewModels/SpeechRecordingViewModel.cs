using System.Runtime.InteropServices;
using CommunityToolkit.Mvvm.ComponentModel;
using CommunityToolkit.Mvvm.Input;
using PrivStack.Desktop.Services;
using PrivStack.Desktop.Services.Abstractions;

namespace PrivStack.Desktop.ViewModels;

/// <summary>
/// ViewModel for the speech recording overlay.
/// Handles recording states, download prompts, and transcription.
/// </summary>
public partial class SpeechRecordingViewModel : ViewModelBase
{
    private readonly WhisperService _whisperService;
    private readonly WhisperModelManager _modelManager;
    private readonly IAppSettingsService _appSettings;

    [ObservableProperty]
    private bool _isVisible;

    [ObservableProperty]
    private bool _isRecording;

    [ObservableProperty]
    private bool _isTranscribing;

    [ObservableProperty]
    private bool _isPromptingDownload;

    [ObservableProperty]
    private bool _isDownloading;

    [ObservableProperty]
    private double _downloadProgress;

    [ObservableProperty]
    private TimeSpan _duration;

    [ObservableProperty]
    private string _statusText = "";

    [ObservableProperty]
    private string? _errorMessage;

    public string ShortcutText => RuntimeInformation.IsOSPlatform(OSPlatform.OSX) ? "Cmd+M" : "Ctrl+M";

    /// <summary>
    /// Gets the model name for display in download prompt.
    /// </summary>
    public string ModelDisplayName => _modelManager.GetDefaultModelName();

    /// <summary>
    /// Gets the model size for display in download prompt.
    /// </summary>
    public string ModelSizeDisplay => _modelManager.GetModelSizeDisplay(_modelManager.GetDefaultModelName());

    public event EventHandler<string>? TranscriptionReady;
    public event EventHandler? Cancelled;

    public SpeechRecordingViewModel(IAppSettingsService appSettings)
    {
        _appSettings = appSettings;
        _whisperService = WhisperService.Instance;
        _modelManager = WhisperModelManager.Instance;

        // Subscribe to service events
        _whisperService.PropertyChanged += (_, e) =>
        {
            switch (e.PropertyName)
            {
                case nameof(WhisperService.IsRecording):
                    IsRecording = _whisperService.IsRecording;
                    UpdateStatus();
                    break;
                case nameof(WhisperService.IsTranscribing):
                    IsTranscribing = _whisperService.IsTranscribing;
                    UpdateStatus();
                    break;
                case nameof(WhisperService.RecordingDuration):
                    Duration = _whisperService.RecordingDuration;
                    break;
                case nameof(WhisperService.ErrorMessage):
                    ErrorMessage = _whisperService.ErrorMessage;
                    break;
            }
        };

        _whisperService.TranscriptionCompleted += (_, text) =>
        {
            IsVisible = false;
            TranscriptionReady?.Invoke(this, text);
        };

        _whisperService.Error += (_, error) =>
        {
            ErrorMessage = error;
        };

        // Subscribe to model manager for download progress
        _modelManager.PropertyChanged += (_, e) =>
        {
            if (e.PropertyName == nameof(WhisperModelManager.DownloadProgress))
            {
                DownloadProgress = _modelManager.DownloadProgress;
            }
        };
    }

    /// <summary>
    /// Checks if speech-to-text is ready to use (model downloaded and feature enabled).
    /// </summary>
    public bool IsReady
    {
        get
        {
            var settings = _appSettings.Settings;
            if (!settings.SpeechToTextEnabled) return false;

            var modelName = _modelManager.GetDefaultModelName();
            return _modelManager.IsModelDownloaded(modelName);
        }
    }

    /// <summary>
    /// Attempts to start speech-to-text. Shows download prompt if model not available.
    /// </summary>
    public async Task TryStartAsync()
    {
        ErrorMessage = null;

        // Check if feature is enabled
        var settings = _appSettings.Settings;
        if (!settings.SpeechToTextEnabled)
        {
            // Feature disabled - don't do anything
            return;
        }

        // Check if model is downloaded
        var modelName = _modelManager.GetDefaultModelName();
        if (!_modelManager.IsModelDownloaded(modelName))
        {
            // Show download prompt
            IsPromptingDownload = true;
            IsVisible = true;
            StatusText = "Speech model required";
            return;
        }

        // Model ready - start recording
        await StartRecordingAsync();
    }

    /// <summary>
    /// Starts or stops recording based on current state.
    /// </summary>
    public async Task ToggleRecordingAsync()
    {
        ErrorMessage = null;

        if (IsPromptingDownload || IsDownloading)
        {
            // In download flow, don't toggle
            return;
        }

        if (IsTranscribing)
        {
            // Already transcribing, can't toggle
            return;
        }

        if (IsRecording)
        {
            // Stop recording and transcribe
            await StopAndTranscribeAsync();
        }
        else
        {
            // Start recording
            await StartRecordingAsync();
        }
    }

    /// <summary>
    /// User confirmed they want to download the model.
    /// </summary>
    [RelayCommand]
    private async Task ConfirmDownloadAsync()
    {
        IsPromptingDownload = false;
        IsDownloading = true;
        DownloadProgress = 0;
        StatusText = "Downloading speech model...";

        try
        {
            var modelName = _modelManager.GetDefaultModelName();
            await _modelManager.DownloadModelAsync(modelName);

            // Download complete - now start recording
            IsDownloading = false;
            StatusText = "Model ready!";

            // Small delay to show success, then start recording
            await Task.Delay(500);
            await StartRecordingAsync();
        }
        catch (OperationCanceledException)
        {
            IsDownloading = false;
            IsVisible = false;
            StatusText = "";
        }
        catch (Exception ex)
        {
            IsDownloading = false;
            ErrorMessage = $"Download failed: {ex.Message}";
            StatusText = "Download failed";
        }
    }

    /// <summary>
    /// User declined the download.
    /// </summary>
    [RelayCommand]
    private void DeclineDownload()
    {
        IsPromptingDownload = false;
        IsVisible = false;
        StatusText = "";
        Cancelled?.Invoke(this, EventArgs.Empty);
    }

    /// <summary>
    /// Cancel an in-progress download.
    /// </summary>
    [RelayCommand]
    private void CancelDownload()
    {
        _modelManager.CancelDownload();
        IsDownloading = false;
        IsVisible = false;
        StatusText = "";
    }

    /// <summary>
    /// Starts recording.
    /// </summary>
    public async Task StartRecordingAsync()
    {
        try
        {
            ErrorMessage = null;

            // Initialize model if needed
            if (!_whisperService.IsModelLoaded)
            {
                StatusText = "Loading model...";
                IsVisible = true;
                await _whisperService.InitializeAsync();
            }

            if (!_whisperService.IsModelLoaded)
            {
                // Model failed to load - error message is already set
                ErrorMessage = _whisperService.ErrorMessage ?? "Failed to load speech model.";
                return;
            }

            await _whisperService.StartRecordingAsync();
            IsVisible = true;
            UpdateStatus();
        }
        catch (Exception ex)
        {
            ErrorMessage = ex.Message;
            IsVisible = false;
        }
    }

    /// <summary>
    /// Stops recording and starts transcription.
    /// </summary>
    public async Task StopAndTranscribeAsync()
    {
        try
        {
            ErrorMessage = null;
            UpdateStatus();

            await _whisperService.StopRecordingAndTranscribeAsync();
            // TranscriptionReady event will be fired when complete
        }
        catch (Exception ex)
        {
            ErrorMessage = ex.Message;
            IsVisible = false;
        }
    }

    [RelayCommand]
    private void Cancel()
    {
        if (IsDownloading)
        {
            CancelDownload();
            return;
        }

        _whisperService.CancelRecording();
        IsPromptingDownload = false;
        IsVisible = false;
        ErrorMessage = null;
        Cancelled?.Invoke(this, EventArgs.Empty);
    }

    private void UpdateStatus()
    {
        if (IsTranscribing)
        {
            StatusText = "Transcribing...";
        }
        else if (IsRecording)
        {
            StatusText = $"Recording... Press {ShortcutText} to stop";
        }
        else
        {
            StatusText = "Ready";
        }
    }
}
