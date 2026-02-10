using System.ComponentModel;

namespace PrivStack.Desktop.Tests.Helpers;

/// <summary>
/// Base class for ViewModel unit tests providing common utilities.
/// </summary>
public abstract class ViewModelTestBase
{
    /// <summary>
    /// Waits for a property change notification on an observable object.
    /// </summary>
    /// <param name="obj">The object to observe</param>
    /// <param name="propertyName">The property name to wait for</param>
    /// <param name="timeout">Maximum time to wait (default 5 seconds)</param>
    /// <returns>Task that completes when the property changes or times out</returns>
    protected async Task WaitForPropertyAsync(
        INotifyPropertyChanged obj,
        string propertyName,
        TimeSpan? timeout = null)
    {
        var tcs = new TaskCompletionSource();
        timeout ??= TimeSpan.FromSeconds(5);

        void Handler(object? sender, PropertyChangedEventArgs e)
        {
            if (e.PropertyName == propertyName)
                tcs.TrySetResult();
        }

        obj.PropertyChanged += Handler;
        try
        {
            var completed = await Task.WhenAny(tcs.Task, Task.Delay(timeout.Value));
            if (completed != tcs.Task)
            {
                throw new TimeoutException($"Timed out waiting for property '{propertyName}' to change");
            }
        }
        finally
        {
            obj.PropertyChanged -= Handler;
        }
    }

    /// <summary>
    /// Waits for a collection to reach a specific count.
    /// </summary>
    /// <typeparam name="T">Element type</typeparam>
    /// <param name="collection">The collection to observe</param>
    /// <param name="expectedCount">Expected number of elements</param>
    /// <param name="timeout">Maximum time to wait (default 5 seconds)</param>
    protected async Task WaitForCollectionCountAsync<T>(
        ICollection<T> collection,
        int expectedCount,
        TimeSpan? timeout = null)
    {
        timeout ??= TimeSpan.FromSeconds(5);
        var deadline = DateTime.UtcNow + timeout.Value;

        while (collection.Count != expectedCount && DateTime.UtcNow < deadline)
        {
            await Task.Delay(10);
        }

        if (collection.Count != expectedCount)
        {
            throw new TimeoutException(
                $"Collection count {collection.Count} did not reach expected {expectedCount}");
        }
    }

    /// <summary>
    /// Executes an action and waits for a property change.
    /// </summary>
    protected async Task ExecuteAndWaitForPropertyAsync(
        INotifyPropertyChanged obj,
        string propertyName,
        Action action,
        TimeSpan? timeout = null)
    {
        var tcs = new TaskCompletionSource();
        timeout ??= TimeSpan.FromSeconds(5);

        void Handler(object? sender, PropertyChangedEventArgs e)
        {
            if (e.PropertyName == propertyName)
                tcs.TrySetResult();
        }

        obj.PropertyChanged += Handler;
        try
        {
            action();
            var completed = await Task.WhenAny(tcs.Task, Task.Delay(timeout.Value));
            if (completed != tcs.Task)
            {
                throw new TimeoutException($"Timed out waiting for property '{propertyName}' to change after action");
            }
        }
        finally
        {
            obj.PropertyChanged -= Handler;
        }
    }
}
