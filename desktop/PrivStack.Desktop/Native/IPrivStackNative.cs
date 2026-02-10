namespace PrivStack.Desktop.Native;

/// <summary>
/// Composite interface over the PrivStack native FFI boundary.
/// Consumers should prefer the focused interfaces (IAuthService, ISyncService, etc.)
/// for better testability. This interface exists for callers that need multiple capabilities.
/// </summary>
public interface IPrivStackNative :
    IPrivStackRuntime,
    IAuthService,
    ISyncService,
    IPairingService,
    ICloudStorageService,
    ILicensingService
{
}
