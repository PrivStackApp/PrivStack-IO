using PrivStack.Desktop.ViewModels;

namespace PrivStack.Desktop.Services.Abstractions;

/// <summary>
/// Abstraction over backup creation and management.
/// </summary>
public interface IBackupService
{
    string DataDirectory { get; }
    string BackupDirectory { get; }
    event EventHandler<BackupCompletedEventArgs>? BackupCompleted;
    Task<string?> BackupNowAsync();
    IEnumerable<BackupInfo> GetExistingBackups();
    Task<bool> RestoreBackupAsync(string backupPath);
    void StartScheduledBackups();
    void StopScheduledBackups();
    void UpdateBackupDirectory(string newPath);
    void UpdateBackupFrequency(BackupFrequency frequency);
}
