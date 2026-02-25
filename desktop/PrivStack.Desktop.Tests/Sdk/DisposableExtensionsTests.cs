namespace PrivStack.Desktop.Tests.Sdk;

using System.Reactive.Disposables;
using PrivStack.Sdk;

public class DisposableExtensionsTests
{
    [Fact]
    public void DisposeWith_adds_to_composite()
    {
        var composite = new CompositeDisposable();
        var disposable = Disposable.Create(() => { });

        disposable.DisposeWith(composite);

        composite.Count.Should().Be(1);
    }

    [Fact]
    public void DisposeWith_returns_same_instance()
    {
        var composite = new CompositeDisposable();
        var disposable = Disposable.Create(() => { });

        var returned = disposable.DisposeWith(composite);

        returned.Should().BeSameAs(disposable);
    }

    [Fact]
    public void DisposeWith_disposes_when_composite_disposes()
    {
        var composite = new CompositeDisposable();
        var disposed = false;
        Disposable.Create(() => disposed = true).DisposeWith(composite);

        composite.Dispose();

        disposed.Should().BeTrue();
    }

    [Fact]
    public void DisposeWith_chains_multiple()
    {
        var composite = new CompositeDisposable();
        var count = 0;
        Disposable.Create(() => count++).DisposeWith(composite);
        Disposable.Create(() => count++).DisposeWith(composite);
        Disposable.Create(() => count++).DisposeWith(composite);

        composite.Dispose();

        count.Should().Be(3);
    }
}
