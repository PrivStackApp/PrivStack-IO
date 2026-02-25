namespace PrivStack.Desktop.Services.Biometric;

/// <summary>
/// Platform abstraction for biometric authentication (Touch ID / Windows Hello).
/// Stores the master password in the OS-level secure keychain, gated by biometric verification.
/// </summary>
public interface IBiometricService
{
    /// <summary>
    /// Whether biometric hardware exists on this platform (compile-time capability).
    /// </summary>
    bool IsSupported { get; }

    /// <summary>
    /// Checks at runtime whether biometric hardware is available and configured.
    /// </summary>
    Task<bool> IsAvailableAsync();

    /// <summary>
    /// Whether the user has enrolled their master password for biometric unlock.
    /// </summary>
    bool IsEnrolled { get; }

    /// <summary>
    /// Display name for the biometric method ("Touch ID", "Windows Hello").
    /// </summary>
    string BiometricDisplayName { get; }

    /// <summary>
    /// Stores the master password in the OS keychain, protected by biometric access control.
    /// </summary>
    Task<bool> EnrollAsync(string masterPassword);

    /// <summary>
    /// Prompts biometric verification and retrieves the stored master password.
    /// Returns null if the user cancels or verification fails.
    /// </summary>
    Task<string?> AuthenticateAsync(string reason);

    /// <summary>
    /// Removes the stored master password from the OS keychain.
    /// </summary>
    void Unenroll();
}
