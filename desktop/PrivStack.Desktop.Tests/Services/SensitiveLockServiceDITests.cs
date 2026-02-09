using PrivStack.Desktop.Native;
using PrivStack.Desktop.Services;

namespace PrivStack.Desktop.Tests.Services;

public class SensitiveLockServiceDITests
{
    private readonly IAuthService _mockAuthService;
    private readonly SensitiveLockService _sut;

    public SensitiveLockServiceDITests()
    {
        _mockAuthService = Substitute.For<IAuthService>();
        _sut = new SensitiveLockService(_mockAuthService);
    }

    [Fact]
    public void Unlock_WithValidPassword_Succeeds()
    {
        // Arrange
        const string validPassword = "correct-password";
        _mockAuthService.ValidateMasterPassword(validPassword).Returns(true);

        var unlockedEventRaised = false;
        _sut.Unlocked += (_, _) => unlockedEventRaised = true;

        // Act
        _sut.Unlock(validPassword);

        // Assert
        _sut.IsSensitiveUnlocked.Should().BeTrue();
        unlockedEventRaised.Should().BeTrue();
        _mockAuthService.Received(1).ValidateMasterPassword(validPassword);
    }

    [Fact]
    public void Unlock_WithInvalidPassword_Fails()
    {
        // Arrange
        const string invalidPassword = "wrong-password";
        _mockAuthService.ValidateMasterPassword(invalidPassword).Returns(false);

        var unlockedEventRaised = false;
        _sut.Unlocked += (_, _) => unlockedEventRaised = true;

        // Act
        _sut.Unlock(invalidPassword);

        // Assert
        _sut.IsSensitiveUnlocked.Should().BeFalse();
        unlockedEventRaised.Should().BeFalse();
        _mockAuthService.Received(1).ValidateMasterPassword(invalidPassword);
    }

    [Fact]
    public void UnlockWithoutValidation_Unlocks_WithoutCallingNativeService()
    {
        // Arrange
        var unlockedEventRaised = false;
        _sut.Unlocked += (_, _) => unlockedEventRaised = true;

        // Act
        _sut.UnlockWithoutValidation();

        // Assert
        _sut.IsSensitiveUnlocked.Should().BeTrue();
        unlockedEventRaised.Should().BeTrue();
        _mockAuthService.DidNotReceive().ValidateMasterPassword(Arg.Any<string>());
        _mockAuthService.DidNotReceive().IsAuthUnlocked();
    }

    [Fact]
    public void Lock_AfterUnlock_FiresLockedEvent()
    {
        // Arrange
        _sut.UnlockWithoutValidation();

        var lockedEventRaised = false;
        _sut.Locked += (_, _) => lockedEventRaised = true;

        // Act
        _sut.Lock();

        // Assert
        _sut.IsSensitiveUnlocked.Should().BeFalse();
        lockedEventRaised.Should().BeTrue();
    }

    [Fact]
    public void Unlock_FiresUnlockedEvent()
    {
        // Arrange
        const string validPassword = "test-password";
        _mockAuthService.ValidateMasterPassword(validPassword).Returns(true);

        var unlockedEventFired = false;
        _sut.Unlocked += (sender, args) =>
        {
            unlockedEventFired = true;
            sender.Should().Be(_sut);
        };

        // Act
        _sut.Unlock(validPassword);

        // Assert
        unlockedEventFired.Should().BeTrue();
    }

    [Fact]
    public void RecordActivity_WhenLocked_IsNoOp()
    {
        // Arrange - service starts locked by default
        _sut.IsSensitiveUnlocked.Should().BeFalse();

        var lockedEventRaised = false;
        var unlockedEventRaised = false;
        _sut.Locked += (_, _) => lockedEventRaised = true;
        _sut.Unlocked += (_, _) => unlockedEventRaised = true;

        // Act
        _sut.RecordActivity();

        // Assert - state should remain locked, no events fired
        _sut.IsSensitiveUnlocked.Should().BeFalse();
        lockedEventRaised.Should().BeFalse();
        unlockedEventRaised.Should().BeFalse();
    }

    [Fact]
    public void Constructor_InitializesWithLockedState()
    {
        // Arrange & Act
        var service = new SensitiveLockService(_mockAuthService);

        // Assert
        service.IsSensitiveUnlocked.Should().BeFalse();
    }

    [Fact]
    public void LockoutMinutes_CanBeSetAndRetrieved()
    {
        // Arrange
        const int expectedMinutes = 15;

        // Act
        _sut.LockoutMinutes = expectedMinutes;

        // Assert
        _sut.LockoutMinutes.Should().Be(expectedMinutes);
    }
}
