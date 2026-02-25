namespace PrivStack.Desktop.Tests.Sdk;

using System.Reactive.Disposables;
using PrivStack.Sdk;

public class ViewModelBaseTests
{
    private sealed class TestViewModel : ViewModelBase
    {
        public CompositeDisposable ExposedDisposables => Disposables;
        public bool DisposeCalled { get; private set; }

        protected override void Dispose(bool disposing)
        {
            DisposeCalled = true;
            base.Dispose(disposing);
        }
    }

    [Fact]
    public void Disposables_is_not_null()
    {
        using var vm = new TestViewModel();
        vm.ExposedDisposables.Should().NotBeNull();
    }

    [Fact]
    public void Dispose_disposes_composite()
    {
        var vm = new TestViewModel();
        var disposed = false;
        Disposable.Create(() => disposed = true).DisposeWith(vm.ExposedDisposables);

        vm.Dispose();

        disposed.Should().BeTrue();
    }

    [Fact]
    public void Dispose_calls_dispose_bool()
    {
        var vm = new TestViewModel();
        vm.Dispose();
        vm.DisposeCalled.Should().BeTrue();
    }

    [Fact]
    public void Dispose_is_idempotent()
    {
        var vm = new TestViewModel();
        var disposeCount = 0;
        Disposable.Create(() => disposeCount++).DisposeWith(vm.ExposedDisposables);

        vm.Dispose();
        vm.Dispose(); // second call should be no-op

        disposeCount.Should().Be(1);
    }

    [Fact]
    public void Implements_IDisposable()
    {
        using var vm = new TestViewModel();
        vm.Should().BeAssignableTo<IDisposable>();
    }

    [Fact]
    public void Inherits_ObservableObject()
    {
        using var vm = new TestViewModel();
        vm.Should().BeAssignableTo<CommunityToolkit.Mvvm.ComponentModel.ObservableObject>();
    }
}
