using System.Runtime.InteropServices;
using PrivStack.Services.Native;
using NativeLib = PrivStack.Services.Native.NativeLibrary;

namespace PrivStack.Services.Sdk;

/// <summary>
/// Transport implementation that routes SDK calls through P/Invoke to the Rust core.
/// Handles native pointer marshalling and memory cleanup.
/// Used in standalone mode when Desktop owns the DuckDB databases directly.
/// </summary>
internal sealed class FfiSdkTransport : ISdkTransport
{
    private readonly IPrivStackRuntime _runtime;

    public FfiSdkTransport(IPrivStackRuntime runtime)
    {
        _runtime = runtime;
    }

    public bool IsReady => _runtime.IsInitialized;

    public string? Execute(string requestJson)
    {
        var ptr = NativeLib.Execute(requestJson);
        return MarshalAndFreeString(ptr);
    }

    public string? Search(string queryJson)
    {
        var ptr = NativeLib.Search(queryJson);
        return MarshalAndFreeString(ptr);
    }

    public int RegisterEntityType(string schemaJson) => NativeLib.RegisterEntityType(schemaJson);

    // =========================================================================
    // Database Maintenance
    // =========================================================================

    public PrivStackError DbMaintenance() => NativeLib.DbMaintenance();

    public string? DbDiagnostics()
    {
        var ptr = NativeLib.DbDiagnostics();
        return MarshalAndFreeString(ptr);
    }

    public string? FindOrphanEntities(string validTypesJson)
    {
        var ptr = NativeLib.FindOrphanEntities(validTypesJson);
        return MarshalAndFreeString(ptr);
    }

    public string? DeleteOrphanEntities(string validTypesJson)
    {
        var ptr = NativeLib.DeleteOrphanEntities(validTypesJson);
        return MarshalAndFreeString(ptr);
    }

    public string? CompactDatabases()
    {
        var ptr = NativeLib.CompactDatabases();
        return MarshalAndFreeString(ptr);
    }

    // =========================================================================
    // Vault
    // =========================================================================

    public bool VaultIsInitialized(string vaultId) => NativeLib.VaultIsInitialized(vaultId);

    public PrivStackError VaultInitialize(string vaultId, string password) =>
        NativeLib.VaultInitialize(vaultId, password);

    public PrivStackError VaultUnlock(string vaultId, string password) =>
        NativeLib.VaultUnlock(vaultId, password);

    public void VaultLock(string vaultId) => NativeLib.VaultLock(vaultId);

    public bool VaultIsUnlocked(string vaultId) => NativeLib.VaultIsUnlocked(vaultId);

    public PrivStackError VaultBlobStore(string vaultId, string blobId, byte[] data)
    {
        unsafe
        {
            fixed (byte* ptr = data)
            {
                return NativeLib.VaultBlobStore(vaultId, blobId, (nint)ptr, (nuint)data.Length);
            }
        }
    }

    public (byte[] data, PrivStackError result) VaultBlobRead(string vaultId, string blobId)
    {
        var result = NativeLib.VaultBlobRead(vaultId, blobId, out var outData, out var outLen);
        if (result != PrivStackError.Ok)
            return ([], result);

        try
        {
            var data = new byte[(int)outLen];
            Marshal.Copy(outData, data, 0, (int)outLen);
            return (data, PrivStackError.Ok);
        }
        finally
        {
            NativeLib.FreeBytes(outData, outLen);
        }
    }

    public PrivStackError VaultBlobDelete(string vaultId, string blobId) =>
        NativeLib.VaultBlobDelete(vaultId, blobId);

    // =========================================================================
    // Blob (Unencrypted)
    // =========================================================================

    public PrivStackError BlobStore(string ns, string blobId, byte[] data, string? metadataJson)
    {
        unsafe
        {
            fixed (byte* ptr = data)
            {
                return NativeLib.BlobStore(ns, blobId, (nint)ptr, (nuint)data.Length, metadataJson);
            }
        }
    }

    public (byte[] data, PrivStackError result) BlobRead(string ns, string blobId)
    {
        var result = NativeLib.BlobRead(ns, blobId, out var outData, out var outLen);
        if (result != PrivStackError.Ok)
            return ([], result);

        try
        {
            var data = new byte[(int)outLen];
            Marshal.Copy(outData, data, 0, (int)outLen);
            return (data, PrivStackError.Ok);
        }
        finally
        {
            NativeLib.FreeBytes(outData, outLen);
        }
    }

    public PrivStackError BlobDelete(string ns, string blobId) =>
        NativeLib.BlobDelete(ns, blobId);

    // =========================================================================
    // Helpers
    // =========================================================================

    /// <summary>
    /// Marshals a native UTF-8 string pointer to a managed string and frees the native memory.
    /// Returns null if the pointer is zero.
    /// </summary>
    private static string? MarshalAndFreeString(nint ptr)
    {
        if (ptr == nint.Zero)
            return null;

        try
        {
            return Marshal.PtrToStringUTF8(ptr);
        }
        finally
        {
            NativeLib.FreeString(ptr);
        }
    }

    public void Dispose()
    {
        // No resources to dispose — NativeLibrary lifecycle is managed by IPrivStackRuntime.
    }
}
