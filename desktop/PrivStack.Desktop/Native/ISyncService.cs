namespace PrivStack.Desktop.Native;

/// <summary>
/// Manages P2P sync transport and sync event operations.
/// </summary>
public interface ISyncService
{
    void StartSync();
    void StopSync();
    bool IsSyncRunning();
    string GetSyncStatus();
    T GetSyncStatus<T>();
    string GetLocalPeerId();
    string GetDiscoveredPeers();
    List<T> GetDiscoveredPeers<T>();
    int GetPeerCount();
    void ShareDocumentForSync(string documentId);
    void TriggerDocumentSync(string documentId);
    string PollSyncEvents();
    List<T> PollSyncEvents<T>();
    void RecordSyncSnapshot(string documentId, string entityType, string jsonData);
    bool ImportSyncEntity(string entityType, string jsonData);
}
