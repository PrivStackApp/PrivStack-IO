using System.Runtime.InteropServices;
using System.Text;
using Serilog;

namespace PrivStack.Services.Biometric;

/// <summary>
/// Windows biometric service using Windows Hello (UserConsentVerifier)
/// and Credential Manager for secure password storage.
/// </summary>
public class WindowsBiometricService : IBiometricService
{
    private static readonly ILogger _log = Log.ForContext<WindowsBiometricService>();

    private const string CredentialTarget = "PrivStack:MasterPassword";

    public bool IsSupported => true;
    public string BiometricDisplayName => "Windows Hello";

    public bool IsEnrolled => CredRead(CredentialTarget, 1, 0, out var cred) && cred != IntPtr.Zero && CredFreeHelper(cred);

    public async Task<bool> IsAvailableAsync()
    {
        return await Task.Run(() =>
        {
            try
            {
                // Check if Windows Hello is configured by attempting UserConsentVerifier availability
                // We use the WinRT API via COM interop
                var availability = CheckUserConsentVerifierAvailability();
                return availability;
            }
            catch (Exception ex)
            {
                _log.Warning(ex, "Failed to check Windows Hello availability");
                return false;
            }
        });
    }

    public async Task<bool> EnrollAsync(string masterPassword)
    {
        return await Task.Run(() =>
        {
            try
            {
                var passwordBytes = Encoding.UTF8.GetBytes(masterPassword);

                var credential = new CREDENTIAL
                {
                    Type = 1, // CRED_TYPE_GENERIC
                    TargetName = CredentialTarget,
                    CredentialBlobSize = passwordBytes.Length,
                    CredentialBlob = Marshal.AllocHGlobal(passwordBytes.Length),
                    Persist = 2, // CRED_PERSIST_LOCAL_MACHINE
                    UserName = "PrivStack"
                };

                Marshal.Copy(passwordBytes, 0, credential.CredentialBlob, passwordBytes.Length);

                try
                {
                    var success = CredWrite(ref credential, 0);
                    if (success)
                    {
                        _log.Information("Biometric enrollment successful (credential stored)");
                        return true;
                    }

                    _log.Error("CredWrite failed: {Error}", Marshal.GetLastWin32Error());
                    return false;
                }
                finally
                {
                    Marshal.FreeHGlobal(credential.CredentialBlob);
                    Array.Clear(passwordBytes);
                }
            }
            catch (Exception ex)
            {
                _log.Error(ex, "Biometric enrollment failed");
                return false;
            }
        });
    }

    public async Task<string?> AuthenticateAsync(string reason)
    {
        try
        {
            // First verify biometric
            var verified = await RequestUserConsentAsync(reason);
            if (!verified)
            {
                _log.Debug("Windows Hello verification cancelled or failed");
                return null;
            }

            // Read credential
            if (!CredRead(CredentialTarget, 1, 0, out var credPtr) || credPtr == IntPtr.Zero)
            {
                _log.Warning("No stored credential found after biometric verification");
                return null;
            }

            try
            {
                var cred = Marshal.PtrToStructure<CREDENTIAL>(credPtr);
                if (cred.CredentialBlobSize > 0 && cred.CredentialBlob != IntPtr.Zero)
                {
                    var bytes = new byte[cred.CredentialBlobSize];
                    Marshal.Copy(cred.CredentialBlob, bytes, 0, cred.CredentialBlobSize);
                    var password = Encoding.UTF8.GetString(bytes);
                    Array.Clear(bytes);
                    _log.Information("Biometric authentication successful");
                    return password;
                }

                return null;
            }
            finally
            {
                CredFree(credPtr);
            }
        }
        catch (Exception ex)
        {
            _log.Error(ex, "Biometric authentication failed");
            return null;
        }
    }

    public async Task<bool> VerifyBiometricAsync(string reason)
    {
        return await RequestUserConsentAsync(reason);
    }

    public void Unenroll()
    {
        try
        {
            CredDelete(CredentialTarget, 1, 0);
            _log.Information("Biometric enrollment removed");
        }
        catch (Exception ex)
        {
            _log.Warning(ex, "Failed to delete credential");
        }
    }

    // --- Windows Hello WinRT helpers ---

    private static bool CheckUserConsentVerifierAvailability()
    {
        try
        {
            // Use PowerShell to check Windows Hello availability via WinRT
            // This avoids complex COM interop while remaining reliable
            var psi = new System.Diagnostics.ProcessStartInfo
            {
                FileName = "powershell",
                Arguments = "-NoProfile -Command \"[Windows.Security.Credentials.UI.UserConsentVerifier, Windows.Security.Credentials.UI, ContentType=WindowsRuntime] | Out-Null; $r = [Windows.Security.Credentials.UI.UserConsentVerifier]::CheckAvailabilityAsync().AsTask().Result; if ($r -eq 'Available') { exit 0 } else { exit 1 }\"",
                UseShellExecute = false,
                CreateNoWindow = true,
                RedirectStandardOutput = true,
                RedirectStandardError = true
            };

            using var proc = System.Diagnostics.Process.Start(psi);
            proc?.WaitForExit(5000);
            return proc?.ExitCode == 0;
        }
        catch
        {
            return false;
        }
    }

    private static async Task<bool> RequestUserConsentAsync(string reason)
    {
        return await Task.Run(() =>
        {
            try
            {
                var psi = new System.Diagnostics.ProcessStartInfo
                {
                    FileName = "powershell",
                    Arguments = $"-NoProfile -Command \"[Windows.Security.Credentials.UI.UserConsentVerifier, Windows.Security.Credentials.UI, ContentType=WindowsRuntime] | Out-Null; $r = [Windows.Security.Credentials.UI.UserConsentVerifier]::RequestVerificationAsync('{reason.Replace("'", "''")}').AsTask().Result; if ($r -eq 'Verified') {{ exit 0 }} else {{ exit 1 }}\"",
                    UseShellExecute = false,
                    CreateNoWindow = true,
                    RedirectStandardOutput = true,
                    RedirectStandardError = true
                };

                using var proc = System.Diagnostics.Process.Start(psi);
                proc?.WaitForExit(60000); // 60s timeout for biometric prompt
                return proc?.ExitCode == 0;
            }
            catch (Exception ex)
            {
                _log.Error(ex, "Windows Hello verification failed");
                return false;
            }
        });
    }

    private static bool CredFreeHelper(IntPtr cred)
    {
        CredFree(cred);
        return true;
    }

    // --- P/Invoke: Credential Manager ---

    [StructLayout(LayoutKind.Sequential, CharSet = CharSet.Unicode)]
    private struct CREDENTIAL
    {
        public uint Flags;
        public uint Type;
        public string TargetName;
        public string Comment;
        public long LastWritten;
        public int CredentialBlobSize;
        public IntPtr CredentialBlob;
        public uint Persist;
        public uint AttributeCount;
        public IntPtr Attributes;
        public string TargetAlias;
        public string UserName;
    }

    [DllImport("advapi32.dll", SetLastError = true, CharSet = CharSet.Unicode)]
    private static extern bool CredWrite(ref CREDENTIAL credential, uint flags);

    [DllImport("advapi32.dll", SetLastError = true, CharSet = CharSet.Unicode)]
    private static extern bool CredRead(string target, uint type, uint flags, out IntPtr credential);

    [DllImport("advapi32.dll", SetLastError = true, CharSet = CharSet.Unicode)]
    private static extern bool CredDelete(string target, uint type, uint flags);

    [DllImport("advapi32.dll")]
    private static extern void CredFree(IntPtr credential);
}
