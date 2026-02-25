namespace PrivStack.Desktop.Tests.Services;

using PrivStack.Desktop.Services;
using PrivStack.Sdk;

public class ToastServiceTests
{
    // =========================================================================
    // ActiveToast
    // =========================================================================

    [Fact]
    public void ActiveToast_defaults()
    {
        var toast = new ActiveToast();
        toast.Id.Should().NotBeNullOrEmpty();
        toast.Message.Should().BeEmpty();
        toast.FriendlyLabel.Should().BeEmpty();
        toast.ActionLabel.Should().BeNull();
        toast.Action.Should().BeNull();
    }

    [Fact]
    public void ActiveToast_Id_is_unique()
    {
        var a = new ActiveToast();
        var b = new ActiveToast();
        a.Id.Should().NotBe(b.Id);
    }

    [Theory]
    [InlineData(ToastType.Success, true, false, false, false)]
    [InlineData(ToastType.Info, false, true, false, false)]
    [InlineData(ToastType.Warning, false, false, true, false)]
    [InlineData(ToastType.Error, false, false, false, true)]
    public void ActiveToast_type_flags(ToastType type, bool isSuccess, bool isInfo, bool isWarning, bool isError)
    {
        var toast = new ActiveToast { Type = type };
        toast.IsSuccess.Should().Be(isSuccess);
        toast.IsInfo.Should().Be(isInfo);
        toast.IsWarning.Should().Be(isWarning);
        toast.IsError.Should().Be(isError);
    }

    [Theory]
    [InlineData(ToastType.Success, "toast-success")]
    [InlineData(ToastType.Info, "toast-info")]
    [InlineData(ToastType.Warning, "toast-warning")]
    [InlineData(ToastType.Error, "toast-error")]
    public void ActiveToast_TypeClass(ToastType type, string expected)
    {
        var toast = new ActiveToast { Type = type };
        toast.TypeClass.Should().Be(expected);
    }

    // =========================================================================
    // ToastService static methods
    // =========================================================================

    [Theory]
    [InlineData(ToastType.Success, "All Set")]
    [InlineData(ToastType.Info, "FYI")]
    [InlineData(ToastType.Warning, "Heads Up")]
    [InlineData(ToastType.Error, "Action Needed")]
    public void GetDisplayLabel_returns_correct_label(ToastType type, string expected)
    {
        ToastService.GetDisplayLabel(type).Should().Be(expected);
    }
}
