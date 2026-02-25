namespace PrivStack.Desktop.Tests.Sdk;

using PrivStack.Sdk;

public class PluginTypesTests
{
    // =========================================================================
    // PluginMetadata
    // =========================================================================

    [Fact]
    public void PluginMetadata_required_fields()
    {
        var meta = new PluginMetadata
        {
            Id = "privstack.notes",
            Name = "Notes",
            Description = "Block-based note editor",
            Version = new Version(1, 2, 3)
        };
        meta.Id.Should().Be("privstack.notes");
        meta.Version.Should().Be(new Version(1, 2, 3));
    }

    [Fact]
    public void PluginMetadata_defaults()
    {
        var meta = new PluginMetadata
        {
            Id = "test",
            Name = "Test",
            Description = "Test plugin",
            Version = new Version(1, 0, 0)
        };
        meta.Author.Should().Be("PrivStack");
        meta.NavigationOrder.Should().Be(1000);
        meta.Category.Should().Be(PluginCategory.Utility);
        meta.Tags.Should().BeEmpty();
        meta.CanDisable.Should().BeTrue();
        meta.IsExperimental.Should().BeFalse();
        meta.IsHardLocked.Should().BeFalse();
        meta.SupportsInfoPanel.Should().BeTrue();
        meta.ReleaseStage.Should().Be(ReleaseStage.Release);
        meta.DetailedDescription.Should().BeNull();
        meta.Icon.Should().BeNull();
        meta.MinAppVersion.Should().BeNull();
        meta.WebsiteUrl.Should().BeNull();
    }

    [Fact]
    public void PluginMetadata_ToString()
    {
        var meta = new PluginMetadata
        {
            Id = "privstack.tasks",
            Name = "Tasks",
            Description = "Task manager",
            Version = new Version(2, 1, 0)
        };
        meta.ToString().Should().Be("Tasks (privstack.tasks) v2.1.0");
    }

    [Fact]
    public void PluginMetadata_experimental_and_hard_locked()
    {
        var meta = new PluginMetadata
        {
            Id = "privstack.canvas",
            Name = "Canvas",
            Description = "Whiteboard",
            Version = new Version(0, 1, 0),
            IsExperimental = true,
            IsHardLocked = true,
            HardLockedReason = "Requires license upgrade",
            ReleaseStage = ReleaseStage.Alpha
        };
        meta.IsExperimental.Should().BeTrue();
        meta.IsHardLocked.Should().BeTrue();
        meta.HardLockedReason.Should().Contain("license");
        meta.ReleaseStage.Should().Be(ReleaseStage.Alpha);
    }

    // =========================================================================
    // NavigationItem
    // =========================================================================

    [Fact]
    public void NavigationItem_defaults()
    {
        var item = new NavigationItem
        {
            Id = "notes",
            DisplayName = "Notes"
        };
        item.IsSelected.Should().BeFalse();
        item.IsEnabled.Should().BeTrue();
        item.Order.Should().Be(1000);
        item.ShowBadge.Should().BeFalse();
        item.BadgeCount.Should().Be(0);
        item.IsExperimental.Should().BeFalse();
        item.IsAlpha.Should().BeFalse();
        item.IsBeta.Should().BeFalse();
        item.ReleaseStage.Should().Be(ReleaseStage.Release);
    }

    [Fact]
    public void NavigationItem_IsSelected_raises_property_changed()
    {
        var item = new NavigationItem { Id = "test", DisplayName = "Test" };
        var raised = false;
        item.PropertyChanged += (_, e) =>
        {
            if (e.PropertyName == nameof(NavigationItem.IsSelected))
                raised = true;
        };

        item.IsSelected = true;
        raised.Should().BeTrue();
        item.IsSelected.Should().BeTrue();
    }

    [Fact]
    public void NavigationItem_IsEnabled_raises_property_changed()
    {
        var item = new NavigationItem { Id = "test", DisplayName = "Test" };
        var raised = false;
        item.PropertyChanged += (_, e) =>
        {
            if (e.PropertyName == nameof(NavigationItem.IsEnabled))
                raised = true;
        };

        item.IsEnabled = false;
        raised.Should().BeTrue();
        item.IsEnabled.Should().BeFalse();
    }

    [Fact]
    public void NavigationItem_IsAlpha_and_IsBeta()
    {
        var alpha = new NavigationItem
        {
            Id = "a",
            DisplayName = "Alpha",
            ReleaseStage = ReleaseStage.Alpha
        };
        alpha.IsAlpha.Should().BeTrue();
        alpha.IsBeta.Should().BeFalse();

        var beta = new NavigationItem
        {
            Id = "b",
            DisplayName = "Beta",
            ReleaseStage = ReleaseStage.Beta
        };
        beta.IsAlpha.Should().BeFalse();
        beta.IsBeta.Should().BeTrue();
    }

    [Fact]
    public void NavigationItem_with_badge_and_shortcut()
    {
        var item = new NavigationItem
        {
            Id = "tasks",
            DisplayName = "Tasks",
            Icon = "CheckSquare",
            Order = 200,
            ShowBadge = true,
            BadgeCount = 5,
            ShortcutHint = "Cmd+2"
        };
        item.ShowBadge.Should().BeTrue();
        item.BadgeCount.Should().Be(5);
        item.ShortcutHint.Should().Be("Cmd+2");
        item.Icon.Should().Be("CheckSquare");
    }

    // =========================================================================
    // Enums
    // =========================================================================

    [Theory]
    [InlineData(PluginState.Discovered)]
    [InlineData(PluginState.Initializing)]
    [InlineData(PluginState.Initialized)]
    [InlineData(PluginState.Active)]
    [InlineData(PluginState.Deactivated)]
    [InlineData(PluginState.Failed)]
    [InlineData(PluginState.Disposed)]
    public void PluginState_all_values(PluginState state)
    {
        Enum.IsDefined(state).Should().BeTrue();
    }

    [Theory]
    [InlineData(PluginCategory.Productivity)]
    [InlineData(PluginCategory.Security)]
    [InlineData(PluginCategory.Communication)]
    [InlineData(PluginCategory.Information)]
    [InlineData(PluginCategory.Utility)]
    [InlineData(PluginCategory.Extension)]
    public void PluginCategory_all_values(PluginCategory category)
    {
        Enum.IsDefined(category).Should().BeTrue();
    }
}
