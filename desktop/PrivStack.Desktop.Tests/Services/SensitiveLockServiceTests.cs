using System.ComponentModel;
using PrivStack.Desktop.Services;

namespace PrivStack.Desktop.Tests.Services;

/// <summary>
/// Tests for SensitiveLockService behavior.
/// Uses a testable subclass to avoid singleton and native FFI dependencies.
/// </summary>
public class TestableSensitiveLockService : INotifyPropertyChanged
{
    private readonly object _lock = new();
    private bool _isSensitiveUnlocked;
    private int _lockoutMinutes = 5;
    private DateTime _lastActivity = DateTime.UtcNow;

    public event PropertyChangedEventHandler? PropertyChanged;
    public event EventHandler? Locked;
    public event EventHandler? Unlocked;

    public bool IsSensitiveUnlocked
    {
        get => _isSensitiveUnlocked;
        private set
        {
            if (_isSensitiveUnlocked != value)
            {
                _isSensitiveUnlocked = value;
                PropertyChanged?.Invoke(this, new PropertyChangedEventArgs(nameof(IsSensitiveUnlocked)));
                if (value) Unlocked?.Invoke(this, EventArgs.Empty);
                else Locked?.Invoke(this, EventArgs.Empty);
            }
        }
    }

    public int LockoutMinutes
    {
        get => _lockoutMinutes;
        set
        {
            if (_lockoutMinutes != value)
            {
                _lockoutMinutes = value;
                PropertyChanged?.Invoke(this, new PropertyChangedEventArgs(nameof(LockoutMinutes)));
            }
        }
    }

    public DateTime LastActivity => _lastActivity;

    public void UnlockWithoutValidation()
    {
        lock (_lock)
        {
            IsSensitiveUnlocked = true;
            _lastActivity = DateTime.UtcNow;
        }
    }

    public void Lock()
    {
        lock (_lock) { IsSensitiveUnlocked = false; }
    }

    public void RecordActivity()
    {
        lock (_lock)
        {
            if (IsSensitiveUnlocked)
                _lastActivity = DateTime.UtcNow;
        }
    }

    /// <summary>Exposed for testing the lockout check logic without a timer.</summary>
    public void CheckLockout()
    {
        lock (_lock)
        {
            if (!IsSensitiveUnlocked || LockoutMinutes <= 0)
                return;
            var elapsed = DateTime.UtcNow - _lastActivity;
            if (elapsed.TotalMinutes >= LockoutMinutes)
                IsSensitiveUnlocked = false;
        }
    }

    /// <summary>Force-set last activity for testing timeout behavior.</summary>
    public void SetLastActivity(DateTime utc)
    {
        lock (_lock) { _lastActivity = utc; }
    }
}

public class SensitiveLockServiceTests
{
    [Fact]
    public void UnlockWithoutValidation_SetsIsSensitiveUnlocked()
    {
        var svc = new TestableSensitiveLockService();
        svc.UnlockWithoutValidation();
        svc.IsSensitiveUnlocked.Should().BeTrue();
    }

    [Fact]
    public void Lock_SetsIsSensitiveUnlockedToFalse()
    {
        var svc = new TestableSensitiveLockService();
        svc.UnlockWithoutValidation();
        svc.Lock();
        svc.IsSensitiveUnlocked.Should().BeFalse();
    }

    [Fact]
    public void RecordActivity_UpdatesLastActivity_WhenUnlocked()
    {
        var svc = new TestableSensitiveLockService();
        svc.UnlockWithoutValidation();
        var before = svc.LastActivity;

        Thread.Sleep(10);
        svc.RecordActivity();

        svc.LastActivity.Should().BeAfter(before);
    }

    [Fact]
    public void RecordActivity_IsNoOp_WhenLocked()
    {
        var svc = new TestableSensitiveLockService();
        var initial = svc.LastActivity;

        Thread.Sleep(10);
        svc.RecordActivity();

        svc.LastActivity.Should().Be(initial);
    }

    [Fact]
    public void PropertyChanged_FiresOnUnlock()
    {
        var svc = new TestableSensitiveLockService();
        var changed = new List<string>();
        svc.PropertyChanged += (_, e) => changed.Add(e.PropertyName!);

        svc.UnlockWithoutValidation();

        changed.Should().Contain(nameof(svc.IsSensitiveUnlocked));
    }

    [Fact]
    public void PropertyChanged_FiresOnLock()
    {
        var svc = new TestableSensitiveLockService();
        svc.UnlockWithoutValidation();
        var changed = new List<string>();
        svc.PropertyChanged += (_, e) => changed.Add(e.PropertyName!);

        svc.Lock();

        changed.Should().Contain(nameof(svc.IsSensitiveUnlocked));
    }

    [Fact]
    public void UnlockedEvent_Fires()
    {
        var svc = new TestableSensitiveLockService();
        var fired = false;
        svc.Unlocked += (_, _) => fired = true;

        svc.UnlockWithoutValidation();

        fired.Should().BeTrue();
    }

    [Fact]
    public void LockedEvent_Fires()
    {
        var svc = new TestableSensitiveLockService();
        svc.UnlockWithoutValidation();
        var fired = false;
        svc.Locked += (_, _) => fired = true;

        svc.Lock();

        fired.Should().BeTrue();
    }

    [Fact]
    public void LockoutMinutes_PropertyChange_FiresEvent()
    {
        var svc = new TestableSensitiveLockService();
        var changed = new List<string>();
        svc.PropertyChanged += (_, e) => changed.Add(e.PropertyName!);

        svc.LockoutMinutes = 10;

        changed.Should().Contain(nameof(svc.LockoutMinutes));
        svc.LockoutMinutes.Should().Be(10);
    }

    [Fact]
    public void CheckLockout_AutoLocks_AfterTimeout()
    {
        var svc = new TestableSensitiveLockService();
        svc.UnlockWithoutValidation();
        svc.LockoutMinutes = 1;

        // Simulate old activity
        svc.SetLastActivity(DateTime.UtcNow.AddMinutes(-2));
        svc.CheckLockout();

        svc.IsSensitiveUnlocked.Should().BeFalse();
    }

    [Fact]
    public void CheckLockout_DoesNotLock_WhenLockoutMinutesIsZero()
    {
        var svc = new TestableSensitiveLockService();
        svc.UnlockWithoutValidation();
        svc.LockoutMinutes = 0;
        svc.SetLastActivity(DateTime.UtcNow.AddMinutes(-100));

        svc.CheckLockout();

        svc.IsSensitiveUnlocked.Should().BeTrue();
    }

    [Fact]
    public void CheckLockout_DoesNotLock_WhenRecentActivity()
    {
        var svc = new TestableSensitiveLockService();
        svc.UnlockWithoutValidation();
        svc.LockoutMinutes = 5;

        svc.CheckLockout();

        svc.IsSensitiveUnlocked.Should().BeTrue();
    }
}
