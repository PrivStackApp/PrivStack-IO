using System.Security.Cryptography.X509Certificates;

namespace PrivStack.Services.Api;

/// <summary>
/// TLS configuration for the local API server.
/// Supports manual certificate files (PFX/PEM) and Let's Encrypt (ACME) auto-provisioning.
/// </summary>
public sealed class TlsOptions
{
    public TlsMode Mode { get; set; }

    // ── Manual mode ──
    /// <summary>Path to certificate file (.pfx, .p12, or .pem).</summary>
    public string? CertPath { get; set; }

    /// <summary>Path to private key file (.pem). Only needed for PEM certificates.</summary>
    public string? KeyPath { get; set; }

    /// <summary>Password for PFX/P12 certificate files.</summary>
    public string? CertPassword { get; set; }

    // ── Let's Encrypt (ACME) mode ──
    /// <summary>Domain name(s) for the certificate.</summary>
    public string? Domain { get; set; }

    /// <summary>Email address for Let's Encrypt registration and renewal notices.</summary>
    public string? Email { get; set; }

    /// <summary>Must be true to use Let's Encrypt (required by ACME protocol).</summary>
    public bool AcceptTermsOfService { get; set; }

    /// <summary>Use Let's Encrypt staging environment (for testing).</summary>
    public bool UseStaging { get; set; }

    /// <summary>Directory to persist ACME certificates and account keys.</summary>
    public string? CertStorePath { get; set; }

    /// <summary>
    /// Loads the certificate from CertPath for manual TLS mode.
    /// Handles PFX/P12 and PEM formats.
    /// </summary>
    public X509Certificate2 LoadCertificate()
    {
        if (string.IsNullOrEmpty(CertPath))
            throw new InvalidOperationException("CertPath is required for manual TLS mode.");

        if (!File.Exists(CertPath))
            throw new FileNotFoundException($"Certificate file not found: {CertPath}");

        // PFX / P12 — may have embedded private key
        if (CertPath.EndsWith(".pfx", StringComparison.OrdinalIgnoreCase) ||
            CertPath.EndsWith(".p12", StringComparison.OrdinalIgnoreCase))
        {
            return X509CertificateLoader.LoadPkcs12FromFile(CertPath, CertPassword,
                X509KeyStorageFlags.MachineKeySet | X509KeyStorageFlags.EphemeralKeySet);
        }

        // PEM certificate + optional PEM private key
        var certPem = File.ReadAllText(CertPath);

        if (!string.IsNullOrEmpty(KeyPath))
        {
            if (!File.Exists(KeyPath))
                throw new FileNotFoundException($"Private key file not found: {KeyPath}");

            var keyPem = File.ReadAllText(KeyPath);
            var cert = X509Certificate2.CreateFromPem(certPem, keyPem);
            // Re-export to ensure private key is usable by Kestrel
            return X509CertificateLoader.LoadPkcs12(cert.Export(X509ContentType.Pfx), null);
        }

        // PEM with embedded private key
        var pemCert = X509Certificate2.CreateFromPem(certPem);
        return X509CertificateLoader.LoadPkcs12(pemCert.Export(X509ContentType.Pfx), null);
    }
}

public enum TlsMode
{
    /// <summary>User-provided certificate files (PFX, P12, or PEM).</summary>
    Manual,

    /// <summary>Automatic certificate via Let's Encrypt (ACME protocol).</summary>
    LetsEncrypt,
}
