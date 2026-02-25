using System.Runtime.InteropServices;
using System.Text;
using Serilog;

namespace PrivStack.Desktop.Services.Biometric;

/// <summary>
/// macOS biometric service using Touch ID via LocalAuthentication.framework
/// and Keychain via Security.framework for secure password storage.
/// </summary>
public class MacBiometricService : IBiometricService
{
    private static readonly ILogger _log = Log.ForContext<MacBiometricService>();

    private const string KeychainService = "com.privstack.desktop";
    private const string KeychainAccount = "biometric_master";

    public bool IsSupported => true;
    public string BiometricDisplayName => "Touch ID";

    public bool IsEnrolled
    {
        get
        {
            var existing = SecItemCopyMatching();
            return existing != null;
        }
    }

    public async Task<bool> IsAvailableAsync()
    {
        return await Task.Run(() =>
        {
            var context = objc_msgSend_ReturnIntPtr(
                objc_msgSend_ReturnIntPtr(objc_getClass("LAContext"), sel_registerName("alloc")),
                sel_registerName("init"));

            if (context == IntPtr.Zero) return false;

            try
            {
                var errorPtr = IntPtr.Zero;
                // LAPolicyDeviceOwnerAuthenticationWithBiometrics = 1
                var canEvaluate = objc_msgSend_ReturnBool_IntInt(
                    context, sel_registerName("canEvaluatePolicy:error:"), 1, ref errorPtr);
                return canEvaluate;
            }
            catch (Exception ex)
            {
                _log.Warning(ex, "Failed to check biometric availability");
                return false;
            }
            finally
            {
                objc_msgSend_Void(context, sel_registerName("release"));
            }
        });
    }

