namespace PrivStack.Desktop.Services.Biometric;

/// <summary>
/// Fallback biometric service for Linux and other unsupported platforms.
/// All operations gracefully return false/null.
/// </summary>
public class NullBiometricService : IBiometricService
{
    public bool IsSupported => false;
    public bool IsEnrolled => false;
    public string BiometricDisplayName => "Biometric";

    public Task<bool> IsAvailableAsync() => Task.FromResult(false);
    public Task<bool> EnrollAsync(string masterPassword) => Task.FromResult(false);
    public Task<string?> AuthenticateAsync(string reason) => Task.FromResult<string?>(null);
    public Task<bool> VerifyBiometricAsync(string reason) => Task.FromResult(false);
    public void Unenroll() { }
}
