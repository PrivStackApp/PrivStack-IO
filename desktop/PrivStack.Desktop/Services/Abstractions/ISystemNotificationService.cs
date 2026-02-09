namespace PrivStack.Desktop.Services.Abstractions;

/// <summary>
/// Sends native OS notifications (macOS Notification Center, Windows Toast, Linux notify-send).
/// </summary>
public interface ISystemNotificationService
{
    /// <summary>
    /// Sends a system notification. Returns true if the notification was dispatched successfully.
    /// </summary>
    Task<bool> SendNotificationAsync(string title, string body, string? subtitle = null);
}
