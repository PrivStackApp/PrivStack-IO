namespace PrivStack.Services.Abstractions;

/// <summary>
/// Abstraction over system dialogs (confirmation, file picker).
/// Platform-agnostic — no Avalonia/Window dependencies.
/// </summary>
public interface IDialogService
{
    Task<bool> ShowConfirmationAsync(string title, string message, string confirmButtonText = "Confirm");
    Task<string?> ShowPasswordConfirmationAsync(string title, string message, string confirmButtonText = "Confirm");
    Task<string?> ShowOpenFileDialogAsync(string title, (string Name, string Extension)[] filters);
    Task<string?> ShowSaveFileDialogAsync(string title, string defaultFileName, (string Name, string Extension)[] filters);
    Task<string?> ShowOpenFolderDialogAsync(string title);
}
