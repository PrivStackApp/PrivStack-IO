using PrivStack.Desktop.Services;
using PrivStack.Desktop.ViewModels;
using CommandDefinition = PrivStack.Desktop.Services.CommandDefinition;
using ICommandProvider = PrivStack.Desktop.Services.ICommandProvider;

namespace PrivStack.Desktop.Tests.ViewModels;

public class TestCommandProvider : ICommandProvider
{
    private readonly List<CommandDefinition> _commands;
    public int Priority { get; }

    public TestCommandProvider(List<CommandDefinition> commands, int priority = 100)
    {
        _commands = commands;
        Priority = priority;
    }

    public IEnumerable<CommandDefinition> GetCommands() => _commands;
}

public class CommandPaletteViewModelTests
{
    // CommandPaletteViewModel requires MainWindowViewModel which has heavy dependencies.
    // We test the CommandItem and command provider logic directly.

    [Fact]
    public void CommandItem_StoresProperties()
    {
        var executed = false;
        var item = new CommandItem("Test", "A test command", "test keyword", "General", () => executed = true);

        item.Name.Should().Be("Test");
        item.Description.Should().Be("A test command");
        item.Keywords.Should().Be("test keyword");
        item.Category.Should().Be("General");

        item.Action!.Invoke();
        executed.Should().BeTrue();
    }

    [Fact]
    public void CommandItem_LegacyConstructor_DefaultsCategory()
    {
        var item = new CommandItem("Test", "Desc", "kw", null);

        item.Category.Should().Be("General");
    }

    [Fact]
    public void TestCommandProvider_ReturnsCommands()
    {
        var commands = new List<CommandDefinition>
        {
            new() { Name = "Do Thing", Description = "Does a thing", Execute = () => { } },
            new() { Name = "Another", Description = "Another thing", Execute = () => { } }
        };
        var provider = new TestCommandProvider(commands);

        provider.GetCommands().Should().HaveCount(2);
    }

    [Fact]
    public void CommandDefinition_DefaultValues()
    {
        var def = new CommandDefinition
        {
            Name = "Test",
            Description = "Test desc",
            Execute = () => { }
        };

        def.Keywords.Should().BeEmpty();
        def.Category.Should().Be("General");
        def.Icon.Should().BeNull();
    }

    [Fact]
    public void TestCommandProvider_Priority_IsRespected()
    {
        var p1 = new TestCommandProvider([], priority: 50);
        var p2 = new TestCommandProvider([], priority: 200);

        p1.Priority.Should().BeLessThan(p2.Priority);
    }

    [Fact]
    public void CommandItem_NullAction_IsAllowed()
    {
        var item = new CommandItem("Test", "Desc", "kw", "Cat", null);
        item.Action.Should().BeNull();
    }

    [Fact]
    public void MultipleProviders_CanBeSortedByPriority()
    {
        var providers = new List<TestCommandProvider>
        {
            new([], priority: 300),
            new([], priority: 50),
            new([], priority: 100)
        };

        providers.Sort((a, b) => a.Priority.CompareTo(b.Priority));

        providers[0].Priority.Should().Be(50);
        providers[1].Priority.Should().Be(100);
        providers[2].Priority.Should().Be(300);
    }
}
