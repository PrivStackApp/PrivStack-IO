using System.Text.Json;
using System.Text.Json.Serialization;
using PrivStack.Services;
using PrivStack.Services.Api;

namespace PrivStack.Server;

/// <summary>
/// Server-specific configuration stored at {DataPaths.BaseDir}/headless-config.json.
/// Separate from desktop's window-settings.json.
/// </summary>
public sealed class HeadlessConfig
{
    private static readonly JsonSerializerOptions _jsonOptions = new()
    {
        WriteIndented = true,
        Converters = { new JsonStringEnumConverter() },
    };

    [JsonPropertyName("unlock_method")]
    public UnlockMethod UnlockMethod { get; set; } = UnlockMethod.PasswordEveryStart;

    [JsonPropertyName("bind_address")]
    public string BindAddress { get; set; } = "127.0.0.1";

    [JsonPropertyName("port")]
    public int Port { get; set; } = 9720;

    [JsonPropertyName("tls")]
    public ServerTlsConfig? Tls { get; set; }

    [JsonPropertyName("policy_path")]
    public string? PolicyPath { get; set; }

    private static string ConfigPath => Path.Combine(DataPaths.BaseDir, "headless-config.json");

    public static HeadlessConfig Load()
    {
        var path = ConfigPath;
        if (!File.Exists(path)) return new HeadlessConfig();

        try
        {
            var json = File.ReadAllText(path);
            return JsonSerializer.Deserialize<HeadlessConfig>(json, _jsonOptions) ?? new HeadlessConfig();
        }
        catch
        {
            return new HeadlessConfig();
        }
    }

    public void Save()
    {
        var path = ConfigPath;
        Directory.CreateDirectory(Path.GetDirectoryName(path)!);
        var json = JsonSerializer.Serialize(this, _jsonOptions);
        File.WriteAllText(path, json);
    }

    /// <summary>
    /// Converts the server TLS config to the shared TlsOptions used by LocalApiServer.
    /// Returns null if TLS is not enabled.
    /// </summary>
    public TlsOptions? ToTlsOptions()
    {
        if (Tls is not { Enabled: true }) return null;

        return new TlsOptions
        {
            Mode = Tls.Mode,
            CertPath = Tls.CertPath,
            KeyPath = Tls.KeyPath,
            CertPassword = Tls.CertPassword,
            Domain = Tls.Domain,
            Email = Tls.Email,
            AcceptTermsOfService = Tls.AcceptTermsOfService,
            UseStaging = Tls.UseStaging,
            CertStorePath = Tls.CertStorePath ?? Path.Combine(DataPaths.BaseDir, "certs"),
        };
    }
}

[JsonConverter(typeof(JsonStringEnumConverter))]
public enum UnlockMethod
{
    PasswordEveryStart,
    OsKeyring,
    EnvironmentVariable,
}

/// <summary>
/// TLS configuration for the headless server.
/// Supports manual certificates (PFX/PEM) and automatic Let's Encrypt provisioning.
/// </summary>
public sealed class ServerTlsConfig
{
    [JsonPropertyName("enabled")]
    public bool Enabled { get; set; }

    [JsonPropertyName("mode")]
    public TlsMode Mode { get; set; } = TlsMode.Manual;

    // ── Manual mode ──

    [JsonPropertyName("cert_path")]
    public string CertPath { get; set; } = "";

    [JsonPropertyName("key_path")]
    public string KeyPath { get; set; } = "";

    [JsonPropertyName("cert_password")]
    public string? CertPassword { get; set; }

    // ── Let's Encrypt (ACME) mode ──

    [JsonPropertyName("domain")]
    public string? Domain { get; set; }

    [JsonPropertyName("email")]
    public string? Email { get; set; }

    [JsonPropertyName("accept_tos")]
    public bool AcceptTermsOfService { get; set; }

    [JsonPropertyName("use_staging")]
    public bool UseStaging { get; set; }

    [JsonPropertyName("cert_store_path")]
    public string? CertStorePath { get; set; }
}
