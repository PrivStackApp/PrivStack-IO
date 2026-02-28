using CommunityToolkit.Mvvm.ComponentModel;
using CommunityToolkit.Mvvm.Input;
using PrivStack.Desktop.Services;
using PrivStack.Desktop.Services.Abstractions;
using PrivStack.Services.Biometric;
using Serilog;

namespace PrivStack.Desktop.ViewModels;

/// <summary>
/// ViewModel for the sensitive data unlock overlay.
/// Handles password entry and validation for re-authentication.
/// </summary>
public partial class SensitiveUnlockViewModel : ObservableObject
{
    private static readonly ILogger _log = Serilog.Log.ForContext<SensitiveUnlockViewModel>();

    private readonly ISensitiveLockService _sensitiveLockService;
    private readonly IBiometricService? _biometricService;
    private readonly IAppSettingsService? _appSettings;

    public SensitiveUnlockViewModel(
        ISensitiveLockService sensitiveLockService,
        IBiometricService? biometricService = null,
        IAppSettingsService? appSettings = null)
    {
        _sensitiveLockService = sensitiveLockService;
        _biometricService = biometricService;
        _appSettings = appSettings;
    }

    [ObservableProperty]
    private string _password = string.Empty;

    [ObservableProperty]
    private string? _errorMessage;

    [ObservableProperty]
    private bool _isUnlocking;

    [ObservableProperty]
    private bool _isBiometricAvailable;

    [ObservableProperty]
    private string _biometricDisplayName = string.Empty;

    [ObservableProperty]
    private bool _isBiometricAuthenticating;

    /// <summary>
    /// Checks biometric availability for the re-auth overlay.
    /// </summary>
    public async Task InitializeBiometricAsync()
    {
        if (_biometricService == null || !_biometricService.IsSupported) return;

        var available = await _biometricService.IsAvailableAsync();
        var enrolled = _biometricService.IsEnrolled;
        var enabled = _appSettings?.Settings.BiometricUnlockEnabled == true;

        IsBiometricAvailable = available && enrolled && enabled;
        BiometricDisplayName = _biometricService.BiometricDisplayName;

        if (IsBiometricAvailable)
        {
            await AttemptBiometricUnlockAsync();
        }
    }

    [RelayCommand]
    private async Task AttemptBiometricUnlockAsync()
    {
        if (_biometricService == null || IsBiometricAuthenticating || IsUnlocking) return;

        IsBiometricAuthenticating = true;
        ErrorMessage = null;

        try
        {
            var password = await _biometricService.AuthenticateAsync("Unlock sensitive data");
            if (password != null)
            {
                var success = await Task.Run(() => _sensitiveLockService.Unlock(password));
                if (!success)
                {
                    ErrorMessage = "Biometric unlock failed. Please enter your password.";
                    _log.Warning("Biometric-retrieved password was incorrect for sensitive unlock");
                }
            }
        }
        catch (Exception ex)
        {
            _log.Warning(ex, "Biometric authentication error during sensitive unlock");
        }
        finally
        {
            IsBiometricAuthenticating = false;
        }
    }

    /// <summary>
    /// Attempts to unlock sensitive data with the entered password.
    /// </summary>
    [RelayCommand]
    private async Task UnlockAsync()
    {
        if (string.IsNullOrEmpty(Password))
        {
            ErrorMessage = "Please enter your password";
            return;
        }

        IsUnlocking = true;
        ErrorMessage = null;

        try
        {
            // Run on background thread to avoid UI freeze
            var success = await Task.Run(() => _sensitiveLockService.Unlock(Password));

            if (!success)
            {
                ErrorMessage = "Incorrect password";
            }
            // If successful, the view will automatically hide due to binding to IsSensitiveUnlocked
        }
        catch (Exception)
        {
            ErrorMessage = "An error occurred. Please try again.";
        }
        finally
        {
            // Clear password from memory for security
            Password = string.Empty;
            IsUnlocking = false;
        }
    }

    /// <summary>
    /// Resets the view model state. Call when the overlay is shown.
    /// </summary>
    public void Reset()
    {
        Password = string.Empty;
        ErrorMessage = null;
        IsUnlocking = false;
        _ = InitializeBiometricAsync();
    }
}
