namespace PrivStack.Desktop.Services.Abstractions;

/// <summary>
/// Abstracts UI thread dispatching for testability.
/// Production implementation wraps Avalonia's Dispatcher.UIThread.
/// Test implementation executes actions synchronously.
/// </summary>
public interface IUiDispatcher
{
    void Post(Action action);
    Task InvokeAsync(Action action);
    Task InvokeAsync(Func<Task> action);
}
