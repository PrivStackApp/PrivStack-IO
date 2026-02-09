using PrivStack.Desktop.Models;

namespace PrivStack.Desktop.Native;

/// <summary>
/// Manages cloud storage operations (Google Drive, iCloud).
/// </summary>
public interface ICloudStorageService
{
    void InitGoogleDrive(string clientId, string clientSecret);
    void InitICloud(string? bundleId = null);
    string? CloudAuthenticate(CloudProvider provider);
    void CloudCompleteAuth(CloudProvider provider, string authCode);
    bool IsCloudAuthenticated(CloudProvider provider);
    string ListCloudFiles(CloudProvider provider);
    List<T> ListCloudFiles<T>(CloudProvider provider);
    string CloudUpload(CloudProvider provider, string name, byte[] data);
    T CloudUpload<T>(CloudProvider provider, string name, byte[] data);
    byte[] CloudDownload(CloudProvider provider, string fileId);
    void CloudDelete(CloudProvider provider, string fileId);
}
