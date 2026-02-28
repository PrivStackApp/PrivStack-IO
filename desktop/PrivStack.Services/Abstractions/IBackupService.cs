namespace PrivStack.Services.Abstractions;

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

public class BackupCompletedEventArgs : EventArgs
{
    public bool Success { get; }
    public string? BackupPath { get; }
    public string? ErrorMessage { get; }

    public BackupCompletedEventArgs(bool success, string? backupPath, string? errorMessage)
    {
        Success = success;
        BackupPath = backupPath;
        ErrorMessage = errorMessage;
    }
}

public record BackupInfo(string Path, DateTime CreatedAt, long SizeBytes)
{
    public string FormattedSize => SizeBytes switch
    {
        < 1024 => $"{SizeBytes} B",
        < 1024 * 1024 => $"{SizeBytes / 1024.0:F1} KB",
        < 1024 * 1024 * 1024 => $"{SizeBytes / (1024.0 * 1024.0):F1} MB",
        _ => $"{SizeBytes / (1024.0 * 1024.0 * 1024.0):F1} GB"
    };
}

public enum BackupFrequency
{
    Manual,
    Hourly,
    Daily,
    Weekly
}
