namespace PrivStack.Desktop.Tests.Services;

using PrivStack.Desktop.Services;

public class FocusModeServiceTests
{
    [Fact]
    public void IsFocusMode_defaults_to_false()
    {
        var service = new FocusModeService();
        service.IsFocusMode.Should().BeFalse();
    }

    [Fact]
    public void SetFocusMode_enables_focus_mode()
    {
        var service = new FocusModeService();
        service.SetFocusMode(true);
        service.IsFocusMode.Should().BeTrue();
    }

    [Fact]
    public void SetFocusMode_disables_focus_mode()
    {
        var service = new FocusModeService();
        service.SetFocusMode(true);
        service.SetFocusMode(false);
        service.IsFocusMode.Should().BeFalse();
    }

    [Fact]
    public void SetFocusMode_fires_event_on_change()
    {
        var service = new FocusModeService();
        bool? receivedValue = null;
        service.FocusModeChanged += value => receivedValue = value;

        service.SetFocusMode(true);
        receivedValue.Should().BeTrue();
    }

    [Fact]
    public void SetFocusMode_does_not_fire_event_when_same_value()
    {
        var service = new FocusModeService();
        var eventCount = 0;
        service.FocusModeChanged += _ => eventCount++;

        service.SetFocusMode(false); // already false
        eventCount.Should().Be(0);
    }

    [Fact]
    public void SetFocusMode_fires_event_with_correct_value_on_disable()
    {
        var service = new FocusModeService();
        service.SetFocusMode(true);

        bool? receivedValue = null;
        service.FocusModeChanged += value => receivedValue = value;

        service.SetFocusMode(false);
        receivedValue.Should().BeFalse();
    }

    [Fact]
    public void SetFocusMode_toggle_fires_two_events()
    {
        var service = new FocusModeService();
        var values = new List<bool>();
        service.FocusModeChanged += value => values.Add(value);

        service.SetFocusMode(true);
        service.SetFocusMode(false);

        values.Should().HaveCount(2);
        values[0].Should().BeTrue();
        values[1].Should().BeFalse();
    }
}
