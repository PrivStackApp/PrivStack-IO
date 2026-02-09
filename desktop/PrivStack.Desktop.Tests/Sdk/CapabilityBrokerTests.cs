using PrivStack.Desktop.Sdk;
using PrivStack.Sdk;

namespace PrivStack.Desktop.Tests.Sdk;

public interface ITestCapability
{
    string Id { get; }
    Task<IReadOnlyList<string>> GetItemsAsync();
}

public class CapabilityBrokerTests
{
    private readonly CapabilityBroker _broker = new();

    [Fact]
    public void Register_And_GetProviders_ReturnsSingleProvider()
    {
        var provider = Substitute.For<ITestCapability>();
        _broker.Register(provider);

        var providers = _broker.GetProviders<ITestCapability>();

        providers.Should().ContainSingle().Which.Should().BeSameAs(provider);
    }

    [Fact]
    public void Register_MultipleProviders_ReturnsAll()
    {
        var p1 = Substitute.For<ITestCapability>();
        var p2 = Substitute.For<ITestCapability>();
        _broker.Register(p1);
        _broker.Register(p2);

        var providers = _broker.GetProviders<ITestCapability>();

        providers.Should().HaveCount(2);
    }

    [Fact]
    public void Register_DuplicateProvider_IsIdempotent()
    {
        var provider = Substitute.For<ITestCapability>();
        _broker.Register(provider);
        _broker.Register(provider);

        var providers = _broker.GetProviders<ITestCapability>();

        providers.Should().ContainSingle();
    }

    [Fact]
    public void GetProviders_UnregisteredType_ReturnsEmpty()
    {
        var providers = _broker.GetProviders<ITestCapability>();
        providers.Should().BeEmpty();
    }

    [Fact]
    public void GetProvider_WithSelector_FindsCorrectProvider()
    {
        var p1 = Substitute.For<ITestCapability>();
        p1.Id.Returns("alpha");
        var p2 = Substitute.For<ITestCapability>();
        p2.Id.Returns("beta");

        _broker.Register(p1);
        _broker.Register(p2);

        var found = _broker.GetProvider<ITestCapability>("beta", p => p.Id);

        found.Should().BeSameAs(p2);
    }

    [Fact]
    public void GetProvider_WithSelector_ReturnsNull_WhenNotFound()
    {
        var p1 = Substitute.For<ITestCapability>();
        p1.Id.Returns("alpha");
        _broker.Register(p1);

        var found = _broker.GetProvider<ITestCapability>("missing", p => p.Id);

        found.Should().BeNull();
    }

    [Fact]
    public async Task QueryAllAsync_AggregatesResults()
    {
        var p1 = Substitute.For<ITestCapability>();
        p1.GetItemsAsync().Returns(new List<string> { "a", "b" }.AsReadOnly());
        var p2 = Substitute.For<ITestCapability>();
        p2.GetItemsAsync().Returns(new List<string> { "c" }.AsReadOnly());

        _broker.Register(p1);
        _broker.Register(p2);

        var results = await _broker.QueryAllAsync<ITestCapability, string>(
            p => p.GetItemsAsync());

        results.Should().BeEquivalentTo(["a", "b", "c"]);
    }

    [Fact]
    public async Task QueryAllAsync_EmptyProviders_ReturnsEmpty()
    {
        var results = await _broker.QueryAllAsync<ITestCapability, string>(
            p => p.GetItemsAsync());

        results.Should().BeEmpty();
    }

    [Fact]
    public void ThreadSafety_ConcurrentRegistrations_DoNotThrow()
    {
        var tasks = Enumerable.Range(0, 100).Select(i =>
        {
            return Task.Run(() =>
            {
                var provider = Substitute.For<ITestCapability>();
                provider.Id.Returns($"provider-{i}");
                _broker.Register(provider);
            });
        });

        var act = () => Task.WhenAll(tasks);
        act.Should().NotThrowAsync();
    }
}
