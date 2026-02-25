namespace PrivStack.Desktop.Tests.Sdk;

using PrivStack.Sdk.Capabilities;

public class ReminderModelsTests
{
    [Fact]
    public void ReminderInfo_construction()
    {
        var fireAt = DateTimeOffset.UtcNow.AddHours(1);
        var reminder = new ReminderInfo
        {
            Key = "privstack.tasks:t-123:1707350400",
            Title = "Complete report",
            Body = "Report is due soon",
            FireAtUtc = fireAt,
            SourcePluginId = "privstack.tasks",
            ItemId = "t-123"
        };
        reminder.Key.Should().Contain("privstack.tasks");
        reminder.Title.Should().Be("Complete report");
        reminder.FireAtUtc.Should().Be(fireAt);
        reminder.SourcePluginId.Should().Be("privstack.tasks");
        reminder.ItemId.Should().Be("t-123");
    }

    [Fact]
    public void ReminderInfo_is_record_with_equality()
    {
        var fireAt = new DateTimeOffset(2024, 6, 15, 12, 0, 0, TimeSpan.Zero);
        var a = new ReminderInfo
        {
            Key = "k",
            Title = "T",
            Body = "B",
            FireAtUtc = fireAt,
            SourcePluginId = "p",
            ItemId = "i"
        };
        var b = new ReminderInfo
        {
            Key = "k",
            Title = "T",
            Body = "B",
            FireAtUtc = fireAt,
            SourcePluginId = "p",
            ItemId = "i"
        };
        a.Should().Be(b);
    }
}
