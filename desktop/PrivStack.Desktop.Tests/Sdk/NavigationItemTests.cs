using System.ComponentModel;
using PrivStack.Sdk;

namespace PrivStack.Desktop.Tests.Sdk;

public class NavigationItemTests
{
    [Fact]
    public void IsSelected_FiresPropertyChanged()
    {
        var item = new NavigationItem { Id = "test", DisplayName = "Test" };
        var changed = new List<string>();
        item.PropertyChanged += (_, e) => changed.Add(e.PropertyName!);

        item.IsSelected = true;

        changed.Should().Contain(nameof(NavigationItem.IsSelected));
    }

    [Fact]
    public void IsEnabled_FiresPropertyChanged()
    {
        var item = new NavigationItem { Id = "test", DisplayName = "Test" };
        var changed = new List<string>();
        item.PropertyChanged += (_, e) => changed.Add(e.PropertyName!);

        item.IsEnabled = false;

        changed.Should().Contain(nameof(NavigationItem.IsEnabled));
    }

    [Fact]
    public void IsEnabled_DefaultsToTrue()
    {
        var item = new NavigationItem { Id = "test", DisplayName = "Test" };
        item.IsEnabled.Should().BeTrue();
    }

    [Fact]
    public void IsSelected_DefaultsToFalse()
    {
        var item = new NavigationItem { Id = "test", DisplayName = "Test" };
        item.IsSelected.Should().BeFalse();
    }

    [Fact]
    public void IsSelected_SameValue_DoesNotFirePropertyChanged()
    {
        var item = new NavigationItem { Id = "test", DisplayName = "Test" };
        var fired = false;
        item.PropertyChanged += (_, _) => fired = true;

        item.IsSelected = false; // same as default

        fired.Should().BeFalse();
    }

    [Fact]
    public void NavigationItem_BadgeProperties_UsedForSidebarRendering()
    {
        // Badges are displayed in the sidebar to indicate pending items
        var item = new NavigationItem
        {
            Id = "tasks",
            DisplayName = "Tasks",
            ShowBadge = true,
            BadgeCount = 5
        };

        item.ShowBadge.Should().BeTrue();
        item.BadgeCount.Should().Be(5);
    }

    [Fact]
    public void NavigationItem_ExperimentalAndHardLocked_ControlVisibility()
    {
        // Experimental plugins show a warning icon, hard-locked plugins are greyed out
        var item = new NavigationItem
        {
            Id = "vault",
            DisplayName = "Vault",
            IsExperimental = true,
            IsHardLocked = true,
            HardLockedReason = "Requires license activation"
        };

        item.IsExperimental.Should().BeTrue();
        item.IsHardLocked.Should().BeTrue();
        item.HardLockedReason.Should().Be("Requires license activation");
    }

    [Fact]
    public void NavigationItem_DisplayMetadata_UsedForTooltipsAndShortcuts()
    {
        // Subtitle, Icon, Tooltip, ShortcutHint are rendered in the sidebar
        var item = new NavigationItem
        {
            Id = "notes",
            DisplayName = "Notes",
            Subtitle = "All your notes",
            Icon = "📝",
            Tooltip = "Open the notes plugin",
            ShortcutHint = "Ctrl+1"
        };

        item.Subtitle.Should().Be("All your notes");
        item.Icon.Should().Be("📝");
        item.Tooltip.Should().Be("Open the notes plugin");
        item.ShortcutHint.Should().Be("Ctrl+1");
    }

    [Fact]
    public void NavigationItem_Order_CanBeUpdatedForDragReordering()
    {
        // Order is mutable (set;) because users can drag-reorder navigation items
        var item = new NavigationItem { Id = "test", DisplayName = "Test" };
        item.Order.Should().Be(1000);

        item.Order = 100;
        item.Order.Should().Be(100);
    }
}
