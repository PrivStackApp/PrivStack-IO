using System.Diagnostics;
using System.Runtime.InteropServices;
using System.Text;

namespace PrivStack.Server;

/// <summary>
/// Non-biometric OS credential storage. No Touch ID / Windows Hello — just the system keychain.
/// </summary>
public interface IKeyringService
{
    bool IsAvailable { get; }
    bool Store(string service, string account, string secret);
    string? Retrieve(string service, string account);
    void Delete(string service, string account);
}

/// <summary>
/// Factory that returns the correct keyring implementation for the current OS.
/// </summary>
internal static class KeyringServiceFactory
{
    public static IKeyringService Create()
    {
        if (RuntimeInformation.IsOSPlatform(OSPlatform.OSX))
            return new MacKeyringService();
        if (RuntimeInformation.IsOSPlatform(OSPlatform.Windows))
            return new WindowsKeyringService();
        if (RuntimeInformation.IsOSPlatform(OSPlatform.Linux))
            return new LinuxKeyringService();
        return new NullKeyringService();
    }
}

/// <summary>
/// macOS Keychain via Security framework P/Invoke.
/// Uses legacy generic password API (no biometric ACL).
/// </summary>
internal sealed class MacKeyringService : IKeyringService
{
    public bool IsAvailable => RuntimeInformation.IsOSPlatform(OSPlatform.OSX);

    public bool Store(string service, string account, string secret)
    {
        try
        {
            // Delete existing first (SecKeychainAddGenericPassword fails on duplicate)
            Delete(service, account);

            var serviceBytes = Encoding.UTF8.GetBytes(service);
            var accountBytes = Encoding.UTF8.GetBytes(account);
            var secretBytes = Encoding.UTF8.GetBytes(secret);

            var status = SecKeychainAddGenericPassword(
                IntPtr.Zero, // default keychain
                (uint)serviceBytes.Length, serviceBytes,
                (uint)accountBytes.Length, accountBytes,
                (uint)secretBytes.Length, secretBytes,
                IntPtr.Zero);

            return status == 0; // errSecSuccess
        }
        catch
        {
            return false;
        }
    }

    public string? Retrieve(string service, string account)
    {
        try
        {
            var serviceBytes = Encoding.UTF8.GetBytes(service);
            var accountBytes = Encoding.UTF8.GetBytes(account);

            var status = SecKeychainFindGenericPassword(
                IntPtr.Zero,
                (uint)serviceBytes.Length, serviceBytes,
                (uint)accountBytes.Length, accountBytes,
                out var passwordLength, out var passwordData,
                IntPtr.Zero);

            if (status != 0 || passwordData == IntPtr.Zero)
                return null;

            try
            {
                var bytes = new byte[passwordLength];
                Marshal.Copy(passwordData, bytes, 0, (int)passwordLength);
                return Encoding.UTF8.GetString(bytes);
            }
            finally
            {
                SecKeychainItemFreeContent(IntPtr.Zero, passwordData);
            }
        }
        catch
        {
            return null;
        }
    }

    public void Delete(string service, string account)
    {
        try
        {
            var serviceBytes = Encoding.UTF8.GetBytes(service);
            var accountBytes = Encoding.UTF8.GetBytes(account);

            var status = SecKeychainFindGenericPassword(
                IntPtr.Zero,
                (uint)serviceBytes.Length, serviceBytes,
                (uint)accountBytes.Length, accountBytes,
                out _, out _,
                out var itemRef);

            if (status == 0 && itemRef != IntPtr.Zero)
            {
                SecKeychainItemDelete(itemRef);
                CFRelease(itemRef);
            }
        }
        catch { /* ignore deletion errors */ }
    }

    [DllImport("/System/Library/Frameworks/Security.framework/Security")]
    private static extern int SecKeychainAddGenericPassword(
        IntPtr keychain,
        uint serviceNameLength, byte[] serviceName,
        uint accountNameLength, byte[] accountName,
        uint passwordLength, byte[] passwordData,
        IntPtr itemRef);

    [DllImport("/System/Library/Frameworks/Security.framework/Security")]
    private static extern int SecKeychainFindGenericPassword(
        IntPtr keychain,
        uint serviceNameLength, byte[] serviceName,
        uint accountNameLength, byte[] accountName,
        out uint passwordLength, out IntPtr passwordData,
        out IntPtr itemRef);

    [DllImport("/System/Library/Frameworks/Security.framework/Security")]
    private static extern int SecKeychainFindGenericPassword(
        IntPtr keychain,
        uint serviceNameLength, byte[] serviceName,
        uint accountNameLength, byte[] accountName,
        out uint passwordLength, out IntPtr passwordData,
        IntPtr itemRef);

    [DllImport("/System/Library/Frameworks/Security.framework/Security")]
    private static extern int SecKeychainItemFreeContent(IntPtr attrList, IntPtr data);

    [DllImport("/System/Library/Frameworks/Security.framework/Security")]
    private static extern int SecKeychainItemDelete(IntPtr itemRef);

    [DllImport("/System/Library/Frameworks/CoreFoundation.framework/CoreFoundation")]
    private static extern void CFRelease(IntPtr cf);
}

/// <summary>
/// Windows Credential Manager via P/Invoke.
/// </summary>
internal sealed class WindowsKeyringService : IKeyringService
{
    public bool IsAvailable => RuntimeInformation.IsOSPlatform(OSPlatform.Windows);

