using System.Runtime.InteropServices;
using System.Text;
using Serilog;

namespace PrivStack.Desktop.Services.Biometric;

/// <summary>
/// macOS biometric service using Touch ID via LocalAuthentication.framework
/// and Keychain via Security.framework for secure password storage.
///
/// Debug builds use the legacy file-based Keychain API (SecKeychainAddGenericPassword)
/// which works without entitlements for unsigned dev builds.
/// Release builds use the modern SecItem API (SecItemAdd) with biometric access control,
/// which requires keychain-access-groups entitlement on signed/notarized builds.
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
#if DEBUG
            return LegacyKeychainExists();
#else
            return ModernKeychainExists();
#endif
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
#if DEBUG
        return await LegacyEnrollAsync(masterPassword);
#else
        return await ModernEnrollAsync(masterPassword);
#endif
    }

    public async Task<string?> AuthenticateAsync(string reason)
    {
#if DEBUG
        return await LegacyAuthenticateAsync(reason);
#else
        return await ModernAuthenticateAsync(reason);
#endif
    }

    public async Task<bool> VerifyBiometricAsync(string reason)
    {
        _log.Debug("VerifyBiometricAsync called with reason: {Reason}", reason);
        try
        {
            var result = await EvaluateBiometricPolicyAsync(reason);
            _log.Information("Biometric verification result: {Result}", result);
            return result;
        }
        catch (Exception ex)
        {
            _log.Error(ex, "Biometric verification failed");
            return false;
        }
    }

    public void Unenroll()
    {
#if DEBUG
        LegacyKeychainDelete();
#else
        ModernKeychainDelete();
#endif
        _log.Information("Biometric enrollment removed");
    }

    // =====================================================================
    // DEBUG: Legacy file-based Keychain API (no entitlements required)
    // =====================================================================

#if DEBUG

    private bool LegacyKeychainExists()
    {
        try
        {
            var serviceBytes = Encoding.UTF8.GetBytes(KeychainService);
            var accountBytes = Encoding.UTF8.GetBytes(KeychainAccount);

            var status = SecKeychainFindGenericPassword(
                IntPtr.Zero,
                serviceBytes.Length, serviceBytes,
                accountBytes.Length, accountBytes,
                out _, out _,
                out var itemRef);

            if (itemRef != IntPtr.Zero)
                CFRelease(itemRef);

            return status == 0; // errSecSuccess
        }
        catch
        {
            return false;
        }
    }

    private async Task<bool> LegacyEnrollAsync(string masterPassword)
    {
        return await Task.Run(() =>
        {
            try
            {
                // Remove any existing entry first
                LegacyKeychainDelete();

                var serviceBytes = Encoding.UTF8.GetBytes(KeychainService);
                var accountBytes = Encoding.UTF8.GetBytes(KeychainAccount);
                var passwordBytes = Encoding.UTF8.GetBytes(masterPassword);

                var status = SecKeychainAddGenericPassword(
                    IntPtr.Zero,
                    serviceBytes.Length, serviceBytes,
                    accountBytes.Length, accountBytes,
                    passwordBytes.Length, passwordBytes,
                    out var itemRef);

                if (itemRef != IntPtr.Zero)
                    CFRelease(itemRef);

                Array.Clear(passwordBytes);

                if (status == 0)
                {
                    _log.Information("Biometric enrollment successful (legacy keychain)");
                    return true;
                }

                _log.Error("SecKeychainAddGenericPassword failed with status {Status}", status);
                return false;
            }
            catch (Exception ex)
            {
                _log.Error(ex, "Biometric enrollment failed (legacy keychain)");
                return false;
            }
        });
    }

    private async Task<string?> LegacyAuthenticateAsync(string reason)
    {
        // First verify biometric via LAContext
        var biometricOk = await EvaluateBiometricPolicyAsync(reason);
        if (!biometricOk) return null;

        // Then read password from legacy keychain
        return await Task.Run(() =>
        {
            try
            {
                var serviceBytes = Encoding.UTF8.GetBytes(KeychainService);
                var accountBytes = Encoding.UTF8.GetBytes(KeychainAccount);

                var status = SecKeychainFindGenericPassword(
                    IntPtr.Zero,
                    serviceBytes.Length, serviceBytes,
                    accountBytes.Length, accountBytes,
                    out var passwordLength, out var passwordData,
                    out var itemRef);

                if (itemRef != IntPtr.Zero)
                    CFRelease(itemRef);

                if (status != 0 || passwordData == IntPtr.Zero)
                {
                    _log.Warning("SecKeychainFindGenericPassword failed with status {Status}", status);
                    return null;
                }

                var bytes = new byte[passwordLength];
                Marshal.Copy(passwordData, bytes, 0, passwordLength);
                SecKeychainItemFreeContent(IntPtr.Zero, passwordData);

                var password = Encoding.UTF8.GetString(bytes);
                Array.Clear(bytes);

                _log.Information("Biometric authentication successful (legacy keychain)");
                return password;
            }
            catch (Exception ex)
            {
                _log.Error(ex, "Biometric authentication failed (legacy keychain)");
                return null;
            }
        });
    }

    private void LegacyKeychainDelete()
    {
        try
        {
            var serviceBytes = Encoding.UTF8.GetBytes(KeychainService);
            var accountBytes = Encoding.UTF8.GetBytes(KeychainAccount);

            var status = SecKeychainFindGenericPassword(
                IntPtr.Zero,
                serviceBytes.Length, serviceBytes,
                accountBytes.Length, accountBytes,
                out _, out _,
                out var itemRef);

            if (status == 0 && itemRef != IntPtr.Zero)
            {
                SecKeychainItemDelete(itemRef);
                CFRelease(itemRef);
            }
        }
        catch (Exception ex)
        {
            _log.Warning(ex, "Failed to delete legacy keychain item");
        }
    }

    // --- Legacy Keychain P/Invoke ---

    [DllImport("/System/Library/Frameworks/Security.framework/Security")]
    private static extern int SecKeychainAddGenericPassword(
        IntPtr keychain,
        int serviceNameLength, byte[] serviceName,
        int accountNameLength, byte[] accountName,
        int passwordLength, byte[] passwordData,
        out IntPtr itemRef);

    [DllImport("/System/Library/Frameworks/Security.framework/Security")]
    private static extern int SecKeychainFindGenericPassword(
        IntPtr keychain,
        int serviceNameLength, byte[] serviceName,
        int accountNameLength, byte[] accountName,
        out int passwordLength, out IntPtr passwordData,
        out IntPtr itemRef);

    [DllImport("/System/Library/Frameworks/Security.framework/Security")]
    private static extern int SecKeychainItemDelete(IntPtr itemRef);

    [DllImport("/System/Library/Frameworks/Security.framework/Security")]
    private static extern int SecKeychainItemFreeContent(IntPtr attrList, IntPtr data);

