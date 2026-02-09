using CommunityToolkit.Mvvm.ComponentModel;
using CommunityToolkit.Mvvm.Input;
using PrivStack.Desktop.Models;
using PrivStack.Desktop.Services;
using PrivStack.Desktop.Services.Abstractions;

namespace PrivStack.Desktop.ViewModels;

/// <summary>
/// ViewModel for managing application updates via the PrivStack registry API.
/// </summary>
public partial class UpdateViewModel : ViewModelBase
{
    private readonly IUpdateService _updateService;
    private readonly IDialogService _dialogService;
    private readonly IUiDispatcher _dispatcher;
    private readonly IAppSettingsService _appSettings;
    private System.Timers.Timer? _autoCheckTimer;

    [ObservableProperty]
    private string _currentVersion = "0.0.0";

    [ObservableProperty]
    private bool _isChecking;

    [ObservableProperty]
    private bool _isDownloading;

    [ObservableProperty]
    private bool _updateAvailable;

    [ObservableProperty]
    private string _updateVersion = string.Empty;

    [ObservableProperty]
    private int _downloadProgress;

    [ObservableProperty]
    private string _statusMessage = string.Empty;

    [ObservableProperty]
    private bool _updateReady;

    [ObservableProperty]
    private bool _needsAuthentication;

    public UpdateViewModel(
        IUpdateService updateService,
        IDialogService dialogService,
        IUiDispatcher dispatcher,
        IAppSettingsService appSettings)
    {
        _updateService = updateService;
        _dialogService = dialogService;
        _dispatcher = dispatcher;
        _appSettings = appSettings;

        CurrentVersion = _updateService.CurrentVersion;

        _updateService.UpdateFound += OnUpdateFound;
        _updateService.UpdateError += OnUpdateError;
    }

    private void OnUpdateFound(object? sender, LatestReleaseInfo release)
    {
        _dispatcher.Post(() =>
        {
            UpdateAvailable = true;
            UpdateVersion = release.Version;
            StatusMessage = $"Version {release.Version} available";
        });
    }

    private void OnUpdateError(object? sender, Exception ex)
    {
        _dispatcher.Post(() =>
        {
            IsChecking = false;
            IsDownloading = false;
            StatusMessage = $"Update error: {ex.Message}";
        });
    }

    [RelayCommand]
    private async Task CheckForUpdatesAsync()
    {
        if (IsChecking) return;

        IsChecking = true;
        StatusMessage = "Checking for updates...";

        try
        {
            var release = await _updateService.CheckForUpdatesAsync();

            if (release == null)
            {
                StatusMessage = "You're up to date";
                UpdateAvailable = false;
            }
        }
        finally
        {
            IsChecking = false;
        }
    }

    [RelayCommand]
    private async Task DownloadUpdateAsync()
    {
        if (IsDownloading || !UpdateAvailable) return;

        // Check for stored access token
        if (string.IsNullOrEmpty(_appSettings.Settings.AccessToken))
        {
            NeedsAuthentication = true;
            StatusMessage = "Sign in required to download updates";
            return;
        }

        NeedsAuthentication = false;
        IsDownloading = true;
        DownloadProgress = 0;

        try
        {
            var progress = new Progress<int>(p =>
            {
                _dispatcher.Post(() =>
                {
                    DownloadProgress = p;
                    StatusMessage = $"Downloading... {p}%";
                });
            });

            var filePath = await _updateService.DownloadUpdateAsync(progress);

            if (filePath != null)
            {
                UpdateReady = true;
                StatusMessage = "Update ready to install";
            }
            else
            {
                StatusMessage = "Download failed";
            }
        }
        finally
        {
            IsDownloading = false;
        }
    }

    [RelayCommand]
    private async Task InstallAndRestartAsync()
    {
        if (!UpdateReady) return;

        StatusMessage = "Installing update...";

        var success = await _updateService.ApplyUpdateAndRestartAsync();

        if (!success)
        {
            await _dialogService.ShowConfirmationAsync(
                "Restart Required",
                "The update has been downloaded. Please restart PrivStack to apply the update.",
                "OK");
        }
    }

    /// <summary>
    /// Starts automatic update checking on a timer.
    /// </summary>
    public void StartAutoCheck(TimeSpan interval)
    {
        if (!_appSettings.Settings.AutoCheckForUpdates)
            return;

        StopAutoCheck();

        _autoCheckTimer = new System.Timers.Timer(interval.TotalMilliseconds);
        _autoCheckTimer.Elapsed += async (_, _) =>
        {
            await _dispatcher.InvokeAsync(async () =>
            {
                await CheckForUpdatesAsync();
            });
        };
        _autoCheckTimer.AutoReset = true;
        _autoCheckTimer.Start();

        // Also check immediately
        _ = CheckForUpdatesAsync();
    }

    /// <summary>
    /// Stops automatic update checking.
    /// </summary>
    public void StopAutoCheck()
    {
        _autoCheckTimer?.Stop();
        _autoCheckTimer?.Dispose();
        _autoCheckTimer = null;
    }

    public void Cleanup()
    {
        StopAutoCheck();
        _updateService.UpdateFound -= OnUpdateFound;
        _updateService.UpdateError -= OnUpdateError;
    }
}
