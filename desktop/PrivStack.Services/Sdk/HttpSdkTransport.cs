using System.Net.Http.Headers;
using System.Text;
using System.Text.Json;
using PrivStack.Services.Native;
using Serilog;

namespace PrivStack.Services.Sdk;

/// <summary>
/// Transport implementation that proxies SDK calls over HTTP to a running headless server.
/// Used in client mode when Desktop detects a running server and delegates all data operations
/// to it, avoiding DuckDB single-process conflicts.
/// </summary>
internal sealed class HttpSdkTransport : ISdkTransport
{
    private static ILogger _log => Log.ForContext<HttpSdkTransport>();

    private readonly HttpClient _httpClient;

    public HttpSdkTransport(string serverUrl, string apiKey)
    {
        _httpClient = new HttpClient { BaseAddress = new Uri(serverUrl) };
        _httpClient.DefaultRequestHeaders.Add("X-API-Key", apiKey);
        _httpClient.Timeout = TimeSpan.FromSeconds(30);
    }

    /// <summary>
    /// Always ready — the server handles readiness checks.
    /// </summary>
    public bool IsReady => true;

    public string? Execute(string requestJson)
    {
        return PostJson("/api/v1/sdk/execute", requestJson);
    }

    public string? Search(string queryJson)
    {
        return PostJson("/api/v1/sdk/search", queryJson);
    }

    public int RegisterEntityType(string schemaJson)
    {
        var response = PostJson("/api/v1/sdk/register-entity-type", schemaJson);
        return ParseIntResult(response);
    }

    // =========================================================================
    // Database Maintenance
    // =========================================================================

    public PrivStackError DbMaintenance()
    {
        var response = PostJson("/api/v1/sdk/db/maintenance", "{}");
        return ParseErrorCode(response);
    }

    public string? DbDiagnostics()
    {
        return GetJson("/api/v1/sdk/db/diagnostics");
    }

    public string? FindOrphanEntities(string validTypesJson)
    {
        return PostJson("/api/v1/sdk/db/find-orphans", validTypesJson);
    }

    public string? DeleteOrphanEntities(string validTypesJson)
    {
        return PostJson("/api/v1/sdk/db/delete-orphans", validTypesJson);
    }

    public string? CompactDatabases()
    {
        return PostJson("/api/v1/sdk/db/compact", "{}");
    }

    // =========================================================================
    // Vault
    // =========================================================================

    public bool VaultIsInitialized(string vaultId)
    {
        var response = PostJson("/api/v1/sdk/vault/is-initialized", JsonSerializer.Serialize(new { vault_id = vaultId }));
        return ParseBool(response);
    }

    public PrivStackError VaultInitialize(string vaultId, string password)
    {
        var response = PostJson("/api/v1/sdk/vault/initialize",
            JsonSerializer.Serialize(new { vault_id = vaultId, password }));
        return ParseErrorCode(response);
    }

    public PrivStackError VaultUnlock(string vaultId, string password)
    {
        var response = PostJson("/api/v1/sdk/vault/unlock",
            JsonSerializer.Serialize(new { vault_id = vaultId, password }));
        return ParseErrorCode(response);
    }

    public void VaultLock(string vaultId)
    {
        PostJson("/api/v1/sdk/vault/lock", JsonSerializer.Serialize(new { vault_id = vaultId }));
    }

    public bool VaultIsUnlocked(string vaultId)
    {
        var response = PostJson("/api/v1/sdk/vault/is-unlocked", JsonSerializer.Serialize(new { vault_id = vaultId }));
        return ParseBool(response);
    }

    public PrivStackError VaultBlobStore(string vaultId, string blobId, byte[] data)
    {
        var payload = JsonSerializer.Serialize(new
        {
            vault_id = vaultId,
            blob_id = blobId,
            data = Convert.ToBase64String(data),
        });
        var response = PostJson("/api/v1/sdk/vault/blob-store", payload);
        return ParseErrorCode(response);
    }

    public (byte[] data, PrivStackError result) VaultBlobRead(string vaultId, string blobId)
    {
        var response = PostJson("/api/v1/sdk/vault/blob-read",
            JsonSerializer.Serialize(new { vault_id = vaultId, blob_id = blobId }));
        if (string.IsNullOrEmpty(response))
            return ([], PrivStackError.StorageError);

        try
        {
            using var doc = JsonDocument.Parse(response);
            var root = doc.RootElement;

            if (root.TryGetProperty("error_code", out var errProp) && errProp.GetInt32() != 0)
                return ([], (PrivStackError)errProp.GetInt32());

            if (root.TryGetProperty("data", out var dataProp))
            {
                var bytes = Convert.FromBase64String(dataProp.GetString()!);
                return (bytes, PrivStackError.Ok);
            }

            return ([], PrivStackError.StorageError);
        }
        catch (Exception ex)
        {
            _log.Error(ex, "Failed to parse vault blob read response");
            return ([], PrivStackError.StorageError);
        }
    }

    public PrivStackError VaultBlobDelete(string vaultId, string blobId)
    {
        var response = PostJson("/api/v1/sdk/vault/blob-delete",
            JsonSerializer.Serialize(new { vault_id = vaultId, blob_id = blobId }));
        return ParseErrorCode(response);
    }

    // =========================================================================
    // Blob (Unencrypted)
    // =========================================================================

