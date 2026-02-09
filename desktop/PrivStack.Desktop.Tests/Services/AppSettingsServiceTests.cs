using System.Text.Json;
using PrivStack.Desktop.Services;

namespace PrivStack.Desktop.Tests.Services;

public class AppSettingsTests
{
    [Fact]
    public void DefaultSettings_HaveExpectedValues()
    {
        var settings = new AppSettings();

        settings.WindowWidth.Should().Be(1400);
        settings.WindowHeight.Should().Be(900);
        settings.Theme.Should().Be("Dark");
        settings.LastActiveTab.Should().Be("Notes");
        settings.SidebarWidth.Should().Be(200);
        settings.SidebarCollapsed.Should().BeFalse();
        settings.CalendarViewMode.Should().Be("month");
        settings.TasksViewMode.Should().Be("list");
        settings.MaxBackups.Should().Be(7);
        settings.BackupFrequency.Should().Be("Daily");
        settings.BackupType.Should().Be("Rolling");
        settings.SensitiveLockoutMinutes.Should().Be(5);
        settings.FontScaleMultiplier.Should().Be(1.0);
        settings.ExperimentalPluginsEnabled.Should().BeFalse();
        settings.SeedDataVersion.Should().Be(0);
    }

    [Fact]
    public void Settings_RoundTrips_ViaJson()
    {
        var settings = new AppSettings
        {
            WindowWidth = 1920,
            WindowHeight = 1080,
            Theme = "Light",
            LastActiveTab = "Tasks",
            PluginOrder = ["notes", "tasks", "calendar"],
            DisabledPlugins = ["experimental-plugin"],
            PluginSettings = new Dictionary<string, string>
            {
                ["plugin.notes.sort"] = "\"title\""
            }
        };

        var json = JsonSerializer.Serialize(settings, new JsonSerializerOptions { WriteIndented = true });
        var deserialized = JsonSerializer.Deserialize<AppSettings>(json);

        deserialized.Should().NotBeNull();
        deserialized!.WindowWidth.Should().Be(1920);
        deserialized.Theme.Should().Be("Light");
        deserialized.LastActiveTab.Should().Be("Tasks");
        deserialized.PluginOrder.Should().BeEquivalentTo(["notes", "tasks", "calendar"]);
        deserialized.DisabledPlugins.Should().Contain("experimental-plugin");
        deserialized.PluginSettings.Should().ContainKey("plugin.notes.sort");
    }

    [Fact]
    public void Settings_DeserializeFromCorruptJson_ThrowsJsonException()
    {
        var act = () => JsonSerializer.Deserialize<AppSettings>("not valid json {{{");
        act.Should().Throw<JsonException>();
    }

    [Fact]
    public void Settings_DeserializeFromEmptyObject_ReturnsDefaults()
    {
        var result = JsonSerializer.Deserialize<AppSettings>("{}");

        result.Should().NotBeNull();
        result!.WindowWidth.Should().Be(1400);
        result.Theme.Should().Be("Dark");
    }

    [Fact]
    public void PluginSettings_NamespacedKeys()
    {
        var settings = new AppSettings();
        var key = "plugin.privstack.notes.sort_order";
        settings.PluginSettings[key] = JsonSerializer.Serialize("title_asc");

        var stored = settings.PluginSettings[key];
        var value = JsonSerializer.Deserialize<string>(stored);

        value.Should().Be("title_asc");
    }

    [Fact]
    public void ActiveTimerState_RoundTrips()
    {
        var timer = new ActiveTimerState
        {
            TaskId = "task-1",
            TaskTitle = "Build feature",
            StartedAtUtc = new DateTime(2025, 6, 15, 10, 0, 0, DateTimeKind.Utc),
            ElapsedSecondsBefore = 120.5,
            IsPaused = true
        };

        var json = JsonSerializer.Serialize(timer);
        var deserialized = JsonSerializer.Deserialize<ActiveTimerState>(json);

        deserialized!.TaskId.Should().Be("task-1");
        deserialized.TaskTitle.Should().Be("Build feature");
        deserialized.ElapsedSecondsBefore.Should().Be(120.5);
        deserialized.IsPaused.Should().BeTrue();
    }

    [Fact]
    public void Settings_WithActiveTimer_RoundTrips()
    {
        var settings = new AppSettings
        {
            ActiveTimer = new ActiveTimerState
            {
                TaskId = "t1",
                TaskTitle = "Test",
                StartedAtUtc = DateTime.UtcNow
            }
        };

        var json = JsonSerializer.Serialize(settings);
        var deserialized = JsonSerializer.Deserialize<AppSettings>(json);

        deserialized!.ActiveTimer.Should().NotBeNull();
        deserialized.ActiveTimer!.TaskId.Should().Be("t1");
    }

    [Fact]
    public void Settings_PluginOrder_DefaultsToEmptyList()
    {
        var settings = new AppSettings();
        settings.PluginOrder.Should().BeEmpty();
    }

    [Fact]
    public void Settings_DisabledPlugins_DefaultsToEmptySet()
    {
        var settings = new AppSettings();
        settings.DisabledPlugins.Should().BeEmpty();
    }
}
