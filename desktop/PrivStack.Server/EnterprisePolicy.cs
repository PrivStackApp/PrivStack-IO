using System.Security.Cryptography;
using System.Text;
using Tomlyn;
using Tomlyn.Model;

namespace PrivStack.Server;

/// <summary>
/// Enterprise policy loaded from a TOML file.
/// Supports optional ECDSA P-256 signing to prevent tampering.
/// </summary>
public sealed class EnterprisePolicy
{
    /// <summary>ECDSA P-256 public key for signature verification (base64).</summary>
    public string? AuthorityPublicKey { get; set; }

    /// <summary>Signature over the policy body (base64). Covers all sections except [authority].</summary>
    public string? AuthoritySignature { get; set; }

    /// <summary>Plugin allowlist/blocklist mode.</summary>
    public PluginPolicy Plugins { get; set; } = new();

    /// <summary>Network access restrictions.</summary>
    public NetworkPolicy Network { get; set; } = new();

    /// <summary>API security requirements.</summary>
    public ApiPolicy Api { get; set; } = new();

    /// <summary>Audit logging configuration.</summary>
    public AuditPolicy Audit { get; set; } = new();

    /// <summary>
    /// Loads a policy from a TOML file. Returns null if the file doesn't exist.
    /// Throws if the file exists but is malformed or fails signature verification.
    /// </summary>
    public static EnterprisePolicy? LoadFromFile(string path)
    {
        if (!File.Exists(path)) return null;

        var toml = File.ReadAllText(path);
        var model = Toml.ToModel(toml);
        var policy = new EnterprisePolicy();

        // [authority]
        if (model.TryGetValue("authority", out var authObj) && authObj is TomlTable authTable)
        {
            policy.AuthorityPublicKey = GetString(authTable, "public_key");
            policy.AuthoritySignature = GetString(authTable, "signature");
        }

        // [plugins]
        if (model.TryGetValue("plugins", out var plugObj) && plugObj is TomlTable plugTable)
        {
            policy.Plugins.Mode = GetString(plugTable, "mode") ?? "disabled";
            policy.Plugins.List = GetStringArray(plugTable, "list");
        }

        // [network]
        if (model.TryGetValue("network", out var netObj) && netObj is TomlTable netTable)
        {
            policy.Network.AllowedCidrs = GetStringArray(netTable, "allowed_cidrs");
        }

        // [api]
        if (model.TryGetValue("api", out var apiObj) && apiObj is TomlTable apiTable)
        {
            policy.Api.RequireTls = GetBool(apiTable, "require_tls");
        }

        // [audit]
        if (model.TryGetValue("audit", out var auditObj) && auditObj is TomlTable auditTable)
        {
            policy.Audit.Enabled = GetBool(auditTable, "enabled");
            policy.Audit.LogPath = GetString(auditTable, "log_path");
            policy.Audit.Level = GetString(auditTable, "level") ?? "all";
        }

        // Verify signature if authority section is present
        if (!string.IsNullOrEmpty(policy.AuthorityPublicKey) && !string.IsNullOrEmpty(policy.AuthoritySignature))
        {
            VerifySignature(toml, policy.AuthorityPublicKey, policy.AuthoritySignature);
        }

        return policy;
    }

    /// <summary>
    /// Signs a policy file using an ECDSA P-256 private key.
    /// Writes the signature into the [authority] section.
    /// Used by admin tooling to create signed policies.
    /// </summary>
    public static string SignPolicy(string tomlContent, ECDsa privateKey)
    {
        var bodyToSign = ExtractPolicyBody(tomlContent);
        var bodyBytes = Encoding.UTF8.GetBytes(bodyToSign);
        var signature = privateKey.SignData(bodyBytes, HashAlgorithmName.SHA256);
        return Convert.ToBase64String(signature);
    }

    /// <summary>
    /// Generates a new ECDSA P-256 key pair for policy signing.
    /// Returns (publicKeyBase64, privateKeyBase64).
    /// </summary>
    public static (string PublicKey, string PrivateKey) GenerateSigningKeyPair()
    {
        using var ecdsa = ECDsa.Create(ECCurve.NamedCurves.nistP256);
        var pubKey = Convert.ToBase64String(ecdsa.ExportSubjectPublicKeyInfo());
        var privKey = Convert.ToBase64String(ecdsa.ExportPkcs8PrivateKey());
        return (pubKey, privKey);
    }

    private static void VerifySignature(string toml, string publicKeyBase64, string signatureBase64)
    {
        var bodyToVerify = ExtractPolicyBody(toml);
        var bodyBytes = Encoding.UTF8.GetBytes(bodyToVerify);
        var pubKeyBytes = Convert.FromBase64String(publicKeyBase64);
        var sigBytes = Convert.FromBase64String(signatureBase64);

        using var ecdsa = ECDsa.Create();
        ecdsa.ImportSubjectPublicKeyInfo(pubKeyBytes, out _);

        if (!ecdsa.VerifyData(bodyBytes, sigBytes, HashAlgorithmName.SHA256))
        {
            throw new InvalidOperationException("Enterprise policy signature verification failed. The policy file may have been tampered with.");
        }
    }

    /// <summary>
    /// Extracts the policy body (everything except the [authority] section) for signing/verification.
    /// </summary>
    private static string ExtractPolicyBody(string toml)
    {
        var lines = toml.Split('\n');
        var sb = new StringBuilder();
        var inAuthority = false;

        foreach (var line in lines)
        {
            var trimmed = line.TrimStart();

            // Detect section headers
            if (trimmed.StartsWith('[') && !trimmed.StartsWith("[["))
            {
                inAuthority = trimmed.StartsWith("[authority]", StringComparison.OrdinalIgnoreCase);
            }

            if (!inAuthority)
            {
                sb.AppendLine(line);
            }
        }

        return sb.ToString().TrimEnd();
    }

    private static string? GetString(TomlTable table, string key)
        => table.TryGetValue(key, out var val) ? val?.ToString() : null;

    private static bool GetBool(TomlTable table, string key)
        => table.TryGetValue(key, out var val) && val is bool b && b;

    private static List<string> GetStringArray(TomlTable table, string key)
    {
        if (!table.TryGetValue(key, out var val)) return [];
        if (val is not TomlArray arr) return [];
        return arr.OfType<string>().ToList();
    }
}

public sealed class PluginPolicy
{
    /// <summary>"allowlist" or "blocklist". Default "disabled" means no enforcement.</summary>
    public string Mode { get; set; } = "disabled";

    /// <summary>Plugin IDs in the allow/block list.</summary>
    public List<string> List { get; set; } = [];
}

public sealed class NetworkPolicy
{
    /// <summary>CIDR ranges allowed to access the API. Empty means no restriction.</summary>
    public List<string> AllowedCidrs { get; set; } = [];
}

public sealed class ApiPolicy
{
    /// <summary>Refuse to start if TLS is not configured.</summary>
    public bool RequireTls { get; set; }
}

public sealed class AuditPolicy
{
    /// <summary>Enable audit logging.</summary>
    public bool Enabled { get; set; }

    /// <summary>Path for the audit log file (JSON Lines format).</summary>
    public string? LogPath { get; set; }

    /// <summary>"all", "write", or "auth". Controls what gets logged.</summary>
    public string Level { get; set; } = "all";
}
