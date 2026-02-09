using PrivStack.Desktop.Models;

namespace PrivStack.Desktop.Native;

/// <summary>
/// Manages device pairing, sync codes, and trusted peer relationships.
/// </summary>
public interface IPairingService
{
    SyncCode GenerateSyncCode();
    void SetSyncCode(string code);
    SyncCode? GetSyncCode();
    void ClearSyncCode();
    List<PairingPeerInfo> GetPairingDiscoveredPeers();
    void ApprovePeer(string peerId);
    void RejectPeer(string peerId);
    List<TrustedPeer> GetTrustedPeers();
    void RemoveTrustedPeer(string peerId);
    bool IsPeerTrusted(string peerId);
    string SavePairingState();
    void LoadPairingState(string json);
    string GetDeviceName();
    void SetDeviceName(string name);
}
