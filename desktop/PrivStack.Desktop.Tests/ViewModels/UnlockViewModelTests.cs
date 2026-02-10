using PrivStack.Desktop.Native;
using PrivStack.Desktop.Services.Abstractions;
using PrivStack.Desktop.ViewModels;

namespace PrivStack.Desktop.Tests.ViewModels;

/// <summary>
/// Tests for UnlockViewModel authentication flow and state management.
/// </summary>
public class UnlockViewModelTests
{
    private static UnlockViewModel CreateVm(IAuthService? authService = null)
    {
        return new UnlockViewModel(
            authService ?? Substitute.For<IAuthService>(),
            Substitute.For<IPrivStackRuntime>(),
            Substitute.For<IWorkspaceService>());
    }

    [Fact]
    public void Constructor_InitializesWithEmptyState()
    {
        var vm = CreateVm();

        vm.MasterPassword.Should().BeEmpty();
        vm.ErrorMessage.Should().BeEmpty();
        vm.IsLoading.Should().BeFalse();
        vm.HasError.Should().BeFalse();
        vm.CanUnlock.Should().BeFalse();
    }

    [Fact]
    public void CanUnlock_IsFalse_WhenPasswordIsEmpty()
    {
        var vm = CreateVm();
        vm.MasterPassword = string.Empty;

        vm.CanUnlock.Should().BeFalse();
    }

    [Fact]
    public void CanUnlock_IsFalse_WhenPasswordIsWhitespace()
    {
        var vm = CreateVm();
        vm.MasterPassword = "   ";

        vm.CanUnlock.Should().BeFalse();
    }

    [Fact]
    public void CanUnlock_IsTrue_WhenPasswordIsProvided()
    {
        var vm = CreateVm();
        vm.MasterPassword = "SecurePassword123";

        vm.CanUnlock.Should().BeTrue();
    }

    [Fact]
    public void CanUnlock_IsFalse_WhenIsLoadingIsTrue()
    {
        var vm = CreateVm();
        vm.MasterPassword = "SecurePassword123";
        vm.IsLoading = true;

        vm.CanUnlock.Should().BeFalse();
    }

    [Fact]
    public void HasError_IsTrue_WhenErrorMessageIsSet()
    {
        var vm = CreateVm();
        vm.ErrorMessage = "Something went wrong";

        vm.HasError.Should().BeTrue();
    }

    [Fact]
    public void HasError_IsFalse_WhenErrorMessageIsEmpty()
    {
        var vm = CreateVm();
        vm.ErrorMessage = string.Empty;

        vm.HasError.Should().BeFalse();
    }

    [Fact]
    public void OnMasterPasswordChanged_ClearsErrorMessage()
    {
        var vm = CreateVm();
        vm.ErrorMessage = "Previous error";

        vm.MasterPassword = "NewPassword";

        vm.ErrorMessage.Should().BeEmpty();
        vm.HasError.Should().BeFalse();
    }

    [Fact]
    public async Task UnlockAsync_CallsAuthService_WithCorrectPassword()
    {
        var authService = Substitute.For<IAuthService>();
        var vm = CreateVm(authService);
        vm.MasterPassword = "TestPassword123";

        await vm.UnlockCommand.ExecuteAsync(null);

        authService.Received(1).UnlockApp("TestPassword123");
    }

    [Fact]
    public async Task UnlockAsync_SetsIsLoading_DuringExecution()
    {
        var authService = Substitute.For<IAuthService>();
        var loadingStates = new List<bool>();

        authService.When(x => x.UnlockApp(Arg.Any<string>()))
            .Do(_ => loadingStates.Add(true));

        var vm = CreateVm(authService);
        vm.MasterPassword = "TestPassword123";

        vm.PropertyChanged += (_, e) =>
        {
            if (e.PropertyName == nameof(vm.IsLoading))
                loadingStates.Add(vm.IsLoading);
        };

        await vm.UnlockCommand.ExecuteAsync(null);

        loadingStates.Should().Contain(true);
        vm.IsLoading.Should().BeFalse();
    }

    [Fact]
    public async Task UnlockAsync_ClearsErrorMessage_BeforeUnlock()
    {
        var authService = Substitute.For<IAuthService>();
        var vm = CreateVm(authService);
        vm.MasterPassword = "TestPassword123";
        vm.ErrorMessage = "Old error";

        await vm.UnlockCommand.ExecuteAsync(null);

        vm.ErrorMessage.Should().BeEmpty();
    }

    [Fact]
    public async Task UnlockAsync_ClearsPassword_OnSuccess()
    {
        var authService = Substitute.For<IAuthService>();
        var vm = CreateVm(authService);
        vm.MasterPassword = "TestPassword123";

        await vm.UnlockCommand.ExecuteAsync(null);

        vm.MasterPassword.Should().BeEmpty();
    }

    [Fact]
    public async Task UnlockAsync_RaisesAppUnlockedEvent_OnSuccess()
    {
        var authService = Substitute.For<IAuthService>();
        var vm = CreateVm(authService);
        vm.MasterPassword = "TestPassword123";

        var eventRaised = false;
        vm.AppUnlocked += (_, _) => eventRaised = true;

        await vm.UnlockCommand.ExecuteAsync(null);

        eventRaised.Should().BeTrue();
    }

