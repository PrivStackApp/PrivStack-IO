using PrivStack.Sdk;

namespace PrivStack.Desktop.Tests.Sdk;

public class PluginHostTests
{
    [Fact]
    public void IPluginHost_Interface_HasExpectedMembers()
    {
        var host = Substitute.For<IPluginHost>();
        host.Sdk.Returns(Substitute.For<IPrivStackSdk>());
        host.Capabilities.Returns(Substitute.For<ICapabilityBroker>());
        host.Settings.Returns(Substitute.For<IPluginSettings>());
        host.Logger.Returns(Substitute.For<IPluginLogger>());
        host.Navigation.Returns(Substitute.For<INavigationService>());
        host.AppVersion.Returns(new Version(1, 0, 0));

        host.Sdk.Should().NotBeNull();
        host.Capabilities.Should().NotBeNull();
        host.Settings.Should().NotBeNull();
        host.Logger.Should().NotBeNull();
        host.Navigation.Should().NotBeNull();
        host.AppVersion.Should().Be(new Version(1, 0, 0));
    }

    [Fact]
    public void PluginSettings_MockBehavior()
    {
        var settings = Substitute.For<IPluginSettings>();
        settings.Get("sort", "title").Returns("date");

        var result = settings.Get("sort", "title");

        result.Should().Be("date");
    }

    [Fact]
    public void PluginLogger_MockBehavior()
    {
        var logger = Substitute.For<IPluginLogger>();

        // Should not throw
        logger.Debug("Test {Value}", "hello");
        logger.Info("Info {Value}", "world");
        logger.Warn("Warn {Value}", "caution");
        logger.Error("Error {Value}", "oops");
        logger.Error(new Exception("test"), "Error with exception {Value}", "details");

        logger.Received(1).Debug("Test {Value}", "hello");
        logger.Received(1).Error(Arg.Any<Exception>(), "Error with exception {Value}", "details");
    }

    [Fact]
    public void NavigationService_MockBehavior()
    {
        var nav = Substitute.For<INavigationService>();

        nav.NavigateTo("privstack.notes");
        nav.NavigateBack();

        nav.Received(1).NavigateTo("privstack.notes");
        nav.Received(1).NavigateBack();
    }
}
