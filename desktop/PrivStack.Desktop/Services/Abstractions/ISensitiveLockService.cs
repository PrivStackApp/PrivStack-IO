using System.ComponentModel;

namespace PrivStack.Desktop.Services.Abstractions;

/// <summary>
/// Abstraction over sensitive-data lock/unlock lifecycle.
/// </summary>
public interface ISensitiveLockService : INotifyPropertyChanged
{
    bool IsSensitiveUnlocked { get; }
    int LockoutMinutes { get; set; }
    event EventHandler? Locked;
    event EventHandler? Unlocked;
    bool Unlock(string masterPassword);
    void UnlockWithoutValidation();
    void Lock();
    void RecordActivity();
}
