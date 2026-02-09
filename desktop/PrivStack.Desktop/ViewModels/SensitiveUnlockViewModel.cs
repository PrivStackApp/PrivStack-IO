using CommunityToolkit.Mvvm.ComponentModel;
using CommunityToolkit.Mvvm.Input;
using PrivStack.Desktop.Services;
using PrivStack.Desktop.Services.Abstractions;

namespace PrivStack.Desktop.ViewModels;

/// <summary>
/// ViewModel for the sensitive data unlock overlay.
/// Handles password entry and validation for re-authentication.
/// </summary>
public partial class SensitiveUnlockViewModel : ObservableObject
{
    private readonly ISensitiveLockService _sensitiveLockService;

    public SensitiveUnlockViewModel(ISensitiveLockService sensitiveLockService)
    {
        _sensitiveLockService = sensitiveLockService;
    }

    [ObservableProperty]
    private string _password = string.Empty;

    [ObservableProperty]
    private string? _errorMessage;

    [ObservableProperty]
    private bool _isUnlocking;

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
    }
}