#endif

    // =====================================================================
    // RELEASE: Modern SecItem API (requires keychain entitlement)
    // =====================================================================

#if !DEBUG

    private bool ModernKeychainExists()
    {
        try
        {
            var serviceData = CFStringCreateWithCString(KeychainService);
            var accountData = CFStringCreateWithCString(KeychainAccount);

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
            return status == 0 || status == -25293;
        }
        catch
        {
            return false;
        }
    }

    private async Task<bool> ModernEnrollAsync(string masterPassword)
    {
        return await Task.Run(() =>
        {
            try
            {
                ModernKeychainDelete();

                var passwordBytes = Encoding.UTF8.GetBytes(masterPassword);
                var passwordData = CFDataCreate(IntPtr.Zero, passwordBytes, passwordBytes.Length);
                Array.Clear(passwordBytes);

                var serviceData = CFStringCreateWithCString(KeychainService);
                var accountData = CFStringCreateWithCString(KeychainAccount);

                var accessControl = SecAccessControlCreateWithFlags(
                    IntPtr.Zero,
                    kSecAttrAccessibleWhenPasscodeSetThisDeviceOnly,
                    2, // kSecAccessControlBiometryCurrentSet
                    IntPtr.Zero);

                if (accessControl == IntPtr.Zero)
                {
                    _log.Error("Failed to create SecAccessControl for biometric");
                    CFRelease(passwordData);
                    CFRelease(serviceData);
                    CFRelease(accountData);
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

                if (status == 0)
                {
                    _log.Information("Biometric enrollment successful (modern SecItem)");
                    return true;
                }

                _log.Error("SecItemAdd failed with status {Status}", status);
                return false;
            }
            catch (Exception ex)
            {
                _log.Error(ex, "Biometric enrollment failed (modern SecItem)");
                return false;
            }
        });
    }

    private async Task<string?> ModernAuthenticateAsync(string reason)
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

                if (status == 0 && resultData != IntPtr.Zero)
                {
                    var length = CFDataGetLength(resultData);
                    var bytes = new byte[length];
                    Marshal.Copy(CFDataGetBytePtr(resultData), bytes, 0, length);
                    CFRelease(resultData);

                    var password = Encoding.UTF8.GetString(bytes);
                    Array.Clear(bytes);
                    _log.Information("Biometric authentication successful (modern SecItem)");
                    return password;
                }

                if (status == -128)
                    _log.Debug("Biometric authentication cancelled by user");
                else
                    _log.Warning("SecItemCopyMatching failed with status {Status}", status);

                return null;
            }
            catch (Exception ex)
            {
                _log.Error(ex, "Biometric authentication failed (modern SecItem)");
                return null;
            }
        });
    }

    private void ModernKeychainDelete()
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
            _log.Warning(ex, "Failed to delete modern keychain item");
        }
    }

    // --- Modern SecItem P/Invoke ---

    [DllImport("/System/Library/Frameworks/Security.framework/Security")]
    private static extern int SecItemAdd(IntPtr attributes, IntPtr result);

    [DllImport("/System/Library/Frameworks/Security.framework/Security")]
    private static extern int SecItemCopyMatching(IntPtr query, out IntPtr result);

    [DllImport("/System/Library/Frameworks/Security.framework/Security")]
    private static extern int SecItemDelete(IntPtr query);

    [DllImport("/System/Library/Frameworks/Security.framework/Security")]
    private static extern IntPtr SecAccessControlCreateWithFlags(
        IntPtr allocator, IntPtr protection, long flags, IntPtr error);

    // --- Modern SecItem constants ---

    private static readonly IntPtr kSecAttrAccessControl = GetSecurityConstant("kSecAttrAccessControl");
    private static readonly IntPtr kSecAttrAccessibleWhenPasscodeSetThisDeviceOnly =
        GetSecurityConstant("kSecAttrAccessibleWhenPasscodeSetThisDeviceOnly");
    private static readonly IntPtr kSecReturnData = GetSecurityConstant("kSecReturnData");
    private static readonly IntPtr kSecValueData = GetSecurityConstant("kSecValueData");
    private static readonly IntPtr kSecUseOperationPrompt = GetSecurityConstant("kSecUseOperationPrompt");
    private static readonly IntPtr kSecUseAuthenticationUI = GetSecurityConstant("kSecUseAuthenticationUI");
    private static readonly IntPtr kSecUseAuthenticationUISkip = GetSecurityConstant("kSecUseAuthenticationUISkip");
    private static readonly IntPtr kCFBooleanTrue = GetCFConstant("kCFBooleanTrue");

