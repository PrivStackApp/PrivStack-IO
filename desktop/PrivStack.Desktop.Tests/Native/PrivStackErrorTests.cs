using PrivStack.Desktop.Native;

namespace PrivStack.Desktop.Tests.Native;

public class PrivStackErrorTests
{
    [Theory]
    [InlineData(PrivStackError.Ok, 0)]
    [InlineData(PrivStackError.NullPointer, 1)]
    [InlineData(PrivStackError.InvalidUtf8, 2)]
    [InlineData(PrivStackError.JsonError, 3)]
    [InlineData(PrivStackError.StorageError, 4)]
    [InlineData(PrivStackError.NotFound, 5)]
    [InlineData(PrivStackError.NotInitialized, 6)]
    [InlineData(PrivStackError.SyncNotRunning, 7)]
    [InlineData(PrivStackError.SyncAlreadyRunning, 8)]
    [InlineData(PrivStackError.SyncError, 9)]
    [InlineData(PrivStackError.PeerNotFound, 10)]
    [InlineData(PrivStackError.AuthError, 11)]
    [InlineData(PrivStackError.CloudError, 12)]
    [InlineData(PrivStackError.LicenseInvalidFormat, 13)]
    [InlineData(PrivStackError.LicenseInvalidSignature, 14)]
    [InlineData(PrivStackError.LicenseExpired, 15)]
    [InlineData(PrivStackError.LicenseNotActivated, 16)]
    [InlineData(PrivStackError.LicenseActivationFailed, 17)]
    [InlineData(PrivStackError.Unknown, 99)]
    public void ErrorCode_HasExpectedIntegerValue(PrivStackError error, int expectedValue)
    {
        ((int)error).Should().Be(expectedValue);
    }

    [Fact]
    public void AllErrorCodes_AreUnique()
    {
        var values = Enum.GetValues<PrivStackError>().Select(e => (int)e).ToList();
        values.Should().OnlyHaveUniqueItems();
    }
}
