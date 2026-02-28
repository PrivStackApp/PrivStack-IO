using System.ComponentModel;
using PrivStack.Services.Models;
using PrivStack.Services.Abstractions;
using PrivStack.Sdk;
using PrivStack.Sdk.Services;
using IToastService = PrivStack.Sdk.IToastService;

#pragma warning disable CS0067 // Event is never used — required by interface contracts

namespace PrivStack.Server;

/// <summary>
/// Executes actions synchronously on the calling thread. No UI dispatcher exists in headless mode.
/// </summary>
internal sealed class HeadlessUiDispatcher : IUiDispatcher
{
    public void Post(Action action) => action();

    public Task InvokeAsync(Action action)
    {
        action();
        return Task.CompletedTask;
    }

    public Task InvokeAsync(Func<Task> action) => action();
}

/// <summary>
/// Returns null/false for all dialog operations. No UI dialogs in headless mode.
/// </summary>
internal sealed class HeadlessDialogService : IDialogService
{
    public Task<bool> ShowConfirmationAsync(string title, string message, string confirmButtonText = "Confirm") => Task.FromResult(false);
    public Task<string?> ShowPasswordConfirmationAsync(string title, string message, string confirmButtonText = "Confirm") => Task.FromResult<string?>(null);
    public Task<string?> ShowOpenFileDialogAsync(string title, (string Name, string Extension)[] filters) => Task.FromResult<string?>(null);
    public Task<string?> ShowSaveFileDialogAsync(string title, string defaultFileName, (string Name, string Extension)[] filters) => Task.FromResult<string?>(null);
    public Task<string?> ShowOpenFolderDialogAsync(string title) => Task.FromResult<string?>(null);
}

/// <summary>
/// No-op SDK dialog service for headless mode.
/// </summary>
internal sealed class HeadlessSdkDialogService : ISdkDialogService
{
    public Task<bool> ShowConfirmationAsync(string title, string message, string confirmButtonText = "Confirm") => Task.FromResult(false);
    public Task<string?> ShowOpenFileDialogAsync(string title, (string Name, string Extension)[] filters) => Task.FromResult<string?>(null);
    public Task<string?> ShowSaveFileDialogAsync(string title, string defaultFileName, (string Name, string Extension)[] filters) => Task.FromResult<string?>(null);
    public Task<string?> ShowOpenFolderDialogAsync(string title) => Task.FromResult<string?>(null);
    public Task<TResult?> ShowDialogAsync<TResult>(Func<object> windowFactory) where TResult : class => Task.FromResult<TResult?>(null);
}

/// <summary>
/// Logs toast messages at Debug level. No visual toasts in headless mode.
/// </summary>
internal sealed class HeadlessToastService : IToastService
{
    private static readonly Serilog.ILogger _log = Serilog.Log.ForContext<HeadlessToastService>();

    public void Show(string message, ToastType type = ToastType.Info)
        => _log.Debug("[Toast:{Type}] {Message}", type, message);

    public void Show(string message, ToastType type, string actionLabel, Action action)
        => _log.Debug("[Toast:{Type}] {Message} (action: {Action})", type, message, actionLabel);
}

/// <summary>
/// Returns 1.0 scale. No font scaling in headless mode.
/// </summary>
internal sealed class HeadlessFontScaleService : IFontScaleService
{
    public double ScaleMultiplier { get => 1.0; set { } }
    public string ScaleDisplayText => "100%";
    public string CurrentFontFamily { get => "Inter"; set { } }
    public void Initialize() { }
    public void ReapplyScale() { }
    public double GetScaledSize(double baseSize) => baseSize;

    public event PropertyChangedEventHandler? PropertyChanged;
}

/// <summary>
/// Returns Desktop layout mode. No responsive layout in headless mode.
/// </summary>
internal sealed class HeadlessResponsiveLayoutService : IResponsiveLayoutService
{
    public LayoutMode CurrentMode => LayoutMode.Wide;
    public double ContentAreaWidth => 1920;
    public void UpdateContentAreaWidth(double width) { }
    public void Initialize() { }
    public void ReapplyLayout() { }

    public event PropertyChangedEventHandler? PropertyChanged;
}

/// <summary>
/// Returns false for all notification attempts. No system notifications in headless mode.
/// </summary>
internal sealed class HeadlessSystemNotificationService : ISystemNotificationService
{
    public Task<bool> SendNotificationAsync(string title, string body, string? subtitle = null, bool playSound = true)
        => Task.FromResult(false);
}

/// <summary>
/// Returns false for focus mode. No focus mode in headless mode.
/// </summary>
internal sealed class HeadlessFocusModeService : IFocusModeService
{
    public bool IsFocusMode => false;
    public event Action<bool>? FocusModeChanged;
    public void SetFocusMode(bool enabled) { }
}

/// <summary>
/// No-op audio recording for headless mode.
/// </summary>
internal sealed class HeadlessAudioRecorderService : IAudioRecorderService
{
    public bool IsRecordingAvailable => false;
    public bool IsRecording => false;
    public TimeSpan RecordingDuration => TimeSpan.Zero;
    public Task<string> StartRecordingAsync() => Task.FromResult(string.Empty);
    public Task<string> StopRecordingAsync() => Task.FromResult(string.Empty);
    public void CancelRecording() { }
    public event EventHandler<TimeSpan>? DurationChanged;
}

/// <summary>
/// No-op transcription for headless mode.
/// </summary>
internal sealed class HeadlessTranscriptionService : ITranscriptionService
{
    public bool IsAvailable => false;
    public Task<string> TranscribeFileAsync(string audioFilePath, CancellationToken ct = default) => Task.FromResult(string.Empty);
}

/// <summary>
/// No-op backup service for headless mode.
/// </summary>
internal sealed class HeadlessBackupService : IBackupService
{
    public string DataDirectory => PrivStack.Services.DataPaths.BaseDir;
    public string BackupDirectory => Path.Combine(PrivStack.Services.DataPaths.BaseDir, "backups");
    public event EventHandler<BackupCompletedEventArgs>? BackupCompleted;
    public Task<string?> BackupNowAsync() => Task.FromResult<string?>(null);
    public IEnumerable<BackupInfo> GetExistingBackups() => [];
    public Task<bool> RestoreBackupAsync(string backupPath) => Task.FromResult(false);
    public void StartScheduledBackups() { }
    public void StopScheduledBackups() { }
    public void UpdateBackupDirectory(string newPath) { }
    public void UpdateBackupFrequency(BackupFrequency frequency) { }
}

/// <summary>
/// No-op embedding service for headless mode (no ONNX model).
/// </summary>
internal sealed class HeadlessEmbeddingService : PrivStack.Services.AI.IEmbeddingService
{
    public bool IsReady => false;
    public Task InitializeAsync(CancellationToken ct = default) => Task.CompletedTask;
    public Task<double[]> EmbedAsync(string text, PrivStack.Services.AI.EmbeddingTaskType taskType, CancellationToken ct = default) => Task.FromResult(Array.Empty<double>());
    public Task<double[][]> EmbedBatchAsync(string[] texts, PrivStack.Services.AI.EmbeddingTaskType taskType, CancellationToken ct = default) => Task.FromResult(Array.Empty<double[]>());
}