    public bool Store(string service, string account, string secret)
    {
        if (!IsAvailable) return false;

        try
        {
            var target = $"{service}:{account}";
            var secretBytes = Encoding.UTF8.GetBytes(secret);

            var cred = new CREDENTIAL
            {
                Type = 1, // CRED_TYPE_GENERIC
                TargetName = target,
                CredentialBlobSize = (uint)secretBytes.Length,
                Persist = 2, // CRED_PERSIST_LOCAL_MACHINE
                UserName = account,
            };

            var credBlobPtr = Marshal.AllocHGlobal(secretBytes.Length);
            try
            {
                Marshal.Copy(secretBytes, 0, credBlobPtr, secretBytes.Length);
                cred.CredentialBlob = credBlobPtr;
                return CredWrite(ref cred, 0);
            }
            finally
            {
                Marshal.FreeHGlobal(credBlobPtr);
            }
        }
        catch
        {
            return false;
        }
    }

    public string? Retrieve(string service, string account)
    {
        if (!IsAvailable) return null;

        try
        {
            var target = $"{service}:{account}";
            if (!CredRead(target, 1, 0, out var credPtr) || credPtr == IntPtr.Zero)
                return null;

            try
            {
                var cred = Marshal.PtrToStructure<CREDENTIAL>(credPtr);
                if (cred.CredentialBlob == IntPtr.Zero || cred.CredentialBlobSize == 0)
                    return null;

                var bytes = new byte[cred.CredentialBlobSize];
                Marshal.Copy(cred.CredentialBlob, bytes, 0, (int)cred.CredentialBlobSize);
                return Encoding.UTF8.GetString(bytes);
            }
            finally
            {
                CredFree(credPtr);
            }
        }
        catch
        {
            return null;
        }
    }

    public void Delete(string service, string account)
    {
        if (!IsAvailable) return;
        try
        {
            CredDelete($"{service}:{account}", 1, 0);
        }
        catch { }
    }

    [StructLayout(LayoutKind.Sequential, CharSet = CharSet.Unicode)]
    private struct CREDENTIAL
    {
        public uint Flags;
        public uint Type;
        [MarshalAs(UnmanagedType.LPWStr)] public string TargetName;
        [MarshalAs(UnmanagedType.LPWStr)] public string? Comment;
        public long LastWritten;
        public uint CredentialBlobSize;
        public IntPtr CredentialBlob;
        public uint Persist;
        public uint AttributeCount;
        public IntPtr Attributes;
        [MarshalAs(UnmanagedType.LPWStr)] public string? TargetAlias;
        [MarshalAs(UnmanagedType.LPWStr)] public string? UserName;
    }

    [DllImport("advapi32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
    private static extern bool CredWrite(ref CREDENTIAL credential, uint flags);

    [DllImport("advapi32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
    private static extern bool CredRead(string target, uint type, uint flags, out IntPtr credential);

    [DllImport("advapi32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
    private static extern bool CredDelete(string target, uint type, uint flags);

    [DllImport("advapi32.dll")]
    private static extern void CredFree(IntPtr buffer);
}

/// <summary>
/// Linux keyring via secret-tool (libsecret CLI).
/// </summary>
internal sealed class LinuxKeyringService : IKeyringService
{
    public bool IsAvailable
    {
        get
        {
            try
            {
                var psi = new ProcessStartInfo("which", "secret-tool")
                {
                    RedirectStandardOutput = true,
                    UseShellExecute = false,
                    CreateNoWindow = true,
                };
                using var proc = Process.Start(psi);
                proc?.WaitForExit(2000);
                return proc?.ExitCode == 0;
            }
            catch
            {
                return false;
            }
        }
    }

    public bool Store(string service, string account, string secret)
    {
        try
        {
            var psi = new ProcessStartInfo("secret-tool", $"store --label=\"PrivStack\" service {service} account {account}")
            {
                RedirectStandardInput = true,
                UseShellExecute = false,
                CreateNoWindow = true,
            };
            using var proc = Process.Start(psi);
            if (proc == null) return false;
            proc.StandardInput.Write(secret);
            proc.StandardInput.Close();
            proc.WaitForExit(5000);
            return proc.ExitCode == 0;
        }
        catch
        {
            return false;
        }
    }

    public string? Retrieve(string service, string account)
    {
        try
        {
            var psi = new ProcessStartInfo("secret-tool", $"lookup service {service} account {account}")
            {
                RedirectStandardOutput = true,
                UseShellExecute = false,
                CreateNoWindow = true,
            };
            using var proc = Process.Start(psi);
            if (proc == null) return null;
            var output = proc.StandardOutput.ReadToEnd().Trim();
            proc.WaitForExit(5000);
            return proc.ExitCode == 0 && !string.IsNullOrEmpty(output) ? output : null;
        }
        catch
        {
            return null;
        }
    }

    public void Delete(string service, string account)
    {
        try
        {
            var psi = new ProcessStartInfo("secret-tool", $"clear service {service} account {account}")
            {
                UseShellExecute = false,
                CreateNoWindow = true,
            };
            using var proc = Process.Start(psi);
            proc?.WaitForExit(5000);
        }
        catch { }
    }
}

/// <summary>
/// No-op keyring for unsupported platforms.
/// </summary>
internal sealed class NullKeyringService : IKeyringService
{
    public bool IsAvailable => false;
    public bool Store(string service, string account, string secret) => false;
    public string? Retrieve(string service, string account) => null;
    public void Delete(string service, string account) { }
}
