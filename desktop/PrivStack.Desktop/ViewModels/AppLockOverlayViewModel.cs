using CommunityToolkit.Mvvm.ComponentModel;
using CommunityToolkit.Mvvm.Input;
using PrivStack.Desktop.Native;
using PrivStack.Desktop.Services;
using PrivStack.Desktop.Services.Abstractions;
using PrivStack.Desktop.Services.Biometric;
using Serilog;

namespace PrivStack.Desktop.ViewModels;

/// <summary>
/// ViewModel for the in-app lock overlay (frosted glass).
/// Used for biometric validation after enrollment — locks the UI in-place
/// without closing the main window.
/// </summary>
public partial class AppLockOverlayViewModel : ObservableObject
{
    private static readonly ILogger _log = Serilog.Log.ForContext<AppLockOverlayViewModel>();

    private readonly IAuthService _authService;
    private readonly IBiometricService _biometricService;
    private readonly IAppSettingsService _appSettings;
    private readonly IMasterPasswordCache? _passwordCache;

    public AppLockOverlayViewModel(
        IAuthService authService,
        IBiometricService biometricService,
        IAppSettingsService appSettings,
        IMasterPasswordCache? passwordCache = null)
    {
        _authService = authService;
        _biometricService = biometricService;
        _appSettings = appSettings;
        _passwordCache = passwordCache;
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
    /// Raised when unlock succeeds (biometric validated or password entered).
    /// The bool indicates whether biometric was used (true) or password (false).
    /// </summary>
    public event Action<bool>? Unlocked;

    /// <summary>
    /// Initializes biometric state and auto-attempts biometric unlock.
    /// </summary>
    public async Task InitializeAsync()
    {
        ErrorMessage = null;
        Password = string.Empty;

        var available = await _biometricService.IsAvailableAsync();
        IsBiometricAvailable = available && _biometricService.IsEnrolled;
        BiometricDisplayName = _biometricService.BiometricDisplayName;

        if (IsBiometricAvailable)
        {
            await AttemptBiometricUnlockAsync();
        }
    }

    [RelayCommand]
    private async Task AttemptBiometricUnlockAsync()
    {
        if (IsBiometricAuthenticating || IsUnlocking) return;

        IsBiometricAuthenticating = true;
        ErrorMessage = null;

        try
        {
            var password = await _biometricService.AuthenticateAsync("Verify biometric unlock");
            if (password != null)
            {
                // Validate the password is correct
                if (_authService.ValidateMasterPassword(password))
                {
                    Unlocked?.Invoke(true);
                    return;
                }
                ErrorMessage = "Biometric unlock failed. Please enter your password.";
                _log.Warning("Biometric-retrieved password was incorrect during validation");
            }
            else
            {
                ErrorMessage = $"{BiometricDisplayName} cancelled. Enter your password to continue.";
            }
        }
        catch (Exception ex)
        {
            _log.Warning(ex, "Biometric authentication error during validation");
            ErrorMessage = "Biometric error. Please enter your password.";
        }
        finally
        {
            IsBiometricAuthenticating = false;
        }
    }

    [RelayCommand]
    private void UnlockWithPassword()
    {
        if (string.IsNullOrEmpty(Password))
        {
            ErrorMessage = "Please enter your password.";
            return;
        }

        IsUnlocking = true;
        ErrorMessage = null;

        try
        {
            if (_authService.ValidateMasterPassword(Password))
            {
                Unlocked?.Invoke(false);
            }
            else
            {
                ErrorMessage = "Incorrect password.";
            }
        }
        finally
        {
            Password = string.Empty;
            IsUnlocking = false;
        }
    }
}
