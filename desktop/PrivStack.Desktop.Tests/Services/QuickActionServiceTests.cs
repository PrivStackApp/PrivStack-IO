namespace PrivStack.Desktop.Tests.Services;

using PrivStack.Desktop.Services;
using PrivStack.Desktop.Services.Plugin;
using PrivStack.Sdk.Capabilities;

public class QuickActionServiceTests
{
    private readonly IPluginRegistry _registry;
    private readonly QuickActionService _service;

    public QuickActionServiceTests()
    {
        _registry = Substitute.For<IPluginRegistry>();
        _service = new QuickActionService(_registry);
    }

    [Fact]
    public void GetAllActions_returns_empty_when_no_providers()
    {
        _registry.GetCapabilityProviders<IQuickActionProvider>()
            .Returns(Array.Empty<IQuickActionProvider>());

        var actions = _service.GetAllActions();
        actions.Should().BeEmpty();
    }

    [Fact]
    public void GetAllActions_collects_from_multiple_providers()
    {
        var provider1 = Substitute.For<IQuickActionProvider>();
        provider1.GetQuickActions().Returns(new List<QuickActionDescriptor>
        {
            new() { ActionId = "task.new", DisplayName = "New Task", PluginId = "privstack.tasks", DefaultShortcutHint = "Cmd+T" }
        });

        var provider2 = Substitute.For<IQuickActionProvider>();
        provider2.GetQuickActions().Returns(new List<QuickActionDescriptor>
        {
            new() { ActionId = "note.new", DisplayName = "New Note", PluginId = "privstack.notes", DefaultShortcutHint = "Cmd+N" },
            new() { ActionId = "note.search", DisplayName = "Search Notes", PluginId = "privstack.notes" }
        });

        _registry.GetCapabilityProviders<IQuickActionProvider>()
            .Returns(new List<IQuickActionProvider> { provider1, provider2 });

        var actions = _service.GetAllActions();
        actions.Should().HaveCount(3);
    }

    [Fact]
    public void GetAllActions_caches_results()
    {
        var provider = Substitute.For<IQuickActionProvider>();
        provider.GetQuickActions().Returns(new List<QuickActionDescriptor>
        {
            new() { ActionId = "test.action", DisplayName = "Test", PluginId = "test" }
        });

        _registry.GetCapabilityProviders<IQuickActionProvider>()
            .Returns(new List<IQuickActionProvider> { provider });

        var actions1 = _service.GetAllActions();
        var actions2 = _service.GetAllActions();

        actions1.Should().BeSameAs(actions2);
        // Provider queried only once
        _registry.Received(1).GetCapabilityProviders<IQuickActionProvider>();
    }

    [Fact]
    public void Invalidate_clears_cache()
    {
        var provider = Substitute.For<IQuickActionProvider>();
        provider.GetQuickActions().Returns(new List<QuickActionDescriptor>
        {
            new() { ActionId = "test.action", DisplayName = "Test", PluginId = "test" }
        });

        _registry.GetCapabilityProviders<IQuickActionProvider>()
            .Returns(new List<IQuickActionProvider> { provider });

        _service.GetAllActions();
        _service.Invalidate();
        _service.GetAllActions();

        _registry.Received(2).GetCapabilityProviders<IQuickActionProvider>();
    }

    [Fact]
    public void FindAction_returns_matching_action()
    {
        var provider = Substitute.For<IQuickActionProvider>();
        provider.GetQuickActions().Returns(new List<QuickActionDescriptor>
        {
            new() { ActionId = "task.new", DisplayName = "New Task", PluginId = "privstack.tasks" },
            new() { ActionId = "task.edit", DisplayName = "Edit Task", PluginId = "privstack.tasks" }
        });

        _registry.GetCapabilityProviders<IQuickActionProvider>()
            .Returns(new List<IQuickActionProvider> { provider });

        var action = _service.FindAction("task.new");
        action.Should().NotBeNull();
        action!.Descriptor.DisplayName.Should().Be("New Task");
    }

    [Fact]
    public void FindAction_returns_null_for_unknown()
    {
        _registry.GetCapabilityProviders<IQuickActionProvider>()
            .Returns(Array.Empty<IQuickActionProvider>());

        _service.FindAction("nonexistent").Should().BeNull();
    }

    [Fact]
    public void FindAction_is_case_insensitive()
    {
        var provider = Substitute.For<IQuickActionProvider>();
        provider.GetQuickActions().Returns(new List<QuickActionDescriptor>
        {
            new() { ActionId = "Task.New", DisplayName = "New Task", PluginId = "privstack.tasks" }
        });

        _registry.GetCapabilityProviders<IQuickActionProvider>()
            .Returns(new List<IQuickActionProvider> { provider });

        _service.FindAction("task.new").Should().NotBeNull();
    }

    [Fact]
    public void FindActionByShortcut_returns_matching_action()
    {
        var provider = Substitute.For<IQuickActionProvider>();
        provider.GetQuickActions().Returns(new List<QuickActionDescriptor>
        {
            new() { ActionId = "task.new", DisplayName = "New Task", PluginId = "privstack.tasks", DefaultShortcutHint = "Cmd+T" },
            new() { ActionId = "note.new", DisplayName = "New Note", PluginId = "privstack.notes", DefaultShortcutHint = "Cmd+N" }
        });

        _registry.GetCapabilityProviders<IQuickActionProvider>()
            .Returns(new List<IQuickActionProvider> { provider });

        var action = _service.FindActionByShortcut("Cmd+T");
        action.Should().NotBeNull();
        action!.Descriptor.ActionId.Should().Be("task.new");
    }

    [Fact]
    public void FindActionByShortcut_returns_null_for_unregistered()
    {
        _registry.GetCapabilityProviders<IQuickActionProvider>()
            .Returns(Array.Empty<IQuickActionProvider>());

        _service.FindActionByShortcut("Cmd+Z").Should().BeNull();
    }

    [Fact]
    public void FindActionByShortcut_is_case_insensitive()
    {
        var provider = Substitute.For<IQuickActionProvider>();
        provider.GetQuickActions().Returns(new List<QuickActionDescriptor>
        {
            new() { ActionId = "task.new", DisplayName = "New Task", PluginId = "privstack.tasks", DefaultShortcutHint = "Cmd+T" }
        });

        _registry.GetCapabilityProviders<IQuickActionProvider>()
            .Returns(new List<IQuickActionProvider> { provider });

        _service.FindActionByShortcut("cmd+t").Should().NotBeNull();
    }

    [Fact]
    public void Priority_returns_expected_value()
    {
        _service.Priority.Should().Be(50);
    }

    [Fact]
    public void GetAllActions_handles_provider_exception_gracefully()
    {
        var goodProvider = Substitute.For<IQuickActionProvider>();
        goodProvider.GetQuickActions().Returns(new List<QuickActionDescriptor>
        {
            new() { ActionId = "good.action", DisplayName = "Good", PluginId = "test" }
        });

        var badProvider = Substitute.For<IQuickActionProvider>();
        badProvider.GetQuickActions().Returns(_ => throw new InvalidOperationException("Plugin crashed"));

        _registry.GetCapabilityProviders<IQuickActionProvider>()
            .Returns(new List<IQuickActionProvider> { badProvider, goodProvider });

        var actions = _service.GetAllActions();
        actions.Should().HaveCount(1);
        actions[0].Descriptor.ActionId.Should().Be("good.action");
    }
}
