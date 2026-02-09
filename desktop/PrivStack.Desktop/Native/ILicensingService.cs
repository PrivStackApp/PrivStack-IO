using PrivStack.Desktop.Models;

namespace PrivStack.Desktop.Native;

/// <summary>
/// Manages license key parsing, activation, and validation.
/// </summary>
public interface ILicensingService
{
    string ParseLicenseKey(string key);
    T ParseLicenseKey<T>(string key);
    LicensePlan GetLicensePlan(string key);
    string GetDeviceInfo();
    T GetDeviceInfo<T>();
    string GetDeviceFingerprint();
    string ActivateLicense(string key);
    T ActivateLicense<T>(string key);
    string? CheckLicense();
    T? CheckLicense<T>() where T : class;
    bool IsLicenseValid();
    LicenseStatus GetLicenseStatus();
    LicensePlan GetActivatedLicensePlan();
    void DeactivateLicense();
}
