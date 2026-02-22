using System;
using System.Reactive.Disposables;

namespace PrivStack.Sdk;

public static class DisposableExtensions
{
    public static T DisposeWith<T>(this T disposable, CompositeDisposable compositeDisposable)
        where T : IDisposable
    {
        compositeDisposable.Add(disposable);
        return disposable;
    }
}
