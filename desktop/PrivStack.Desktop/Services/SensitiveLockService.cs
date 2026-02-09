using System.ComponentModel;
using System.Runtime.CompilerServices;
using PrivStack.Desktop.Native;
using PrivStack.Desktop.Services.Abstractions;
using Serilog;

namespace PrivStack.Desktop.Services;

/// <summary>
/// Manages the lock state for sensitive features (Passwords, Vault).
/// Implements a rolling lockout timer that resets on activity.
/// </summary>
public sealed class SensitiveLockService : ISensitiveLockService
{
    private readonly IAuthService _nativeService;
    private readonly object _lock = new();
    private bool _isSensitiveUnlocked;
    private int _lockoutMinutes = 5; // Default 5 minutes
    private DateTime _lastActivity = DateTime.UtcNow;
    private Timer? _lockoutTimer;

    public event PropertyChangedEventHandler? PropertyChanged;
    public event EventHandler? Locked;
    public event EventHandler? Unlocked;

    public SensitiveLockService(IAuthService nativeService)
    {
        _nativeService = nativeService;
        // Start the lockout timer (checks every 10 seconds)
        _lockoutTimer = new Timer(CheckLockout, null, TimeSpan.FromSeconds(10), TimeSpan.FromSeconds(10));
    }

    /// <summary>
    /// Whether sensitive features are currently unlocked.
    /// </summary>
    public bool IsSensitiveUnlocked
    {
        get => _isSensitiveUnlocked;
        private set
        {
            if (_isSensitiveUnlocked != value)
            {
                _isSensitiveUnlocked = value;
                OnPropertyChanged();

                if (value)
                {
                    Log.Debug("Sensitive data unlocked");
                    Unlocked?.Invoke(this, EventArgs.Empty);
                }
                else
                {
                    Log.Debug("Sensitive data locked");
                    Locked?.Invoke(this, EventArgs.Empty);
                }
            }
        }
    }

    /// <summary>
    /// The lockout duration in minutes. 0 means never lock.
    /// </summary>
    public int LockoutMinutes
    {
        get => _lockoutMinutes;
        set
        {
            if (_lockoutMinutes != value)
            {
                _lockoutMinutes = value;
                OnPropertyChanged();
                Log.Debug("Sensitive lockout minutes set to {Minutes}", value);
            }
        }
    }

    /// <summary>
    /// Unlocks sensitive features by validating the master password.
    /// </summary>
    /// <param name="masterPassword">The master password to validate.</param>
    /// <returns>True if the password is correct and unlock succeeded.</returns>
    public bool Unlock(string masterPassword)
    {
        lock (_lock)
        {
            try
            {
                // Validate password using PrivStackService
                var isValid = _nativeService.ValidateMasterPassword(masterPassword);

                if (isValid)
                {
                    IsSensitiveUnlocked = true;
                    _lastActivity = DateTime.UtcNow;
                    return true;
                }

                Log.Warning("Sensitive unlock failed: incorrect password");
                return false;
            }
            catch (Exception ex)
            {
                Log.Error(ex, "Error during sensitive unlock");
                return false;
            }
        }
    }

    /// <summary>
    /// Unlocks sensitive features without password validation.
    /// Used when the app is first unlocked with the master password.
    /// </summary>
    public void UnlockWithoutValidation()
    {
        lock (_lock)
        {
            IsSensitiveUnlocked = true;
            _lastActivity = DateTime.UtcNow;
            Log.Debug("Sensitive data unlocked without validation (initial app unlock)");
        }
    }

    /// <summary>
    /// Locks sensitive features immediately.
    /// </summary>
    public void Lock()
    {
        lock (_lock)
        {
            IsSensitiveUnlocked = false;
        }
    }

    /// <summary>
    /// Records user activity, resetting the rolling lockout timer.
    /// Should be called on sensitive operations like viewing, copying, or editing.
    /// </summary>
    public void RecordActivity()
    {
        lock (_lock)
        {
            if (IsSensitiveUnlocked)
            {
                _lastActivity = DateTime.UtcNow;
            }
        }
    }

    /// <summary>
    /// Timer callback that checks if the lockout period has been exceeded.
    /// </summary>
    private void CheckLockout(object? state)
    {
        lock (_lock)
        {
            // Don't lock if:
            // - Already locked
            // - LockoutMinutes is 0 (never lock)
            // - App-level auth is not unlocked
            if (!IsSensitiveUnlocked || LockoutMinutes <= 0)
            {
                return;
            }

            // Check if app is still unlocked at the system level
            if (!_nativeService.IsAuthUnlocked())
            {
                // App was locked, also lock sensitive features
                IsSensitiveUnlocked = false;
                return;
            }

            var elapsed = DateTime.UtcNow - _lastActivity;
            if (elapsed.TotalMinutes >= LockoutMinutes)
            {
                Log.Information("Sensitive data auto-locked after {Minutes} minutes of inactivity", LockoutMinutes);
                IsSensitiveUnlocked = false;
            }
        }
    }

    /// <summary>
    /// Stops the lockout timer. Call when disposing.
    /// </summary>
    public void StopTimer()
    {
        _lockoutTimer?.Dispose();
        _lockoutTimer = null;
    }

    private void OnPropertyChanged([CallerMemberName] string? propertyName = null)
    {
        PropertyChanged?.Invoke(this, new PropertyChangedEventArgs(propertyName));
    }
}