    public async Task<bool> EnrollAsync(string masterPassword)
    {
        return await Task.Run(() =>
        {
            try
            {
                // Remove any existing entry first
                SecItemDelete();

                var passwordBytes = Encoding.UTF8.GetBytes(masterPassword);
                var passwordData = CFDataCreate(IntPtr.Zero, passwordBytes, passwordBytes.Length);

                var serviceData = CFStringCreateWithCString(KeychainService);
                var accountData = CFStringCreateWithCString(KeychainAccount);

                // Create access control with biometry
                var accessControl = SecAccessControlCreateWithFlags(
                    IntPtr.Zero,
                    kSecAttrAccessibleWhenPasscodeSetThisDeviceOnly,
                    2, // kSecAccessControlBiometryCurrentSet
                    IntPtr.Zero);

                if (accessControl == IntPtr.Zero)
                {
                    _log.Error("Failed to create SecAccessControl for biometric");
                    return false;
                }

                var keys = new[]
                {
                    kSecClass, kSecAttrService, kSecAttrAccount,
                    kSecValueData, kSecAttrAccessControl
                };
                var values = new[]
                {
                    kSecClassGenericPassword, serviceData, accountData,
                    passwordData, accessControl
                };

                var query = CFDictionaryCreate(keys, values, keys.Length);
                var status = SecItemAdd(query, IntPtr.Zero);

                CFRelease(query);
                CFRelease(passwordData);
                CFRelease(serviceData);
                CFRelease(accountData);
                CFRelease(accessControl);

                if (status == 0) // errSecSuccess
                {
                    _log.Information("Biometric enrollment successful");
                    return true;
                }

                _log.Error("SecItemAdd failed with status {Status}", status);
                return false;
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
        return await Task.Run(() =>
        {
            try
            {
                var serviceData = CFStringCreateWithCString(KeychainService);
                var accountData = CFStringCreateWithCString(KeychainAccount);
                var promptData = CFStringCreateWithCString(reason);

                var keys = new[]
                {
                    kSecClass, kSecAttrService, kSecAttrAccount,
                    kSecReturnData, kSecUseOperationPrompt
                };
                var values = new[]
                {
                    kSecClassGenericPassword, serviceData, accountData,
                    kCFBooleanTrue, promptData
                };

                var query = CFDictionaryCreate(keys, values, keys.Length);
                var status = SecItemCopyMatching(query, out var resultData);

                CFRelease(query);
                CFRelease(serviceData);
                CFRelease(accountData);
                CFRelease(promptData);

                if (status == 0 && resultData != IntPtr.Zero) // errSecSuccess
                {
                    var length = CFDataGetLength(resultData);
                    var bytes = new byte[length];
                    Marshal.Copy(CFDataGetBytePtr(resultData), bytes, 0, length);
                    CFRelease(resultData);

                    var password = Encoding.UTF8.GetString(bytes);
                    Array.Clear(bytes);
                    _log.Information("Biometric authentication successful");
                    return password;
                }

                if (status == -128) // errSecUserCanceled
                    _log.Debug("Biometric authentication cancelled by user");
                else
                    _log.Warning("SecItemCopyMatching failed with status {Status}", status);

                return null;
            }
            catch (Exception ex)
            {
                _log.Error(ex, "Biometric authentication failed");
                return null;
            }
        });
    }

    public void Unenroll()
    {
        SecItemDelete();
        _log.Information("Biometric enrollment removed");
    }

    // --- Private helpers ---

    private string? SecItemCopyMatching()
    {
        try
        {
            var serviceData = CFStringCreateWithCString(KeychainService);
            var accountData = CFStringCreateWithCString(KeychainAccount);

            // Query without biometric prompt — just check existence via kSecUseAuthenticationUI = kSecUseAuthenticationUISkip
            var keys = new[]
            {
                kSecClass, kSecAttrService, kSecAttrAccount,
                kSecUseAuthenticationUI
            };
            var values = new[]
            {
                kSecClassGenericPassword, serviceData, accountData,
                kSecUseAuthenticationUISkip
            };

            var query = CFDictionaryCreate(keys, values, keys.Length);
            var status = SecItemCopyMatching(query, out _);

            CFRelease(query);
            CFRelease(serviceData);
            CFRelease(accountData);

            // -25293 = errSecInteractionNotAllowed (item exists but needs auth)
            // 0 = errSecSuccess
            return (status == 0 || status == -25293) ? "enrolled" : null;
        }
        catch
        {
            return null;
        }
    }

    private void SecItemDelete()
    {
        try
        {
            var serviceData = CFStringCreateWithCString(KeychainService);
            var accountData = CFStringCreateWithCString(KeychainAccount);

            var keys = new[] { kSecClass, kSecAttrService, kSecAttrAccount };
            var values = new[] { kSecClassGenericPassword, serviceData, accountData };

            var query = CFDictionaryCreate(keys, values, keys.Length);
            SecItemDelete(query);

            CFRelease(query);
            CFRelease(serviceData);
            CFRelease(accountData);
        }
        catch (Exception ex)
        {
            _log.Warning(ex, "Failed to delete keychain item");
        }
    }

    // --- P/Invoke: Security.framework ---

    [DllImport("/System/Library/Frameworks/Security.framework/Security")]
    private static extern int SecItemAdd(IntPtr attributes, IntPtr result);

    [DllImport("/System/Library/Frameworks/Security.framework/Security")]
    private static extern int SecItemCopyMatching(IntPtr query, out IntPtr result);

    [DllImport("/System/Library/Frameworks/Security.framework/Security")]
    private static extern int SecItemDelete(IntPtr query);

    [DllImport("/System/Library/Frameworks/Security.framework/Security")]
    private static extern IntPtr SecAccessControlCreateWithFlags(
        IntPtr allocator, IntPtr protection, long flags, IntPtr error);

    // --- P/Invoke: CoreFoundation ---

    [DllImport("/System/Library/Frameworks/CoreFoundation.framework/CoreFoundation")]
    private static extern IntPtr CFDataCreate(IntPtr allocator, byte[] bytes, int length);

    [DllImport("/System/Library/Frameworks/CoreFoundation.framework/CoreFoundation")]
    private static extern int CFDataGetLength(IntPtr data);

    [DllImport("/System/Library/Frameworks/CoreFoundation.framework/CoreFoundation")]
    private static extern IntPtr CFDataGetBytePtr(IntPtr data);

    [DllImport("/System/Library/Frameworks/CoreFoundation.framework/CoreFoundation")]
    private static extern void CFRelease(IntPtr obj);

    [DllImport("/System/Library/Frameworks/CoreFoundation.framework/CoreFoundation")]
    private static extern IntPtr CFDictionaryCreate(
        IntPtr allocator, IntPtr[] keys, IntPtr[] values, int count,
        IntPtr keyCallbacks, IntPtr valueCallbacks);

    private static IntPtr CFDictionaryCreate(IntPtr[] keys, IntPtr[] values, int count)
    {
        return CFDictionaryCreate(IntPtr.Zero, keys, values, count,
            kCFTypeDictionaryKeyCallBacks, kCFTypeDictionaryValueCallBacks);
    }

    // --- P/Invoke: ObjC Runtime (for LAContext) ---

    [DllImport("/usr/lib/libobjc.A.dylib", EntryPoint = "objc_getClass")]
    private static extern IntPtr objc_getClass(string className);

    [DllImport("/usr/lib/libobjc.A.dylib", EntryPoint = "sel_registerName")]
    private static extern IntPtr sel_registerName(string selectorName);

    [DllImport("/usr/lib/libobjc.A.dylib", EntryPoint = "objc_msgSend")]
    private static extern IntPtr objc_msgSend_ReturnIntPtr(IntPtr target, IntPtr selector);

    [DllImport("/usr/lib/libobjc.A.dylib", EntryPoint = "objc_msgSend")]
    private static extern bool objc_msgSend_ReturnBool_IntInt(
        IntPtr target, IntPtr selector, long arg1, ref IntPtr arg2);

    [DllImport("/usr/lib/libobjc.A.dylib", EntryPoint = "objc_msgSend")]
    private static extern void objc_msgSend_Void(IntPtr target, IntPtr selector);

    // --- Security framework constants (loaded at runtime) ---

    private static readonly IntPtr _securityLib = NativeLibrary.Load("/System/Library/Frameworks/Security.framework/Security");
    private static readonly IntPtr _cfLib = NativeLibrary.Load("/System/Library/Frameworks/CoreFoundation.framework/CoreFoundation");

    private static IntPtr GetSecurityConstant(string name) =>
        Marshal.ReadIntPtr(NativeLibrary.GetExport(_securityLib, name));

    private static IntPtr GetCFConstant(string name) =>
        Marshal.ReadIntPtr(NativeLibrary.GetExport(_cfLib, name));

    private static readonly IntPtr kSecClass = GetSecurityConstant("kSecClass");
    private static readonly IntPtr kSecClassGenericPassword = GetSecurityConstant("kSecClassGenericPassword");
    private static readonly IntPtr kSecAttrService = GetSecurityConstant("kSecAttrService");
    private static readonly IntPtr kSecAttrAccount = GetSecurityConstant("kSecAttrAccount");
    private static readonly IntPtr kSecValueData = GetSecurityConstant("kSecValueData");
    private static readonly IntPtr kSecReturnData = GetSecurityConstant("kSecReturnData");
    private static readonly IntPtr kSecAttrAccessControl = GetSecurityConstant("kSecAttrAccessControl");
    private static readonly IntPtr kSecAttrAccessibleWhenPasscodeSetThisDeviceOnly =
        GetSecurityConstant("kSecAttrAccessibleWhenPasscodeSetThisDeviceOnly");
    private static readonly IntPtr kSecUseOperationPrompt = GetSecurityConstant("kSecUseOperationPrompt");
    private static readonly IntPtr kSecUseAuthenticationUI = GetSecurityConstant("kSecUseAuthenticationUI");
    private static readonly IntPtr kSecUseAuthenticationUISkip = GetSecurityConstant("kSecUseAuthenticationUISkip");

    private static readonly IntPtr kCFBooleanTrue = GetCFConstant("kCFBooleanTrue");
    private static readonly IntPtr kCFTypeDictionaryKeyCallBacks = NativeLibrary.GetExport(_cfLib, "kCFTypeDictionaryKeyCallBacks");
    private static readonly IntPtr kCFTypeDictionaryValueCallBacks = NativeLibrary.GetExport(_cfLib, "kCFTypeDictionaryValueCallBacks");

    private static IntPtr CFStringCreateWithCString(string value)
    {
        return CFStringCreateWithCString(IntPtr.Zero, value, 0x08000100); // kCFStringEncodingUTF8
    }

    [DllImport("/System/Library/Frameworks/CoreFoundation.framework/CoreFoundation")]
    private static extern IntPtr CFStringCreateWithCString(IntPtr allocator, string value, uint encoding);
}
