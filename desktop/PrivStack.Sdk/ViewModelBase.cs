using System;
using System.Reactive.Disposables;
using CommunityToolkit.Mvvm.ComponentModel;

namespace PrivStack.Sdk;

public abstract class ViewModelBase : ObservableObject, IDisposable
{
    protected CompositeDisposable Disposables { get; } = new();

    private bool _disposed;

    protected virtual void Dispose(bool disposing)
    {
        if (_disposed) return;
        if (disposing)
            Disposables.Dispose();
        _disposed = true;
    }

    public void Dispose()
    {
        Dispose(true);
        GC.SuppressFinalize(this);
    }
}