    public PrivStackError BlobStore(string ns, string blobId, byte[] data, string? metadataJson)
    {
        var payload = JsonSerializer.Serialize(new
        {
            ns,
            blob_id = blobId,
            data = Convert.ToBase64String(data),
            metadata = metadataJson,
        });
        var response = PostJson("/api/v1/sdk/blob/store", payload);
        return ParseErrorCode(response);
    }

    public (byte[] data, PrivStackError result) BlobRead(string ns, string blobId)
    {
        var response = PostJson("/api/v1/sdk/blob/read",
            JsonSerializer.Serialize(new { ns, blob_id = blobId }));
        if (string.IsNullOrEmpty(response))
            return ([], PrivStackError.StorageError);

        try
        {
            using var doc = JsonDocument.Parse(response);
            var root = doc.RootElement;

            if (root.TryGetProperty("error_code", out var errProp) && errProp.GetInt32() != 0)
                return ([], (PrivStackError)errProp.GetInt32());

            if (root.TryGetProperty("data", out var dataProp))
            {
                var bytes = Convert.FromBase64String(dataProp.GetString()!);
                return (bytes, PrivStackError.Ok);
            }

            return ([], PrivStackError.StorageError);
        }
        catch (Exception ex)
        {
            _log.Error(ex, "Failed to parse blob read response");
            return ([], PrivStackError.StorageError);
        }
    }

    public PrivStackError BlobDelete(string ns, string blobId)
    {
        var response = PostJson("/api/v1/sdk/blob/delete",
            JsonSerializer.Serialize(new { ns, blob_id = blobId }));
        return ParseErrorCode(response);
    }

    // =========================================================================
    // HTTP Helpers
    // =========================================================================

    private string? PostJson(string path, string json)
    {
        var sw = System.Diagnostics.Stopwatch.StartNew();
        try
        {
            using var content = new StringContent(json, Encoding.UTF8, "application/json");
            using var request = new HttpRequestMessage(HttpMethod.Post, path) { Content = content };
            using var response = _httpClient.Send(request, HttpCompletionOption.ResponseContentRead);

            using var stream = response.Content.ReadAsStream();
            using var reader = new StreamReader(stream, Encoding.UTF8);
            var body = reader.ReadToEnd();
            sw.Stop();

            if (!response.IsSuccessStatusCode)
            {
                _log.Warning("[Client→Server] POST {Path} → {StatusCode} ({Elapsed}ms): {Body}",
                    path, (int)response.StatusCode, sw.ElapsedMilliseconds,
                    body.Length > 200 ? body[..200] : body);
            }
            else if (sw.ElapsedMilliseconds > 500)
            {
                _log.Warning("[Client→Server] POST {Path} → SLOW {Elapsed}ms ({Len}B)",
                    path, sw.ElapsedMilliseconds, body.Length);
            }

            return body;
        }
        catch (Exception ex)
        {
            sw.Stop();
            _log.Error(ex, "[Client→Server] POST {Path} FAILED ({Elapsed}ms)", path, sw.ElapsedMilliseconds);
            return null;
        }
    }

    private string? GetJson(string path)
    {
        var sw = System.Diagnostics.Stopwatch.StartNew();
        try
        {
            using var request = new HttpRequestMessage(HttpMethod.Get, path);
            using var response = _httpClient.Send(request, HttpCompletionOption.ResponseContentRead);

            using var stream = response.Content.ReadAsStream();
            using var reader = new StreamReader(stream, Encoding.UTF8);
            var body = reader.ReadToEnd();
            sw.Stop();

            if (!response.IsSuccessStatusCode)
            {
                _log.Warning("[Client→Server] GET {Path} → {StatusCode} ({Elapsed}ms)", path, (int)response.StatusCode, sw.ElapsedMilliseconds);
            }

            return body;
        }
        catch (Exception ex)
        {
            sw.Stop();
            _log.Error(ex, "[Client→Server] GET {Path} FAILED ({Elapsed}ms)", path, sw.ElapsedMilliseconds);
            return null;
        }
    }

    private static PrivStackError ParseErrorCode(string? json)
    {
        if (string.IsNullOrEmpty(json))
            return PrivStackError.StorageError;

        try
        {
            using var doc = JsonDocument.Parse(json);
            if (doc.RootElement.TryGetProperty("error_code", out var errProp))
                return (PrivStackError)errProp.GetInt32();
            return PrivStackError.Ok;
        }
        catch
        {
            return PrivStackError.StorageError;
        }
    }

    private static int ParseIntResult(string? json)
    {
        if (string.IsNullOrEmpty(json))
            return -1;

        try
        {
            using var doc = JsonDocument.Parse(json);
            if (doc.RootElement.TryGetProperty("result", out var resultProp))
                return resultProp.GetInt32();
            return -1;
        }
        catch
        {
            return -1;
        }
    }

    private static bool ParseBool(string? json)
    {
        if (string.IsNullOrEmpty(json))
            return false;

        try
        {
            using var doc = JsonDocument.Parse(json);
            if (doc.RootElement.TryGetProperty("result", out var resultProp))
                return resultProp.GetBoolean();
            return false;
        }
        catch
        {
            return false;
        }
    }

    public void Dispose()
    {
        _httpClient.Dispose();
    }
}
