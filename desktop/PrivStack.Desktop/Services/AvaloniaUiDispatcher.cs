using Avalonia.Threading;
using PrivStack.Desktop.Services.Abstractions;

namespace PrivStack.Desktop.Services;

/// <summary>
/// Production implementation that delegates to Avalonia's UI thread dispatcher.
/// </summary>
public sealed class AvaloniaUiDispatcher : IUiDispatcher
{
    public void Post(Action action) => Dispatcher.UIThread.Post(action);

    public async Task InvokeAsync(Action action) => await Dispatcher.UIThread.InvokeAsync(action);

    public async Task InvokeAsync(Func<Task> action) => await Dispatcher.UIThread.InvokeAsync(action);
}
