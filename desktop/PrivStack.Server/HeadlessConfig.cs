using System.Text.Json;
using System.Text.Json.Serialization;
using PrivStack.Services;

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
    public TlsConfig? Tls { get; set; }

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
}

[JsonConverter(typeof(JsonStringEnumConverter))]
public enum UnlockMethod
{
    PasswordEveryStart,
    OsKeyring,
    EnvironmentVariable,
}

public sealed class TlsConfig
{
    [JsonPropertyName("enabled")]
    public bool Enabled { get; set; }

    [JsonPropertyName("cert_path")]
    public string CertPath { get; set; } = "";

    [JsonPropertyName("key_path")]
    public string KeyPath { get; set; } = "";
}
