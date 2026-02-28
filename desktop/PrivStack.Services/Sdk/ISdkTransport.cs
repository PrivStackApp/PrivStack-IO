using PrivStack.Services.Native;

namespace PrivStack.Services.Sdk;

/// <summary>
/// Abstraction over the data transport layer used by <see cref="SdkHost"/>.
/// In standalone mode, <see cref="FfiSdkTransport"/> routes calls through P/Invoke to the Rust core.
/// In client mode, <see cref="HttpSdkTransport"/> proxies calls over HTTP to a running headless server.
/// </summary>
internal interface ISdkTransport : IDisposable
{
    /// <summary>
    /// Whether the transport is ready to accept requests.
    /// </summary>
    bool IsReady { get; }

    /// <summary>
    /// Executes a generic SDK operation. Takes serialized SdkMessage JSON, returns response JSON.
    /// </summary>
    string? Execute(string requestJson);

    /// <summary>
    /// Cross-plugin full-text search. Takes serialized query JSON, returns response JSON.
    /// </summary>
    string? Search(string queryJson);

    // =========================================================================
    // Database Maintenance
    // =========================================================================

    PrivStackError DbMaintenance();
    string? DbDiagnostics();
    string? FindOrphanEntities(string validTypesJson);
    string? DeleteOrphanEntities(string validTypesJson);
    string? CompactDatabases();

    // =========================================================================
    // Vault (Encrypted Blob Storage)
    // =========================================================================

    bool VaultIsInitialized(string vaultId);
    PrivStackError VaultInitialize(string vaultId, string password);
    PrivStackError VaultUnlock(string vaultId, string password);
    void VaultLock(string vaultId);
    bool VaultIsUnlocked(string vaultId);
    PrivStackError VaultBlobStore(string vaultId, string blobId, byte[] data);
    (byte[] data, PrivStackError result) VaultBlobRead(string vaultId, string blobId);
    PrivStackError VaultBlobDelete(string vaultId, string blobId);

    // =========================================================================
    // Blob (Unencrypted Blob Storage)
    // =========================================================================

    PrivStackError BlobStore(string ns, string blobId, byte[] data, string? metadataJson);
    (byte[] data, PrivStackError result) BlobRead(string ns, string blobId);
    PrivStackError BlobDelete(string ns, string blobId);
}