#endif

    // =====================================================================
    // Shared: CoreFoundation + ObjC Runtime + Security constants
    // =====================================================================

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

    private static readonly IntPtr kCFTypeDictionaryKeyCallBacks = NativeLibrary.GetExport(_cfLib, "kCFTypeDictionaryKeyCallBacks");
    private static readonly IntPtr kCFTypeDictionaryValueCallBacks = NativeLibrary.GetExport(_cfLib, "kCFTypeDictionaryValueCallBacks");

    // --- CoreFoundation ---

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

    [DllImport("/System/Library/Frameworks/CoreFoundation.framework/CoreFoundation")]
    private static extern IntPtr CFStringCreateWithCString(IntPtr allocator, string value, uint encoding);

    private static IntPtr CFStringCreateWithCString(string value) =>
        CFStringCreateWithCString(IntPtr.Zero, value, 0x08000100); // kCFStringEncodingUTF8

    // --- ObjC Runtime ---

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

    private static IntPtr CreateNSString(string value)
    {
        var cls = objc_getClass("NSString");
        var alloc = objc_msgSend_ReturnIntPtr(cls, sel_registerName("alloc"));
        var utf8 = Encoding.UTF8.GetBytes(value + '\0');
        var handle = GCHandle.Alloc(utf8, GCHandleType.Pinned);
        try
        {
            return objc_msgSend_InitWithUTF8(alloc, sel_registerName("initWithUTF8String:"), handle.AddrOfPinnedObject());
        }
        finally
        {
            handle.Free();
        }
    }

    [DllImport("/usr/lib/libobjc.A.dylib", EntryPoint = "objc_msgSend")]
    private static extern IntPtr objc_msgSend_InitWithUTF8(IntPtr target, IntPtr selector, IntPtr utf8Str);

    // =====================================================================
    // Touch ID evaluation (shared across DEBUG/RELEASE)
    // =====================================================================

    /// <summary>
    /// Evaluates Touch ID via LAContext.evaluatePolicy:localizedReason:reply:
    /// Uses a ManualResetEventSlim to bridge the async ObjC callback.
    /// </summary>
    private static async Task<bool> EvaluateBiometricPolicyAsync(string reason)
    {
        return await Task.Run(() =>
        {
            var context = objc_msgSend_ReturnIntPtr(
                objc_msgSend_ReturnIntPtr(objc_getClass("LAContext"), sel_registerName("alloc")),
                sel_registerName("init"));

            if (context == IntPtr.Zero) return false;

            Action? cleanup = null;
            try
            {
                var reasonNs = CreateNSString(reason);
                var gate = new ManualResetEventSlim(false);
                var success = false;

                // Create block for reply:(BOOL success, NSError *error)
                // EvaluatePolicy returns a cleanup action we must call after the gate signals.
                cleanup = BlockLiteral.EvaluatePolicy(context, 1, reasonNs, (ok, _) =>
                {
                    success = ok;
                    gate.Set();
                });

                var signaled = gate.Wait(TimeSpan.FromSeconds(60));

                CFRelease(reasonNs);
                return signaled && success;
            }
            finally
            {
                cleanup?.Invoke();
                objc_msgSend_Void(context, sel_registerName("release"));
            }
        });
    }

    // =====================================================================
    // LAContext block-based evaluatePolicy bridging
    // =====================================================================

    /// <summary>
    /// Bridges ObjC block callbacks for LAContext.evaluatePolicy:localizedReason:reply:
    /// The block is heap-allocated with _NSConcreteGlobalBlock (no copy/dispose needed)
    /// and cleaned up after the caller's gate is signaled.
    /// </summary>
    private static class BlockLiteral
    {
        // reply block signature: void (^)(BOOL success, NSError *error)
        // On ARM64, BOOL is a single byte.
        private delegate void BlockInvokeDelegate(IntPtr block, byte success, IntPtr error);

        [StructLayout(LayoutKind.Sequential)]
        private struct Block
        {
            public IntPtr Isa;
            public int Flags;
            public int Reserved;
            public IntPtr Invoke;
            public IntPtr Descriptor;
            public IntPtr Context; // GCHandle to our Action callback
        }

        [StructLayout(LayoutKind.Sequential)]
        private struct BlockDescriptor
        {
            public ulong Reserved;
            public ulong Size;
        }

        private static readonly IntPtr _nsConcreteGlobalBlock;
        private static readonly IntPtr _descriptorPtr;

        // Pin the delegate to prevent GC — it's reused across all calls
        private static readonly BlockInvokeDelegate _invokerDelegate = InvokerImpl;
        private static readonly IntPtr _invokerFnPtr = Marshal.GetFunctionPointerForDelegate(_invokerDelegate);

        static BlockLiteral()
        {
            var objcLib = NativeLibrary.Load("/usr/lib/libobjc.A.dylib");
            _nsConcreteGlobalBlock = NativeLibrary.GetExport(objcLib, "_NSConcreteGlobalBlock");

            var desc = new BlockDescriptor
            {
                Reserved = 0,
                Size = (ulong)Marshal.SizeOf<Block>()
            };
            _descriptorPtr = Marshal.AllocHGlobal(Marshal.SizeOf<BlockDescriptor>());
            Marshal.StructureToPtr(desc, _descriptorPtr, false);
        }

        private static void InvokerImpl(IntPtr block, byte success, IntPtr error)
        {
            try
            {
                var blockStruct = Marshal.PtrToStructure<Block>(block);
                if (blockStruct.Context == IntPtr.Zero) return;
                var handle = GCHandle.FromIntPtr(blockStruct.Context);
                if (handle.Target is Action<bool, IntPtr> cb)
                    cb(success != 0, error);
            }
            catch
            {
                // Swallow — we're being called from native code
            }
        }

        /// <summary>
        /// Calls evaluatePolicy:localizedReason:reply: and returns a cleanup action.
        /// The caller MUST invoke the cleanup action after the gate is signaled.
        /// </summary>
        public static Action EvaluatePolicy(IntPtr laContext, long policy, IntPtr localizedReason, Action<bool, IntPtr> callback)
        {
            var callbackHandle = GCHandle.Alloc(callback);

            var blockData = new Block
            {
                Isa = _nsConcreteGlobalBlock,
                Flags = 1 << 28, // BLOCK_IS_GLOBAL — no copy/dispose
                Reserved = 0,
                Invoke = _invokerFnPtr,
                Descriptor = _descriptorPtr,
                Context = GCHandle.ToIntPtr(callbackHandle)
            };

            var blockPtr = Marshal.AllocHGlobal(Marshal.SizeOf<Block>());
            Marshal.StructureToPtr(blockData, blockPtr, false);

            objc_msgSend_EvaluatePolicy(laContext,
                sel_registerName("evaluatePolicy:localizedReason:reply:"),
                policy, localizedReason, blockPtr);

            // Return cleanup action — caller invokes after gate.Wait() completes
            return () =>
            {
                callbackHandle.Free();
                Marshal.FreeHGlobal(blockPtr);
            };
        }

        [DllImport("/usr/lib/libobjc.A.dylib", EntryPoint = "objc_msgSend")]
        private static extern void objc_msgSend_EvaluatePolicy(
            IntPtr target, IntPtr selector, long policy, IntPtr localizedReason, IntPtr reply);
    }
}