    [Fact]
    public async Task UnlockAsync_SetsErrorMessage_OnPrivStackException()
    {
        var authService = Substitute.For<IAuthService>();
        authService.When(x => x.UnlockApp(Arg.Any<string>()))
            .Do(_ => throw new PrivStackException("Invalid credentials", PrivStackError.AuthError));

        var vm = CreateVm(authService);
        vm.MasterPassword = "WrongPassword";

        await vm.UnlockCommand.ExecuteAsync(null);

        vm.ErrorMessage.Should().Be("Incorrect password. Please try again.");
        vm.HasError.Should().BeTrue();
    }

    [Fact]
    public async Task UnlockAsync_DoesNotRaiseAppUnlockedEvent_OnPrivStackException()
    {
        var authService = Substitute.For<IAuthService>();
        authService.When(x => x.UnlockApp(Arg.Any<string>()))
            .Do(_ => throw new PrivStackException("Invalid credentials", PrivStackError.AuthError));

        var vm = CreateVm(authService);
        vm.MasterPassword = "WrongPassword";

        var eventRaised = false;
        vm.AppUnlocked += (_, _) => eventRaised = true;

        await vm.UnlockCommand.ExecuteAsync(null);

        eventRaised.Should().BeFalse();
    }

    [Fact]
    public async Task UnlockAsync_SetsGenericErrorMessage_OnOtherException()
    {
        var authService = Substitute.For<IAuthService>();
        authService.When(x => x.UnlockApp(Arg.Any<string>()))
            .Do(_ => throw new InvalidOperationException("Database connection failed"));

        var vm = CreateVm(authService);
        vm.MasterPassword = "TestPassword123";

        await vm.UnlockCommand.ExecuteAsync(null);

        vm.ErrorMessage.Should().Be("Failed to unlock: Database connection failed");
        vm.HasError.Should().BeTrue();
    }

    [Fact]
    public async Task UnlockAsync_ResetsIsLoading_OnException()
    {
        var authService = Substitute.For<IAuthService>();
        authService.When(x => x.UnlockApp(Arg.Any<string>()))
            .Do(_ => throw new Exception("Test error"));

        var vm = CreateVm(authService);
        vm.MasterPassword = "TestPassword123";

        await vm.UnlockCommand.ExecuteAsync(null);

        vm.IsLoading.Should().BeFalse();
    }

    [Fact]
    public void RequestLock_CallsAuthServiceLockApp()
    {
        var authService = Substitute.For<IAuthService>();
        var vm = CreateVm(authService);

        vm.RequestLock();

        authService.Received(1).LockApp();
    }

    [Fact]
    public void RequestLock_ClearsMasterPassword()
    {
        var authService = Substitute.For<IAuthService>();
        var vm = CreateVm(authService);
        vm.MasterPassword = "TestPassword123";

        vm.RequestLock();

        vm.MasterPassword.Should().BeEmpty();
    }

    [Fact]
    public void RequestLock_ClearsErrorMessage()
    {
        var authService = Substitute.For<IAuthService>();
        var vm = CreateVm(authService);
        vm.ErrorMessage = "Some error";

        vm.RequestLock();

        vm.ErrorMessage.Should().BeEmpty();
    }

    [Fact]
    public void RequestLock_RaisesLockRequestedEvent()
    {
        var authService = Substitute.For<IAuthService>();
        var vm = CreateVm(authService);

        var eventRaised = false;
        vm.LockRequested += (_, _) => eventRaised = true;

        vm.RequestLock();

        eventRaised.Should().BeTrue();
    }

    [Fact]
    public void RequestLock_SetsErrorMessage_OnException()
    {
        var authService = Substitute.For<IAuthService>();
        authService.When(x => x.LockApp())
            .Do(_ => throw new InvalidOperationException("Lock failed"));

        var vm = CreateVm(authService);

        vm.RequestLock();

        vm.ErrorMessage.Should().Be("Failed to lock: Lock failed");
        vm.HasError.Should().BeTrue();
    }

    [Fact]
    public void RequestLock_DoesNotRaiseLockRequestedEvent_OnException()
    {
        var authService = Substitute.For<IAuthService>();
        authService.When(x => x.LockApp())
            .Do(_ => throw new Exception("Test error"));

        var vm = CreateVm(authService);

        var eventRaised = false;
        vm.LockRequested += (_, _) => eventRaised = true;

        vm.RequestLock();

        eventRaised.Should().BeFalse();
    }

    [Fact]
    public void UnlockCommand_NotifyCanExecuteChanged_WhenIsLoadingChanges()
    {
        var vm = CreateVm();
        vm.MasterPassword = "TestPassword123";

        var canExecuteBefore = vm.UnlockCommand.CanExecute(null);
        vm.IsLoading = true;
        var canExecuteAfter = vm.UnlockCommand.CanExecute(null);

        canExecuteBefore.Should().BeTrue();
        canExecuteAfter.Should().BeFalse();
    }

    [Fact]
    public void UnlockCommand_NotifyCanExecuteChanged_WhenMasterPasswordChanges()
    {
        var vm = CreateVm();

        var canExecuteBefore = vm.UnlockCommand.CanExecute(null);
        vm.MasterPassword = "TestPassword123";
        var canExecuteAfter = vm.UnlockCommand.CanExecute(null);

        canExecuteBefore.Should().BeFalse();
        canExecuteAfter.Should().BeTrue();
    }
}
